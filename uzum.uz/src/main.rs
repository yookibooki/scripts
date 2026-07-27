use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const GRAPHQL_URL: &str = "https://graphql.uzum.uz/";
const REST_BASE: &str = "https://api.uzum.uz/api";
const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const BATCH_SIZE: u64 = 100;
const OFFSET_CAP: u64 = 9900;

// ── HTTP helpers (local — uzum needs unique timeouts + config) ──────

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent()
}

// ── API deserialization ─────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GqlError>>,
}

#[derive(Debug, Deserialize)]
struct GqlError {
    message: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CategoryNode {
    id: u64,
    title: Option<String>,
    children: Option<Vec<CategoryNode>>,
}

#[derive(Debug, Deserialize)]
struct CategoriesResponse {
    payload: Option<Vec<CategoryNode>>,
}

// ── State ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CategoryProgress {
    total: u64,
    offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct StateFile {
    version: u64,
    categories: HashMap<String, CategoryProgress>,
    item_count: u64,
    updated_at: String,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            version: 1,
            categories: HashMap::new(),
            item_count: 0,
            updated_at: String::new(),
        }
    }
}

// ── Paths & auth ────────────────────────────────────────────────────

fn data_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local/share/uzum")
}

fn read_auth() -> (Option<String>, Option<String>) {
    (
        std::env::var("UZUM_ACCESS_TOKEN").ok(),
        std::env::var("UZUM_INSTALL_ID").ok(),
    )
}

fn persist(path: &PathBuf, categories: &HashMap<String, CategoryProgress>, item_count: u64) {
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    let state = StateFile {
        version: 1,
        categories: categories.clone(),
        item_count,
        updated_at: ts,
    };
    let tmp = path.with_extension("tmp");
    if let Ok(json) = serde_json::to_string(&state) {
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, path);
        }
    }
}

// ── Category tree ───────────────────────────────────────────────────

fn fetch_category_tree(agent: &ureq::Agent) -> Vec<CategoryNode> {
    let url = format!("{REST_BASE}/main/root-categories?eco=false");
    let (token, iid) = read_auth();
    let mut req = agent.get(&url).header("Accept", "application/json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(i) = iid {
        req = req.header("X-Iid", i);
    }
    let resp = match req.call() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] Categories fetch failed: {e}");
            return vec![];
        }
    };
    if resp.status() != 200 {
        eprintln!("[ERROR] Categories HTTP {}", resp.status());
        return vec![];
    }
    let text = match resp.into_body().read_to_string() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[ERROR] Read categories response: {e}");
            return vec![];
        }
    };
    match serde_json::from_str::<CategoriesResponse>(&text) {
        Ok(r) => r.payload.unwrap_or_default(),
        Err(e) => {
            eprintln!("[ERROR] Parse category tree: {e}");
            vec![]
        }
    }
}

fn leaf_nodes(nodes: &[CategoryNode]) -> Vec<&CategoryNode> {
    let mut out = Vec::new();
    for n in nodes {
        match &n.children {
            Some(children) if !children.is_empty() => out.extend(leaf_nodes(children)),
            _ => out.push(n),
        }
    }
    out
}

// ── GraphQL ─────────────────────────────────────────────────────────

fn graphql_request(
    agent: &ureq::Agent,
    query: &str,
    vars: &serde_json::Value,
    op_name: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut body = serde_json::json!({ "query": query, "variables": vars });
    if let Some(name) = op_name {
        body["operationName"] = serde_json::json!(name);
    }
    let body_str = serde_json::to_string(&body).map_err(|e| format!("Serialize: {e}"))?;
    let (token, iid) = read_auth();
    let mut req = agent
        .post(GRAPHQL_URL)
        .header("Content-Type", "application/json")
        .header("apollographql-client-name", "web-customers")
        .header("apollographql-client-version", "1.63.2")
        .header("city-id", "1");
    if let Some(t) = &token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(i) = &iid {
        req = req.header("X-Iid", i);
    }
    let resp = req.send(&body_str).map_err(|e| format!("HTTP: {e}"))?;
    let text = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Body: {e}"))?;
    let parsed: GqlResponse<serde_json::Value> =
        serde_json::from_str(&text).map_err(|e| format!("JSON: {e}"))?;
    if let Some(errs) = parsed.errors {
        let msg: Vec<String> = errs.into_iter().map(|e| e.message).collect();
        return Err(msg.join("; "));
    }
    parsed.data.ok_or_else(|| "No data".into())
}

fn search_category(
    agent: &ureq::Agent,
    category_id: u64,
    offset: u64,
    limit: u64,
) -> Result<serde_json::Value, String> {
    let q = r#"
        query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) {
            makeSearch(query: $queryInput) {
                items {
                    catalogCard {
                        productId title
                        minFullPrice minSellPrice
                        feedbackQuantity rating
                        buyingOptions {
                            isSingleSku
                            deliveryOptions { shortDate stockType }
                        }
                        promoFutureInfo { minFuturePrice minFuturePriceDate }
                        badges { id text backgroundColor textColor }
                    }
                }
                total
            }
        }
    "#;
    let vars = serde_json::json!({
        "queryInput": {
            "categoryId": category_id.to_string(),
            "showAdultContent": "TRUE",
            "filters": [],
            "sort": "BY_ORDERS_NUMBER_DESC",
            "pagination": { "offset": offset, "limit": limit },
            "correctQuery": false,
            "getFastCategories": false,
            "getFastFacets": false,
        }
    });
    let data = graphql_request(agent, q, &vars, Some("MakeSearch_ItemsAndFilters"))?;
    Ok(data["makeSearch"].clone())
}

// ── Main ────────────────────────────────────────────────────────────

fn main() {
    let is_refresh = std::env::args().any(|a| a == "--refresh");

    let root = data_root();
    fs::create_dir_all(&root).expect("Failed to create data directory");

    let lock_path = root.join("uzum.lock");
    let lock_file = File::create(&lock_path).expect("Failed to create lock file");
    fs2::FileExt::try_lock_exclusive(&lock_file).expect("Another instance is already running");

    let agent = build_agent();
    let start = Instant::now();

    eprintln!("[INFO] Fetching category tree...");
    let tree = fetch_category_tree(&agent);
    if tree.is_empty() {
        eprintln!("[ERROR] No categories loaded. Check network or auth.");
        std::process::exit(1);
    }
    let leaves = leaf_nodes(&tree);
    eprintln!("[INFO] {} leaf categories", leaves.len());

    let state_path = root.join("state.json");
    let out_path = root.join("uzum_data.jsonl");

    let saved: StateFile = fs::read_to_string(&state_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut categories = saved.categories;
    let mut item_count = saved.item_count;

    let mut writer: Box<dyn Write> = if is_refresh && out_path.exists() {
        Box::new(BufWriter::new(
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&out_path)
                .unwrap(),
        ))
    } else {
        let file = File::create(&out_path).unwrap();
        let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
        let mut w = BufWriter::new(file);
        writeln!(
            w,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "exportedAt": &ts,
                "totalProducts": 0,
                "version": "1.0.0",
                "source": "uzum.uz"
            }))
            .unwrap()
        )
        .ok();
        Box::new(w)
    };

    let total = leaves.len();

    for (i, node) in leaves.iter().enumerate() {
        let cid = node.id;
        let ctitle = node.title.as_deref().unwrap_or("?");
        let key = cid.to_string();
        let d = i as u64 + 1;

        let maybe_progress = |k: &str| -> Option<&CategoryProgress> {
            if is_refresh { categories.get(k) } else { None }
        };

        if let Some(p) = maybe_progress(&key) {
            if p.offset >= OFFSET_CAP || p.offset >= p.total {
                if d % 50 == 0 || d == total as u64 {
                    log_progress(d, total, item_count, &start);
                    persist(&state_path, &categories, item_count);
                }
                continue;
            }
        }

        let first = match search_category(&agent, cid, 0, BATCH_SIZE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[WARN] {ctitle} ({cid}): {e}");
                log_progress_if_needed(d, total, item_count, &start, &state_path, &categories);
                continue;
            }
        };

        let api_total = first["total"].as_u64().unwrap_or(0);
        if api_total == 0 {
            log_progress_if_needed(d, total, item_count, &start, &state_path, &categories);
            categories.insert(key, CategoryProgress { total: 0, offset: 0 });
            continue;
        }

        if is_refresh {
            if let Some(p) = categories.get(&key) {
                if p.total == api_total && p.offset >= api_total.min(OFFSET_CAP) {
                    log_progress_if_needed(d, total, item_count, &start, &state_path, &categories);
                    continue;
                }
            }
        }

        let limit = api_total.min(OFFSET_CAP);
        let page_0_items = first["items"]
            .as_array()
            .map(|v| v.to_vec())
            .unwrap_or_default();

        for item_val in &page_0_items {
            let _ = writeln!(writer, "{}", serde_json::to_string(item_val).unwrap());
            item_count += 1;
        }

        let mut page = 1u64;
        loop {
            let offset = page * BATCH_SIZE;
            if offset >= limit {
                break;
            }
            match search_category(&agent, cid, offset, BATCH_SIZE) {
                Ok(r) => {
                    let items = r["items"]
                        .as_array()
                        .map(|v| v.to_vec())
                        .unwrap_or_default();
                    if items.is_empty() {
                        break;
                    }
                    for item_val in &items {
                        let _ = writeln!(writer, "{}", serde_json::to_string(item_val).unwrap());
                        item_count += 1;
                    }
                    page += 1;
                }
                Err(e) => {
                    if e.contains("too big query offset") {
                        break;
                    }
                    eprintln!("[WARN] {ctitle} ({cid}) offset {offset}: {e}");
                    break;
                }
            }
        }

        categories.insert(
            key,
            CategoryProgress {
                total: api_total,
                offset: (page * BATCH_SIZE).min(limit),
            },
        );

        if d % 50 == 0 || d == total as u64 {
            log_progress(d, total, item_count, &start);
            persist(&state_path, &categories, item_count);
        }
    }

    persist(&state_path, &categories, item_count);

    let elapsed = start.elapsed();
    eprintln!(
        "[INFO] Done: {item_count} items in {}.{:03}s",
        elapsed.as_secs(),
        elapsed.subsec_millis()
    );
    eprintln!("[INFO] Output: {}", out_path.display());
}

// ── Helpers ─────────────────────────────────────────────────────────

fn log_progress(d: u64, total: usize, item_count: u64, start: &Instant) {
    let el = start.elapsed();
    eprintln!(
        "[INFO] {d}/{total} — {item_count} items [{}.{:03}s]",
        el.as_secs(),
        el.subsec_millis()
    );
}

fn log_progress_if_needed(
    d: u64,
    total: usize,
    item_count: u64,
    start: &Instant,
    state_path: &PathBuf,
    categories: &HashMap<String, CategoryProgress>,
) {
    if d % 50 == 0 || d == total as u64 {
        log_progress(d, total, item_count, start);
        persist(state_path, categories, item_count);
    }
}

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const GRAPHQL_URL: &str = "https://graphql.uzum.uz/";
const REST_BASE: &str = "https://api.uzum.uz/api";
const ID_BASE: &str = "https://id.uzum.uz";
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
struct ScopeState {
    category_id: u64,
    total: u64,
    #[serde(default)]
    max_product_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct StateFile {
    version: u64,
    scopes: Vec<ScopeState>,
    item_count: u64,
    updated_at: String,
}

impl Default for StateFile {
    fn default() -> Self {
        Self {
            version: 1,
            scopes: Vec::new(),
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

/// POST to id.uzum.uz/api/auth/token with Bearer token + browser-like headers.
/// Returns new access_token from Set-Cookie response header.
/// Works even when the current token is expired — the endpoint accepts it as refresh auth.
fn refresh_token(agent: &ureq::Agent, token: &str) -> Result<String, String> {
    let req = agent
        .post(format!("{ID_BASE}/api/auth/token"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("Origin", "https://uzum.uz")
        .header("Referer", "https://uzum.uz/")
        .header("Accept", "*/*")
        .header("Accept-Language", "en-US,en;q=0.9");
    let resp = req.send_empty().map_err(|e| format!("refresh HTTP: {e}"))?;
    // Iterate all Set-Cookie headers — server may send access_token and refresh_token
    // as separate headers. Pick the one that starts with "access_token=".
    for cookie_val in resp.headers().get_all("set-cookie") {
        let cookie_str = cookie_val
            .to_str()
            .map_err(|e| format!("invalid set-cookie header: {e}"))?;
        for part in cookie_str.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("access_token=") {
                if !val.is_empty() {
                    return Ok(val.to_string());
                }
            }
        }
    }
    Err("access_token not found in refresh set-cookie".into())
}

fn persist(path: &PathBuf, scopes: &[ScopeState], item_count: u64) {
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string();
    let state = StateFile {
        version: 1,
        scopes: scopes.to_vec(),
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

/// Walk the category tree top-down and decide which scopes to query.
/// For each category: if total <= OFFSET_CAP or it's a leaf → query here (no dupes).
/// If total > OFFSET_CAP and has children → drill into children.
fn discover_scopes(agent: &ureq::Agent, nodes: &[CategoryNode]) -> Vec<ScopeState> {
    let mut scopes = Vec::new();
    for node in nodes {
        let title = node.title.as_deref().unwrap_or("?");
        let total = match search_category(agent, node.id, 0, 1) {
            Ok(r) => r["total"].as_u64().unwrap_or(0),
            Err(e) => {
                eprintln!("[WARN] Skipping scope {} ({}): {e}", title, node.id);
                continue;
            }
        };
        if total == 0 {
            continue;
        }

        let has_children = node.children.as_ref().map(|c| !c.is_empty()).unwrap_or(false);

        if total <= OFFSET_CAP || !has_children {
            // This scope fully covers the category, or we can't drill deeper
            eprintln!("[INFO] Scope {} ({}): {} products", title, node.id, total);
            scopes.push(ScopeState {
                category_id: node.id,
                total,
                max_product_id: 0,
            });
        } else {
            // Total exceeds cap — drill into children
            eprintln!("[INFO] Drilling {} ({}): {} products > {} cap", title, node.id, total, OFFSET_CAP);
            scopes.extend(discover_scopes(agent, node.children.as_ref().unwrap()));
        }
    }
    scopes
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
    // Retry once on 401: refresh token, update env var, rebuild request
    for attempt in 0..2 {
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

        let status = resp.status();
        if status == 401 && attempt == 0 {
            let (t_opt, _iid_opt) = read_auth();
            if let Some(ref t) = t_opt {
                match refresh_token(agent, t) {
                    Ok(new_token) => {
                        // SAFETY: single-threaded CLI, no concurrent access to env
                        unsafe { std::env::set_var("UZUM_ACCESS_TOKEN", &new_token) };
                        eprintln!("[INFO] Token refreshed on 401");
                        continue;
                    }
                    Err(e) => {
                        eprintln!("[WARN] Token refresh on 401 failed: {e}");
                    }
                }
            }
        }

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
        return parsed.data.ok_or_else(|| "No data".into());
    }
    // Unreachable — the loop returns on first non-401 or after retry
    Err("Request failed after 401 retry".into())
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
            "sort": "BY_DATE_ADDED_DESC",
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
    let root = data_root();
    fs::create_dir_all(&root).expect("Failed to create data directory");

    let lock_path = root.join("uzum.lock");
    let lock_file = File::create(&lock_path).expect("Failed to create lock file");
    fs2::FileExt::try_lock_exclusive(&lock_file).expect("Another instance is already running");

    let agent = build_agent();

    // ── Proactive token refresh ──────────────────────────────────
    // The refresh endpoint accepts a (possibly expired) Bearer token + browser-like
    // headers and returns a fresh access_token in the Set-Cookie response header.
    let (token, _iid) = read_auth();
    if let Some(ref t) = token {
        match refresh_token(&agent, t) {
            Ok(new_token) => {
                // SAFETY: single-threaded CLI, no concurrent access to env
                unsafe { std::env::set_var("UZUM_ACCESS_TOKEN", &new_token) };
                eprintln!("[INFO] Token refreshed on startup");
            }
            Err(e) => {
                eprintln!("[WARN] Startup token refresh failed: {e}");
            }
        }
    }

    let start = Instant::now();

    let state_path = root.join("state.json");
    let out_path = root.join("uzum_data.jsonl");

    let saved: StateFile = fs::read_to_string(&state_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut item_count = saved.item_count;

    let is_incremental = state_path.exists() && out_path.exists();

    // ── Phase 1: determine scopes ──────────────────────────────────

    let mut scopes: Vec<ScopeState>;

    if is_incremental && !saved.scopes.is_empty() {
        eprintln!("[INFO] Incremental mode — using {} cached scopes", saved.scopes.len());
        scopes = saved.scopes;
    } else {
        eprintln!("[INFO] Fresh mode — discovering scopes...");
        let tree = fetch_category_tree(&agent);
        if tree.is_empty() {
            eprintln!("[ERROR] No categories loaded. Check network or auth.");
            std::process::exit(1);
        }
        scopes = discover_scopes(&agent, &tree);
        eprintln!("[INFO] {} scopes to collect", scopes.len());
    }

    // ── Phase 2: set up writer ─────────────────────────────────────

    let mut writer: Box<dyn Write> = if is_incremental {
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

    // ── Phase 3: collect from each scope ───────────────────────────

    let total_scopes = scopes.len();

    for idx in 0..scopes.len() {
        let cid = scopes[idx].category_id;
        let d = idx as u64 + 1;
        let saved_max = if is_incremental { scopes[idx].max_product_id } else { 0 };

        let first = match search_category(&agent, cid, 0, BATCH_SIZE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[WARN] scope {cid}: {e}");
                log_progress_if_needed(d, total_scopes, item_count, &start, &state_path, &scopes);
                continue;
            }
        };

        let api_total = first["total"].as_u64().unwrap_or(0);
        if api_total == 0 {
            if !is_incremental && d % 50 == 0 {
                log_progress(d, total_scopes, item_count, &start);
            }
            continue;
        }

        let limit = api_total.min(OFFSET_CAP);
        let page_0_items = first["items"]
            .as_array()
            .map(|v| v.to_vec())
            .unwrap_or_default();

        let mut cat_max_id = saved_max;
        let mut found_new = false;

        // Write items newer than saved_max
        for item_val in &page_0_items {
            let pid = item_val["catalogCard"]["productId"]
                .as_u64()
                .unwrap_or(0);
            if pid > saved_max {
                let _ = writeln!(writer, "{}", serde_json::to_string(item_val).unwrap());
                item_count += 1;
                found_new = true;
            }
            if pid > cat_max_id {
                cat_max_id = pid;
            }
        }

        // Paginate deeper while still finding new items
        if found_new {
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
                        let mut all_old = true;
                        for item_val in &items {
                            let pid = item_val["catalogCard"]["productId"]
                                .as_u64()
                                .unwrap_or(0);
                            if pid > saved_max {
                                let _ = writeln!(writer, "{}", serde_json::to_string(item_val).unwrap());
                                item_count += 1;
                                all_old = false;
                            }
                            if pid > cat_max_id {
                                cat_max_id = pid;
                            }
                        }
                        if all_old {
                            break;
                        }
                        page += 1;
                    }
                    Err(e) => {
                        if e.contains("too big query offset") {
                            break;
                        }
                        eprintln!("[WARN] scope {cid} offset {offset}: {e}");
                        break;
                    }
                }
            }
        }

        // Update scope state
        if found_new || !is_incremental {
            scopes[idx].max_product_id = cat_max_id;
            scopes[idx].total = api_total;
        }

        if found_new && (d % 50 == 0 || d == total_scopes as u64) {
            log_progress(d, total_scopes, item_count, &start);
            persist(&state_path, &scopes, item_count);
        }
    }

    persist(&state_path, &scopes, item_count);

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
    scopes: &[ScopeState],
) {
    if d % 50 == 0 || d == total as u64 {
        log_progress(d, total, item_count, start);
        persist(state_path, scopes, item_count);
    }
}

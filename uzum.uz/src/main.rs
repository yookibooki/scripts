use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

const GRAPHQL_URL: &str = "https://graphql.uzum.uz/";
const REST_CATEGORIES_URL: &str = "https://api.uzum.uz/api/main/root-categories?eco=false";
const TOKEN_REFRESH_URL: &str = "https://id.uzum.uz/api/auth/token";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const BATCH_SIZE: u64 = 100;
const OFFSET_CAP: u64 = 9900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
struct CategoryId(u64);

impl fmt::Display for CategoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(transparent)]
struct ProductId(u64);

impl fmt::Display for ProductId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug)]
enum UzumError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Http(String),
    Auth(String),
    GraphQl(String),
    State(String),
    Locked(String),
}

impl fmt::Display for UzumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Http(e) => write!(f, "HTTP error: {e}"),
            Self::Auth(e) => write!(f, "auth error: {e}"),
            Self::GraphQl(e) => write!(f, "GraphQL error: {e}"),
            Self::State(e) => write!(f, "state error: {e}"),
            Self::Locked(e) => write!(f, "lock error: {e}"),
        }
    }
}

impl std::error::Error for UzumError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for UzumError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for UzumError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

#[derive(Debug, Clone, Default)]
struct Auth {
    token: Option<String>,
    install_id: Option<String>,
}

impl Auth {
    fn from_env() -> Self {
        Self {
            token: std::env::var("UZUM_ACCESS_TOKEN").ok(),
            install_id: std::env::var("UZUM_INSTALL_ID").ok(),
        }
    }
}

struct UzumClient {
    agent: ureq::Agent,
    auth: Auth,
}

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent()
}

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
    id: CategoryId,
    title: Option<String>,
    children: Option<Vec<CategoryNode>>,
}

#[derive(Debug, Deserialize)]
struct CategoriesResponse {
    payload: Option<Vec<CategoryNode>>,
}

#[derive(Debug, Deserialize)]
struct MakeSearchWrapper {
    #[serde(rename = "makeSearch")]
    make_search: MakeSearchInner,
}

#[derive(Debug, Deserialize)]
struct MakeSearchInner {
    total: u64,
    items: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ScopeState {
    category_id: CategoryId,
    total: u64,
    #[serde(default)]
    max_product_id: ProductId,
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

fn data_root() -> Result<PathBuf, UzumError> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    Ok(PathBuf::from(home).join(".local/share/uzum"))
}

fn refresh_access_token(agent: &ureq::Agent, token: &str) -> Result<String, UzumError> {
    let resp = agent
        .post(TOKEN_REFRESH_URL)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("Origin", "https://uzum.uz")
        .header("Referer", "https://uzum.uz/")
        .header("Accept", "*/*")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send_empty()
        .map_err(|e| UzumError::Http(format!("refresh HTTP: {e}")))?;

    for cookie_val in resp.headers().get_all("set-cookie") {
        let cookie_str = cookie_val
            .to_str()
            .map_err(|e| UzumError::Auth(format!("invalid set-cookie header: {e}")))?;
        for part in cookie_str.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("access_token=") {
                if !val.is_empty() {
                    return Ok(val.to_owned());
                }
            }
        }
    }
    Err(UzumError::Auth(
        "access_token not found in refresh set-cookie".into(),
    ))
}

fn persist_state(path: &Path, scopes: &[ScopeState], item_count: u64) -> Result<(), UzumError> {
    let ts = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%S.000Z")
        .to_string();
    let state = StateFile {
        version: 1,
        scopes: scopes.to_owned(),
        item_count,
        updated_at: ts,
    };
    let json = serde_json::to_string(&state)?;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("state.json");
    let tmp_path = path.with_file_name(format!("{file_name}.tmp"));

    fs::write(&tmp_path, json)?;
    fs::rename(&tmp_path, path)?;
    Ok(())
}

fn fetch_category_tree(client: &mut UzumClient) -> Result<Vec<CategoryNode>, UzumError> {
    for attempt in 0..2 {
        let mut req = client
            .agent
            .get(REST_CATEGORIES_URL)
            .header("Accept", "application/json");

        if let Some(token) = &client.auth.token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        if let Some(iid) = &client.auth.install_id {
            req = req.header("X-Iid", iid.as_str());
        }

        let resp = req
            .call()
            .map_err(|e| UzumError::Http(format!("categories HTTP: {e}")))?;

        if resp.status() == 401 && attempt == 0 {
            if let Some(old) = client.auth.token.clone() {
                match refresh_access_token(&client.agent, &old) {
                    Ok(new) => {
                        client.auth.token = Some(new);
                        eprintln!("[INFO] Token refreshed on 401 (categories)");
                        continue;
                    }
                    Err(e) => eprintln!("[WARN] Token refresh failed: {e}"),
                }
            }
        }

        if resp.status() != 200 {
            return Err(UzumError::Http(format!(
                "categories HTTP {}",
                resp.status()
            )));
        }

        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| UzumError::Http(format!("read categories: {e}")))?;

        let parsed: CategoriesResponse = serde_json::from_str(&text)?;
        return Ok(parsed.payload.unwrap_or_default());
    }
    Err(UzumError::Http("categories failed after 401 retry".into()))
}

fn discover_scopes(
    client: &mut UzumClient,
    nodes: &[CategoryNode],
) -> Result<Vec<ScopeState>, UzumError> {
    let mut scopes = Vec::new();

    for node in nodes {
        let title = node.title.as_deref().unwrap_or("?");
        let total = match search_category(client, node.id, 0, 1) {
            Ok(r) => r.total,
            Err(e) => {
                eprintln!("[WARN] Skipping scope {} ({}): {e}", title, node.id);
                continue;
            }
        };
        if total == 0 {
            continue;
        }

        let has_children = node.children.as_ref().is_some_and(|c| !c.is_empty());

        if total <= OFFSET_CAP || !has_children {
            eprintln!("[INFO] Scope {} ({}): {} products", title, node.id, total);
            scopes.push(ScopeState {
                category_id: node.id,
                total,
                max_product_id: ProductId::default(),
            });
        } else {
            eprintln!(
                "[INFO] Drilling {} ({}): {} products > {} cap",
                title, node.id, total, OFFSET_CAP
            );
            if let Some(children) = &node.children {
                scopes.extend(discover_scopes(client, children)?);
            }
        }
    }
    Ok(scopes)
}

fn graphql_request<T: DeserializeOwned>(
    client: &mut UzumClient,
    query: &str,
    vars: &serde_json::Value,
    op_name: Option<&str>,
) -> Result<T, UzumError> {
    let mut body = serde_json::json!({ "query": query, "variables": vars });
    if let Some(name) = op_name {
        body["operationName"] = serde_json::json!(name);
    }
    let body_str = serde_json::to_string(&body)?;

    for attempt in 0..2 {
        let mut req = client
            .agent
            .post(GRAPHQL_URL)
            .header("Content-Type", "application/json")
            .header("apollographql-client-name", "web-customers")
            .header("apollographql-client-version", "1.63.2")
            .header("city-id", "1");

        if let Some(t) = &client.auth.token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
        if let Some(i) = &client.auth.install_id {
            req = req.header("X-Iid", i.as_str());
        }

        let resp = req
            .send(&body_str)
            .map_err(|e| UzumError::Http(format!("HTTP: {e}")))?;

        if resp.status() == 401 && attempt == 0 {
            if let Some(old) = client.auth.token.clone() {
                match refresh_access_token(&client.agent, &old) {
                    Ok(new_token) => {
                        client.auth.token = Some(new_token);
                        eprintln!("[INFO] Token refreshed on 401");
                        continue;
                    }
                    Err(e) => eprintln!("[WARN] Token refresh on 401 failed: {e}"),
                }
            }
        }

        let text = resp
            .into_body()
            .read_to_string()
            .map_err(|e| UzumError::Http(format!("body: {e}")))?;

        let parsed: GqlResponse<T> =
            serde_json::from_str(&text).map_err(|e| UzumError::Http(format!("JSON: {e}")))?;

        if let Some(errs) = parsed.errors {
            let msg = errs
                .into_iter()
                .map(|e| e.message)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(UzumError::GraphQl(msg));
        }

        return parsed
            .data
            .ok_or_else(|| UzumError::GraphQl("No data".into()));
    }

    Err(UzumError::Http("request failed after 401 retry".into()))
}

fn search_category(
    client: &mut UzumClient,
    category_id: CategoryId,
    offset: u64,
    limit: u64,
) -> Result<MakeSearchInner, UzumError> {
    let query = r#"
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
            "categoryId": category_id.0.to_string(),
            "showAdultContent": "TRUE",
            "filters": [],
            "sort": "BY_DATE_ADDED_DESC",
            "pagination": { "offset": offset, "limit": limit },
            "correctQuery": false,
            "getFastCategories": false,
            "getFastFacets": false,
        }
    });

    let data: MakeSearchWrapper =
        graphql_request(client, query, &vars, Some("MakeSearch_ItemsAndFilters"))?;
    Ok(data.make_search)
}

fn extract_product_id(value: &serde_json::Value) -> Option<ProductId> {
    value
        .get("catalogCard")?
        .get("productId")?
        .as_u64()
        .map(ProductId)
}

fn run() -> Result<(), UzumError> {
    let root = data_root()?;
    fs::create_dir_all(&root)?;

    let lock_path = root.join("uzum.lock");
    let lock_file = File::create(&lock_path)?;
    fs2::FileExt::try_lock_exclusive(&lock_file)
        .map_err(|_| UzumError::Locked("Another instance is already running".into()))?;

    let mut client = UzumClient {
        agent: build_agent(),
        auth: Auth::from_env(),
    };

    if let Some(token) = client.auth.token.clone() {
        match refresh_access_token(&client.agent, &token) {
            Ok(new_token) => {
                client.auth.token = Some(new_token);
                eprintln!("[INFO] Token refreshed on startup");
            }
            Err(e) => eprintln!("[WARN] Startup token refresh failed: {e}"),
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

    let mut scopes: Vec<ScopeState> = if is_incremental && !saved.scopes.is_empty() {
        eprintln!(
            "[INFO] Incremental mode — using {} cached scopes",
            saved.scopes.len()
        );
        saved.scopes
    } else {
        eprintln!("[INFO] Fresh mode — discovering scopes...");
        let tree = fetch_category_tree(&mut client)?;
        if tree.is_empty() {
            return Err(UzumError::State(
                "No categories loaded. Check network or auth.".into(),
            ));
        }
        let discovered = discover_scopes(&mut client, &tree)?;
        eprintln!("[INFO] {} scopes to collect", discovered.len());
        discovered
    };

    let file = if is_incremental {
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(&out_path)?
    } else {
        File::create(&out_path)?
    };
    let mut writer = BufWriter::new(file);

    if !is_incremental {
        let ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S.000Z")
            .to_string();
        let header = serde_json::json!({
            "exportedAt": ts,
            "totalProducts": 0,
            "version": "1.0.0",
            "source": "uzum.uz"
        });
        writeln!(writer, "{}", header)?;
    }

    let total_scopes = scopes.len();

    for idx in 0..scopes.len() {
        let category_id = scopes[idx].category_id;
        let processed = idx as u64 + 1;
        let saved_max = if is_incremental {
            scopes[idx].max_product_id
        } else {
            ProductId::default()
        };

        let first = match search_category(&mut client, category_id, 0, BATCH_SIZE) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[WARN] scope {category_id}: {e}");
                log_progress_if_needed(
                    processed,
                    total_scopes,
                    item_count,
                    &start,
                    &state_path,
                    &scopes,
                )?;
                continue;
            }
        };

        let api_total = first.total;
        if api_total == 0 {
            if !is_incremental && processed % 50 == 0 {
                log_progress(processed, total_scopes, item_count, &start);
            }
            continue;
        }

        let limit = api_total.min(OFFSET_CAP);
        let page_0_items = first.items;

        let mut cat_max_id = saved_max;
        let mut found_new = false;

        for item_val in &page_0_items {
            if let Some(pid) = extract_product_id(item_val) {
                if pid > saved_max {
                    let line = serde_json::to_string(item_val)?;
                    writeln!(writer, "{line}")?;
                    item_count += 1;
                    found_new = true;
                }
                cat_max_id = cat_max_id.max(pid);
            }
        }

        if found_new {
            let mut page = 1u64;
            loop {
                let offset = page * BATCH_SIZE;
                if offset >= limit {
                    break;
                }
                match search_category(&mut client, category_id, offset, BATCH_SIZE) {
                    Ok(r) => {
                        if r.items.is_empty() {
                            break;
                        }
                        let mut all_old = true;
                        for item_val in &r.items {
                            if let Some(pid) = extract_product_id(item_val) {
                                if pid > saved_max {
                                    let line = serde_json::to_string(item_val)?;
                                    writeln!(writer, "{line}")?;
                                    item_count += 1;
                                    all_old = false;
                                }
                                cat_max_id = cat_max_id.max(pid);
                            }
                        }
                        if all_old {
                            break;
                        }
                        page += 1;
                    }
                    Err(e) => {
                        if e.to_string().contains("too big query offset") {
                            break;
                        }
                        eprintln!("[WARN] scope {category_id} offset {offset}: {e}");
                        break;
                    }
                }
            }
        }

        if found_new || !is_incremental {
            let s = &mut scopes[idx];
            s.max_product_id = cat_max_id;
            s.total = api_total;
        }

        if found_new && (processed % 50 == 0 || processed == total_scopes as u64) {
            log_progress(processed, total_scopes, item_count, &start);
            persist_state(&state_path, &scopes, item_count)?;
        }
    }

    persist_state(&state_path, &scopes, item_count)?;
    writer.flush()?;

    let elapsed = start.elapsed();
    eprintln!(
        "[INFO] Done: {item_count} items in {}.{:03}s",
        elapsed.as_secs(),
        elapsed.subsec_millis()
    );
    eprintln!("[INFO] Output: {}", out_path.display());
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[ERROR] {e}");
        std::process::exit(1);
    }
}

fn log_progress(processed: u64, total: usize, item_count: u64, start: &Instant) {
    let el = start.elapsed();
    eprintln!(
        "[INFO] {processed}/{total} — {item_count} items [{}.{:03}s]",
        el.as_secs(),
        el.subsec_millis()
    );
}

fn log_progress_if_needed(
    processed: u64,
    total: usize,
    item_count: u64,
    start: &Instant,
    state_path: &Path,
    scopes: &[ScopeState],
) -> Result<(), UzumError> {
    if processed % 50 == 0 || processed == total as u64 {
        log_progress(processed, total, item_count, start);
        persist_state(state_path, scopes, item_count)?;
    }
    Ok(())
}

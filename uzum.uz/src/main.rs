use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use uzum_watch::*;

// Uzum's GraphQL API returns 48 items per page (max observed).
const PAGE_SIZE: u64 = 48;
// Safety net to avoid infinite loops on misbehaving pagination.
const MAX_OFFSET: u64 = 100_000;
// Delay between requests to stay under the rate limit.
const POLL_DELAY_MS: u64 = 5_000;
// Cooldown when hitting 429 rate limits.
const RATE_LIMIT_COOLDOWN_MS: u64 = 15_000;
// Max log file size before rotation (10 MB).
const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

// ── State ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct State {
    max_id: u64,
    initial_complete: bool,
    known_categories: Vec<u64>,
}

fn state_path() -> String {
    format!("{}/state.json", data_dir().display())
}

fn output_path() -> String {
    format!("{}/uzum_export.jsonl", data_dir().display())
}

fn load_state() -> State {
    fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(State {
            max_id: 0,
            initial_complete: false,
            known_categories: Vec::new(),
        })
}

fn save_state(state: &State) {
    let path = state_path();
    let tmp = format!("{path}.tmp");
    if let Ok(json) = serde_json::to_string_pretty(state) {
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Recursively walk the category tree and collect leaf category IDs.
/// Leaf categories are those with a `productAmount` field.
fn collect_categories(value: &serde_json::Value, categories: &mut Vec<u64>) {
    if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
        if value.get("productAmount").is_some() {
            categories.push(id);
        }
    }
    if let Some(children) = value.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_categories(child, categories);
        }
    }
}

/// Fetch the full category tree from the REST API and return all leaf category IDs.
fn fetch_categories(agent: &ureq::Agent, token: &str) -> Vec<u64> {
    let url = format!("{REST_API}/main/root-categories?eco=false");
    let (resp, _status) = fetch_rest_json(agent, &url, token);
    let Some(resp) = resp else {
        return vec![];
    };

    let mut categories = Vec::new();
    if let Some(payload) = resp.get("payload").and_then(|p| p.as_array()) {
        for cat in payload {
            collect_categories(cat, &mut categories);
        }
    }
    categories
}

/// Fetch one page of products from a category via GraphQL.
/// Returns (items, has_more, status_code).
/// status_code is None on connection errors, Some(429) on rate limit, Some(200) on success.
fn fetch_page(
    agent: &ureq::Agent,
    category_id: u64,
    offset: u64,
    sort: &str,
    token: &str,
) -> (Vec<serde_json::Value>, bool, Option<u16>) {
    let variables = serde_json::json!({
        "queryInput": {
            "categoryId": category_id.to_string(),
            "showAdultContent": "NONE",
            "filters": [],
            "sort": sort,
            "pagination": {
                "offset": offset,
                "limit": PAGE_SIZE
            },
            "correctQuery": false,
            "getFastCategories": true,
            "fastCategoriesLimit": 11,
            "fastCategoriesLevelOffset": 1,
            "getPromotionItems": true,
            "getFastFacets": true,
            "fastFacetsLimit": 10
        }
    });

    let (resp, status) = fetch_graphql(agent, GRAPHQL_QUERY, &variables, token);
    let Some(resp) = resp else {
        return (vec![], false, status);
    };

    // Detect GraphQL-level rate limit errors (429 embedded in errors array).
    if let Some(errors) = resp.get("errors").and_then(|e| e.as_array()) {
        let has_429 = errors.iter().any(|e| {
            e.get("extensions")
                .and_then(|ext| ext.get("http"))
                .and_then(|http| http.get("status"))
                .and_then(|s| s.as_u64())
                == Some(429)
        });
        if has_429 {
            return (vec![], false, Some(429));
        }
        eprintln!("[WARN] GraphQL errors: {:?}", errors);
    }

    let data = resp.get("data").and_then(|d| d.get("makeSearch"));

    let items = data
        .and_then(|m| m.get("items"))
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();

    let total = data
        .and_then(|m| m.get("total"))
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    let has_more = items.len() >= PAGE_SIZE as usize && (offset + PAGE_SIZE) < total;

    (items, has_more, status)
}

/// Extract the fields we care about from a product item and serialize to JSON.
/// The product ID is nested under `catalogCard` in Uzum's response.
fn trim_offer(item: &serde_json::Value, category_id: u64) -> String {
    use serde_json::map::Map;

    let card = item.get("catalogCard").unwrap_or(&serde_json::Value::Null);

    let mut r = Map::new();

    // ID and URL
    if let Some(id) = card.get("id").and_then(|v| v.as_u64()) {
        r.insert("id".to_string(), serde_json::json!(id));
        r.insert(
            "url".to_string(),
            serde_json::json!(format!("https://uzum.uz/product/{id}")),
        );
    }

    // Title
    if let Some(title) = card.get("title").and_then(|v| v.as_str()) {
        r.insert("title".to_string(), serde_json::json!(title));
    }

    // Category
    r.insert("category_id".to_string(), serde_json::json!(category_id));

    // Price: sellPrice is the current price, fullPrice is the original
    if let Some(price_block) = card
        .get("buyingOptions")
        .and_then(|b| b.get("priceBlock"))
    {
        if let Some(amount) = price_block
            .get("sellPrice")
            .and_then(|p| p.get("amount"))
            .and_then(|v| v.as_str())
        {
            r.insert("price".to_string(), serde_json::json!(amount));
        }
        if let Some(amount) = price_block
            .get("fullPrice")
            .and_then(|p| p.get("amount"))
            .and_then(|v| v.as_str())
        {
            r.insert("full_price".to_string(), serde_json::json!(amount));
        }
    }

    // Rating and feedback
    if let Some(rating) = card.get("rating").and_then(|v| v.as_f64()) {
        r.insert("rating".to_string(), serde_json::json!(rating));
    }
    if let Some(feedback) = card.get("feedbackQuantity").and_then(|v| v.as_u64()) {
        r.insert("feedback_quantity".to_string(), serde_json::json!(feedback));
    }

    // Image URL (high-res)
    if let Some(photos) = card.get("photos").and_then(|p| p.as_array()) {
        if let Some(photo) = photos.first() {
            if let Some(link) = photo
                .get("link")
                .and_then(|l| l.get("high"))
                .and_then(|v| v.as_str())
            {
                r.insert("image_url".to_string(), serde_json::json!(link));
            }
        }
    }

    // Delivery info
    if let Some(delivery) = card
        .get("buyingOptions")
        .and_then(|b| b.get("deliveryOptions"))
    {
        if let Some(short_date) = delivery.get("shortDate").and_then(|v| v.as_str()) {
            r.insert("delivery_date".to_string(), serde_json::json!(short_date));
        }
        if let Some(stock_type) = delivery.get("stockType").and_then(|v| v.as_str()) {
            r.insert("stock_type".to_string(), serde_json::json!(stock_type));
        }
    }

    serde_json::to_string(&serde_json::Value::Object(r)).unwrap()
}

fn write_record(out_file: &mut fs::File, line: &str) {
    if let Err(e) = writeln!(out_file, "{line}") {
        eprintln!("[ERROR] Failed to write to export file: {e}");
    }
}

/// Flush the output file to ensure data is persisted to disk.
fn flush_output(out_file: &mut fs::File) {
    if let Err(e) = out_file.flush() {
        eprintln!("[ERROR] Failed to flush export file: {e}");
    }
}

// ── Phase 1: Initial full collection via category tree ─────────────────────

fn phase1_initial_collection(
    agent: &ureq::Agent,
    state: &mut State,
    token: &str,
    shutdown: &AtomicBool,
    total_products: &mut u64,
    log_file: &mut Option<BufWriter<File>>,
) {
    log_to_file(log_file, "[INFO] === Phase 1: Initial full collection ===");

    let out_path = output_path();
    let mut out_file = match fs::File::create(&out_path) {
        Ok(f) => f,
        Err(e) => {
            log_to_file(log_file, &format!("[ERROR] Failed to create {out_path}: {e}"));
            return;
        }
    };

    let mut seen_ids: HashSet<u64> = HashSet::new();

    // ── Fetch category tree from REST API ──
    log_to_file(log_file, "[INFO] Fetching category tree...");
    let categories = fetch_categories(agent, token);
    log_to_file(
        log_file,
        &format!("[INFO] Discovered {} leaf categories", categories.len()),
    );

    // ── Paginate each leaf category via GraphQL ──
    for &cid in &categories {
        if shutdown_requested(shutdown) {
            log_to_file(log_file, "[INFO] Shutdown requested, aborting Phase 1");
            break;
        }

        log_to_file(log_file, &format!("[INFO] Paginating category {cid}..."));
        let mut offset = 0u64;
        loop {
            if shutdown_requested(shutdown) {
                break;
            }

            let (items, has_more, status) = fetch_page(agent, cid, offset, "BY_RELEVANCE_DESC", token);
            // Rate limited — cooldown and retry this page.
            if status == Some(429) {
                log_to_file(log_file, &format!("[WARN] Rate limited on category {cid}, retrying..."));
                std::thread::sleep(Duration::from_millis(RATE_LIMIT_COOLDOWN_MS));
                continue;
            }
            if status == Some(401) || status == Some(403) {
                log_to_file(
                    log_file,
                    &format!(
                        "[WARN] Token appears expired (HTTP {:?}) on category {}. \
                         Set UZUM_TOKEN with a fresh token to continue.",
                        status, cid,
                    ),
                );
                break;
            }
            if items.is_empty() {
                break;
            }
            for item in &items {
                let Some(oid) = extract_id(item) else {
                    continue;
                };
                if !seen_ids.insert(oid) {
                    continue;
                }
                if oid > state.max_id {
                    state.max_id = oid;
                }
                let line = trim_offer(item, cid);
                write_record(&mut out_file, &line);
            }
            flush_output(&mut out_file);
            if !has_more || offset >= MAX_OFFSET {
                break;
            }
            offset += PAGE_SIZE;
            std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
        }
        std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
    }

    // If no categories were discovered,
    // Phase 1 did not complete — likely a 401/auth failure.
    if categories.is_empty() {
        log_to_file(
            log_file,
            "[ERROR] Phase 1 aborted: no categories discovered. \
             Check that UZUM_TOKEN is valid and not expired.",
        );
        return;
    }

    *total_products = seen_ids.len() as u64;
    state.initial_complete = true;
    let mut known = categories;
    known.sort();
    state.known_categories = known;

    log_to_file(
        log_file,
        &format!(
            "[INFO] Phase 1 complete: {} unique posts, max_id = {}",
            seen_ids.len(),
            state.max_id
        ),
    );
}

// ── Phase 2: Ongoing poll for new products ──────────────────────────────────

fn phase2_poll_new(
    agent: &ureq::Agent,
    state: &mut State,
    token: &str,
    shutdown: &AtomicBool,
    log_file: &mut Option<BufWriter<File>>,
) -> u32 {
    let out_path = output_path();
    let mut out_file = match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
    {
        Ok(f) => f,
        Err(e) => {
            log_to_file(log_file, &format!("[ERROR] Failed to open {out_path}: {e}"));
            return 0;
        }
    };

    let mut new_count = 0u32;
    let mut cycle_max = state.max_id;

    // Poll each known category with BY_NEW sort (newest-first).
    for &cid in &state.known_categories {
        if shutdown_requested(shutdown) {
            break;
        }

        let mut offset = 0u64;
        loop {
            if shutdown_requested(shutdown) {
                break;
            }

            let (items, has_more, status) = fetch_page(agent, cid, offset, "BY_NEW", token);
            // Rate limited — cooldown and retry this page.
            if status == Some(429) {
                log_to_file(log_file, &format!("[WARN] Rate limited on category {cid}, retrying..."));
                std::thread::sleep(Duration::from_millis(RATE_LIMIT_COOLDOWN_MS));
                continue;
            }
            if status == Some(401) || status == Some(403) {
                log_to_file(
                    log_file,
                    &format!(
                        "[WARN] Token appears expired (HTTP {:?}) on category {}. \
                         Set UZUM_TOKEN with a fresh token to continue.",
                        status, cid,
                    ),
                );
                break;
            }
            if items.is_empty() {
                break;
            }
            let mut all_old = true;

            for item in &items {
                let Some(oid) = extract_id(item) else {
                    continue;
                };
                if oid <= state.max_id {
                    continue;
                }
                all_old = false;
                cycle_max = cycle_max.max(oid);

                let line = trim_offer(item, cid);
                write_record(&mut out_file, &line);
                new_count += 1;
            }

            // If every product on this page was already known,
            // subsequent pages are even older — stop.
            if all_old || !has_more || offset >= MAX_OFFSET {
                break;
            }
            offset += PAGE_SIZE;
            std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
        }
        std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
    }

    if new_count > 0 {
        state.max_id = cycle_max;
    }

    new_count
}

// ── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let dir = data_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("[ERROR] Failed to create data dir {}: {e}", dir.display());
        std::process::exit(1);
    }

    // ── Lock file (single-instance enforcement) ──
    let _lock = acquire_lock();

    // ── Graceful shutdown handler ──
    let shutdown = install_shutdown_handler();

    // ── Log file with rotation ──
    let mut log_file = open_log_file(MAX_LOG_SIZE);

    let poll_interval: u64 = std::env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let token = match resolve_token() {
        Some(t) => t,
        None => {
            eprintln!("[ERROR] No Uzum token available.");
            eprintln!("[INFO] Set UZUM_TOKEN env var, or log into uzum.uz in Brave/Chrome/Chromium.");
            std::process::exit(1);
        }
    };

    let agent = ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent();

    let mut state = load_state();
    let mut total_products: u64 = 0;
    let mut poll_count: u32 = 0;

    if !state.initial_complete {
        // ── Full initial dump ──
        phase1_initial_collection(&agent, &mut state, &token, &shutdown, &mut total_products, &mut log_file);

        // If Phase 1 found no categories (likely a 401), try refreshing the token.
        if !state.initial_complete && state.known_categories.is_empty() {
            log_to_file(
                &mut log_file,
                "[INFO] Phase 1 produced no categories — attempting token refresh...",
            );
            if let Some(new_token) = refresh_token() {
                log_to_file(&mut log_file, "[INFO] Token refreshed, retrying Phase 1...");
                phase1_initial_collection(&agent, &mut state, &new_token, &shutdown, &mut total_products, &mut log_file);
            }
        }

        save_state(&state);
    }

    // ── Ongoing poll (single cycle, or loop if POLL_INTERVAL is set) ──
    if poll_interval > 0 {
        // Daemon mode: loop forever
        log_to_file(
            &mut log_file,
            &format!("[INFO] Daemon mode started (poll interval = {poll_interval}ms)"),
        );

        loop {
            if shutdown_requested(&shutdown) {
                log_to_file(
                    &mut log_file,
                    "[INFO] Shutdown requested, exiting gracefully",
                );
                break;
            }

            let n = phase2_poll_new(&agent, &mut state, &token, &shutdown, &mut log_file);
            poll_count += 1;

            if n > 0 {
                save_state(&state);
                total_products += n as u64;
            }

            log_to_file(
                &mut log_file,
                &format!(
                    "[INFO] Poll #{poll_count}: {n} new posts (max_id = {}, total = {total_products})",
                    state.max_id
                ),
            );

            // Write health report after each poll
            write_health(&HealthReport {
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                status: "ok".to_string(),
                phase: "polling".to_string(),
                max_id: state.max_id,
                total_products,
                poll_count,
                new_since_last_poll: n,
            });

            std::thread::sleep(Duration::from_millis(poll_interval));
        }
    } else {
        // Oneshot mode: single poll cycle
        let n = phase2_poll_new(&agent, &mut state, &token, &shutdown, &mut log_file);
        if n > 0 {
            save_state(&state);
            total_products += n as u64;
        }
        log_to_file(
            &mut log_file,
            &format!("[INFO] Poll: {n} new posts (max_id = {})", state.max_id),
        );
    }

    // Final health report
    write_health(&HealthReport {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        status: "shutdown".to_string(),
        phase: "stopped".to_string(),
        max_id: state.max_id,
        total_products,
        poll_count,
        new_since_last_poll: 0,
    });
}

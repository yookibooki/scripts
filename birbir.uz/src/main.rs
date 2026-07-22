use birbir_watch::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::time::Duration;

const PAGE_SIZE: u64 = 40;
const MAX_PAGE: u64 = 10000; // safety upper bound
const POLL_DELAY_MS: u64 = 100;

// ── State ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct State {
    max_id: u64,
    initial_complete: bool,
}

fn state_path() -> String {
    format!("{}/state.json", data_dir().display())
}

fn output_path() -> String {
    format!("{}/birbir_export.jsonl", data_dir().display())
}

fn load_state() -> State {
    fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(State {
            max_id: 0,
            initial_complete: false,
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

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Extract the category path from a webUri.
/// Example: "uz/toshkent/cat/telefonlar/smartfonlar/o/iphone-17-pro-270391997"
///   → "telefonlar/smartfonlar"
fn extract_category_path(web_uri: &str) -> String {
    if let Some(cat_start) = web_uri.find("/cat/") {
        let after_cat = &web_uri[cat_start + 5..];
        if let Some(o_end) = after_cat.find("/o/") {
            return after_cat[..o_end].to_string();
        }
    }
    String::new()
}

fn format_record(offer: &serde_json::Value, oid: u64) -> String {
    let web_uri = offer
        .get("webUri")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let url = if web_uri.is_empty() {
        String::new()
    } else {
        format!("https://birbir.uz/{web_uri}")
    };

    let title = offer
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let price = offer
        .get("price")
        .and_then(|p| p.get("value"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let currency = offer
        .get("price")
        .and_then(|p| p.get("currency"))
        .and_then(|v| v.as_str())
        .unwrap_or("UZS");

    let city = offer
        .get("region")
        .and_then(|r| r.get("title"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let published_at = offer
        .get("publishedAt")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let category_path = extract_category_path(web_uri);

    let record = serde_json::json!({
        "id": oid,
        "url": url,
        "title": title,
        "price": price,
        "currency": currency,
        "city": city,
        "published_at": published_at,
        "category_path": category_path,
    });
    serde_json::to_string(&record).unwrap()
}

fn write_record(out_file: &mut fs::File, line: &str) {
    if let Err(e) = writeln!(out_file, "{line}") {
        eprintln!("[ERROR] Failed to write to export file: {e}");
    }
}

// ── Pagination ──────────────────────────────────────────────────────────────

/// Fetch one page of offers from the feed.
/// Returns (offers, has_more).
fn fetch_page(
    agent: &ureq::Agent,
    token: &str,
    page: u64,
) -> (Vec<serde_json::Value>, bool) {
    let url = format!("{API}/offer/feed");
    let body = serde_json::json!({
        "page": page,
        "perPage": PAGE_SIZE,
    });

    let raw = match post_json(agent, &url, &body, token) {
        Some(v) => v,
        None => return (vec![], false),
    };

    let parsed: FeedResponse = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] Parse error: {e}");
            return (vec![], false);
        }
    };

    let content = match parsed.content {
        Some(c) => c,
        None => return (vec![], false),
    };

    let offers = content.items.unwrap_or_default();
    let has_more = content
        .paginator
        .map(|p| p.next_page_exists)
        .unwrap_or(false);

    (offers, has_more)
}

// ── Token management ────────────────────────────────────────────────────────

/// Fetch a fresh auth token or exit.
fn obtain_token() -> String {
    match extract_token() {
        Some(t) => {
            eprintln!("[INFO] Auth token obtained (len={})", t.len());
            t
        }
        None => {
            eprintln!("[ERROR] Failed to obtain auth token. Exiting.");
            std::process::exit(1);
        }
    }
}

// ── Phase 1: Initial full collection ────────────────────────────────────────

fn phase1_initial_collection(agent: &ureq::Agent, state: &mut State) {
    eprintln!("[INFO] === Phase 1: Initial full collection ===");

    let token = obtain_token();

    let out_path = output_path();
    let mut out_file = match fs::File::create(&out_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[ERROR] Failed to create {out_path}: {e}");
            return;
        }
    };

    let mut seen_ids: std::collections::HashSet<u64> = std::collections::HashSet::new();

    let mut page = 1u64;
    loop {
        eprintln!("[INFO] Fetching page {page}...");
        let (offers, has_more) = fetch_page(agent, &token, page);
        if offers.is_empty() {
            eprintln!("[INFO] No offers on page {page}, done.");
            break;
        }

        for offer in &offers {
            let Some(oid) = extract_id(offer) else {
                continue;
            };
            if !seen_ids.insert(oid) {
                continue;
            }
            if oid > state.max_id {
                state.max_id = oid;
            }
            let line = format_record(offer, oid);
            write_record(&mut out_file, &line);
        }

        if !has_more || page >= MAX_PAGE {
            eprintln!(
                "[INFO] Reached page limit or no more pages (page={page}, has_more={has_more})"
            );
            break;
        }
        page += 1;
        std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
    }

    state.initial_complete = true;

    eprintln!(
        "[INFO] Phase 1 complete: {} unique posts, max_id = {}",
        seen_ids.len(),
        state.max_id
    );
}

// ── Phase 2: Ongoing poll for new posts ─────────────────────────────────────

fn phase2_poll_new(agent: &ureq::Agent, state: &mut State) -> u32 {
    let t0 = std::time::Instant::now();
    let token = obtain_token();
    eprintln!("[TIMING] obtain_token: {:?}", t0.elapsed());

    let out_path = output_path();
    let mut out_file = match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
    {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[ERROR] Failed to open {out_path}: {e}");
            return 0;
        }
    };

    let mut new_count = 0u32;
    let mut page = 1u64;

    loop {
        let t1 = std::time::Instant::now();
        let (offers, has_more) = fetch_page(agent, &token, page);
        eprintln!("[TIMING] fetch_page page={page}: {:?}", t1.elapsed());
        if offers.is_empty() {
            break;
        }

        let mut all_old = true;

        for offer in &offers {
            let Some(oid) = extract_id(offer) else {
                continue;
            };
            if oid <= state.max_id {
                continue;
            }
            all_old = false;
            state.max_id = oid;

            let line = format_record(offer, oid);
            write_record(&mut out_file, &line);
            new_count += 1;
        }

        // If every post on this page was already known,
        // subsequent pages are even older — stop.
        if all_old || !has_more || page >= MAX_PAGE {
            eprintln!("[TIMING] stopping: all_old={all_old} has_more={has_more} page={page}");
            break;
        }
        page += 1;
        std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
    }

    eprintln!("[TIMING] poll total: {:?}", t0.elapsed());
    new_count
}

// ── Main ────────────────────────────────────────────────────────────────────

fn main() {
    let dir = data_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("[ERROR] Failed to create data dir {}: {e}", dir.display());
        std::process::exit(1);
    }

    let poll_interval: u64 = std::env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let agent = ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .build()
        .new_agent();

    let mut state = load_state();

    if !state.initial_complete {
        // ── Full initial dump ──
        phase1_initial_collection(&agent, &mut state);
        save_state(&state);
        eprintln!("[INFO] Initial collection done. Exiting.");
        return;
    }

    // ── Ongoing poll (single cycle, or loop if POLL_INTERVAL is set) ──
    if poll_interval > 0 {
        // Daemon mode: loop forever
        eprintln!("[INFO] Daemon mode started (poll interval = {poll_interval}ms)");
        loop {
            let n = phase2_poll_new(&agent, &mut state);
            if n > 0 {
                save_state(&state);
            }
            eprintln!("[INFO] Poll: {n} new posts (max_id = {})", state.max_id);
            std::thread::sleep(Duration::from_millis(poll_interval));
        }
    } else {
        // Oneshot mode: single poll cycle
        let n = phase2_poll_new(&agent, &mut state);
        if n > 0 {
            save_state(&state);
        }
        eprintln!("[INFO] Poll: {n} new posts (max_id = {})", state.max_id);
    }
}

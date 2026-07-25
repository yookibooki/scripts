use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

pub const API: &str = "https://www.olx.uz/api/v1/offers";
pub const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
pub struct ApiResponse {
    pub data: Option<Vec<serde_json::Value>>,
}

/// Returns ~/.local/share/olx, creating a cross-platform PathBuf.
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/olx")
}

/// Fetch and deserialize the JSON body from a URL.
/// Retries on transient HTTP errors (which the OLX CDN returns intermittently).
pub fn fetch_json(agent: &ureq::Agent, url: &str) -> Option<serde_json::Value> {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }

        let resp = match agent.get(url).header("Accept", "application/json").call() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] HTTP request failed: {e}");
                continue;
            }
        };

        let status = resp.status();
        if status == 200 {
            let text = match resp.into_body().read_to_string() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read response body: {e}");
                    continue;
                }
            };
            return match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("[ERROR] JSON parse error: {e}");
                    None
                }
            };
        }

        let text = resp.into_body().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() {
            "(empty)"
        } else {
            &text[..text.len().min(200)]
        };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    None
}

/// Extract the numeric ID from an offer.
pub fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

// OLX API contracts (external behaviors, not internal design choices):
// - The API rejects limit values > 50 but always returns ~52 items per page.
//   PAGE_SIZE is set to 50 (the max the API allows) and used as the offset step
//   to advance one full page at a time.
// - The API enforces a hard cap: queries beyond offset=1000 return empty results.
//   Each category is paginated independently so each gets its own 1000-offset budget.
const PAGE_SIZE: u64 = 50;
const MAX_OFFSET: u64 = 1000;
const POLL_DELAY_MS: u64 = 100;

// ── Lock file ──────────────────────────────────────────────────────────────

/// Acquire an exclusive lock on the data directory.
/// Exits immediately if another instance already holds the lock.
fn acquire_lock() -> fs::File {
    let dir = data_dir();
    let path = dir.join("olx.lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(&path)
        .unwrap_or_else(|e| {
            eprintln!("[ERROR] Failed to open lock file {}: {e}", path.display());
            std::process::exit(1);
        });
    if let Err(e) = file.try_lock_exclusive() {
        eprintln!("[ERROR] Another instance is already running (lock: {e}). Exiting.");
        std::process::exit(1);
    }
    file
}

// ── State ─────────────────────────────────────────────────────────────────

fn default_version() -> u64 {
    1
}

#[derive(Serialize, Deserialize)]
struct State {
    #[serde(default = "default_version")]
    version: u64,
    max_id: u64,
    initial_complete: bool,
    known_categories: Vec<u64>,
}

fn state_path() -> String {
    format!("{}/state.json", data_dir().display())
}

fn output_path() -> String {
    format!("{}/olx_export.jsonl", data_dir().display())
}

fn load_state() -> State {
    fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(State {
            version: 1,
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

// ── Helpers ─────────────────────────────────────────────────────────────────

fn extract_category_id(offer: &serde_json::Value) -> Option<u64> {
    offer
        .get("category")
        .and_then(|c| c.get("id"))
        .and_then(|v| v.as_u64())
}

fn trim_offer(offer: &serde_json::Value) -> String {
    use serde_json::map::Map;

    let mut r = Map::new();

    // Top-level keepers
    for key in &[
        "id",
        "url",
        "title",
        "business",
        "created_time",
        "last_refresh_time",
    ] {
        if let Some(v) = offer.get(*key) {
            r.insert(key.to_string(), v.clone());
        }
    }

    // Price: only converted_value in UZS
    if let Some(params) = offer.get("params").and_then(|v| v.as_array()) {
        for p in params {
            if p.get("key").and_then(|v| v.as_str()) == Some("price") {
                if let Some(cv) = p
                    .get("value")
                    .and_then(|v| v.get("converted_value"))
                    .and_then(|v| v.as_f64())
                {
                    r.insert("price_uzs".to_string(), serde_json::json!(cv as u64));
                }
                break;
            }
        }
    }

    // Category: only type
    if let Some(cat) = offer.get("category") {
        if let Some(typ) = cat.get("type").and_then(|v| v.as_str()) {
            r.insert("category_type".to_string(), serde_json::json!(typ));
        }
    }

    // Location: flat name fields
    if let Some(loc) = offer.get("location") {
        for (flat, src) in &[
            ("location_city", "city"),
            ("location_district", "district"),
            ("location_region", "region"),
        ] {
            if let Some(v) = loc.get(*src) {
                if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                    r.insert(flat.to_string(), serde_json::json!(name));
                }
            }
        }
    }

    // Map: flat coordinates array [lon, lat]
    if let Some(m) = offer.get("map") {
        let lat = m.get("lat").and_then(|v| v.as_f64());
        let lon = m.get("lon").and_then(|v| v.as_f64());
        if let (Some(lat), Some(lon)) = (lat, lon) {
            r.insert("coordinates".to_string(), serde_json::json!([lon, lat]));
        }
    }

    serde_json::to_string(&serde_json::Value::Object(r)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_load_missing() {
        let state = serde_json::from_str::<State>("").unwrap_or(State {
            version: 1,
            max_id: 0,
            initial_complete: false,
            known_categories: Vec::new(),
        });
        assert_eq!(state.version, 1);
        assert_eq!(state.max_id, 0);
        assert!(!state.initial_complete);
        assert!(state.known_categories.is_empty());
    }

    #[test]
    fn test_state_load_legacy_no_version() {
        let json = r#"{"max_id": 42, "initial_complete": true, "known_categories": [1, 2, 3]}"#;
        let state: State = serde_json::from_str(json).unwrap();
        assert_eq!(state.version, 1);
        assert_eq!(state.max_id, 42);
        assert!(state.initial_complete);
        assert_eq!(state.known_categories, vec![1, 2, 3]);
    }

    #[test]
    fn test_state_load_current_with_version() {
        let json =
            r#"{"version": 1, "max_id": 99, "initial_complete": false, "known_categories": []}"#;
        let state: State = serde_json::from_str(json).unwrap();
        assert_eq!(state.version, 1);
        assert_eq!(state.max_id, 99);
        assert!(!state.initial_complete);
    }

    #[test]
    fn test_state_load_corrupted() {
        let result = serde_json::from_str::<State>("{broken json}");
        assert!(result.is_err());
    }
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

// ── Pagination ──────────────────────────────────────────────────────────────

/// Fetch one page of offers, optionally scoped to a category.
/// Returns (offers, has_more).
fn fetch_page(
    agent: &ureq::Agent,
    category_id: Option<u64>,
    offset: u64,
) -> (Vec<serde_json::Value>, bool) {
    let url = match category_id {
        Some(cid) => format!(
            "{API}/?offset={offset}&limit={PAGE_SIZE}&category_id={cid}&sort_by=created_at:desc"
        ),
        None => format!("{API}/?offset={offset}&limit={PAGE_SIZE}&sort_by=created_at:desc"),
    };

    let offers: Vec<serde_json::Value> = match fetch_json(agent, &url) {
        Some(v) => match serde_json::from_value::<ApiResponse>(v) {
            Ok(r) => r.data.unwrap_or_default(),
            Err(e) => {
                eprintln!("[ERROR] Parse error: {e}");
                return (vec![], false);
            }
        },
        None => return (vec![], false),
    };

    let has_more = offers.len() >= PAGE_SIZE as usize;
    (offers, has_more)
}

// ── Phase 1: Initial full collection via BFS over categories ────────────────

fn phase1_initial_collection(agent: &ureq::Agent, state: &mut State) {
    eprintln!("[INFO] === Phase 1: Initial full collection ===");

    let out_path = output_path();
    let mut out_file = match fs::File::create(&out_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[ERROR] Failed to create {out_path}: {e}");
            return;
        }
    };

    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut all_known_cats: HashSet<u64> = HashSet::new();

    // ── Round 0: default listing (seed categories) ──
    eprintln!("[INFO] Paginating default listing...");
    let mut offset = 0u64;
    loop {
        let (offers, has_more) = fetch_page(agent, None, offset);
        if offers.is_empty() {
            break;
        }
        for offer in &offers {
            let Some(oid) = extract_id(offer) else {
                continue;
            };
            if !seen_ids.insert(oid) {
                continue;
            }
            if let Some(cid) = extract_category_id(offer) {
                all_known_cats.insert(cid);
            }
            if oid > state.max_id {
                state.max_id = oid;
            }
            let line = trim_offer(offer);
            write_record(&mut out_file, &line);
        }
        flush_output(&mut out_file);
        if !has_more {
            break;
        }
        offset += PAGE_SIZE;
        std::thread::sleep(Duration::from_millis(POLL_DELAY_MS));
    }

    eprintln!(
        "[INFO] Discovered {} categories, max_id = {}",
        all_known_cats.len(),
        state.max_id
    );

    // ── BFS: paginate each discovered category ──
    let mut queue: VecDeque<u64> = all_known_cats.iter().copied().collect();
    while let Some(cid) = queue.pop_front() {
        eprintln!("[INFO] Paginating category {cid}...");
        let mut offset = 0u64;
        loop {
            let (offers, has_more) = fetch_page(agent, Some(cid), offset);
            if offers.is_empty() {
                break;
            }
            for offer in &offers {
                let Some(oid) = extract_id(offer) else {
                    continue;
                };
                if !seen_ids.insert(oid) {
                    continue;
                }
                if let Some(new_cid) = extract_category_id(offer) {
                    if all_known_cats.insert(new_cid) {
                        eprintln!("[INFO] Discovered new category {new_cid}");
                        queue.push_back(new_cid);
                    }
                }
                if oid > state.max_id {
                    state.max_id = oid;
                }
                let line = trim_offer(offer);
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

    state.initial_complete = true;
    state.known_categories = {
        let mut v: Vec<u64> = all_known_cats.into_iter().collect();
        v.sort();
        v
    };

    eprintln!(
        "[INFO] Phase 1 complete: {} unique posts, max_id = {}",
        seen_ids.len(),
        state.max_id
    );
}

// ── Phase 2: Ongoing poll for new posts ─────────────────────────────────────

fn phase2_poll_new(agent: &ureq::Agent, state: &mut State) -> u32 {
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
    let mut offset = 0u64;
    let mut cycle_max = state.max_id;

    loop {
        let (offers, has_more) = fetch_page(agent, None, offset);
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
            cycle_max = cycle_max.max(oid);

            let line = trim_offer(offer);
            write_record(&mut out_file, &line);
            new_count += 1;
        }

        // If every post on this page was already known,
        // subsequent pages are even older — stop.
        if all_old || !has_more || offset >= MAX_OFFSET {
            break;
        }
        offset += PAGE_SIZE;
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

    let _lock = acquire_lock();

    let poll_interval: u64 = std::env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    eprintln!("[INFO] Data directory: {}", dir.display());
    if poll_interval > 0 {
        eprintln!("[INFO] Mode: daemon (poll interval = {poll_interval}ms)");
    } else {
        eprintln!("[INFO] Mode: oneshot");
    }

    let agent = ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent();

    let mut state = load_state();
    eprintln!("[INFO] State version: {}", state.version);

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
            save_state(&state);
            eprintln!("[INFO] Poll: {n} new posts (max_id = {})", state.max_id);
            std::thread::sleep(Duration::from_millis(poll_interval));
        }
    } else {
        // Oneshot mode: single poll cycle
        let n = phase2_poll_new(&agent, &mut state);
        save_state(&state);
        eprintln!("[INFO] Poll: {n} new posts (max_id = {})", state.max_id);
    }
}

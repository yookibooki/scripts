use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

const API: &str = "https://www.olx.uz/api/v1/offers";
const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const PAGE_SIZE: u64 = 50;
const MAX_OFFSET: u64 = 1000;
const POLL_DELAY: Duration = Duration::from_millis(100);

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/olx")
}

// ── HTTP helpers (local — OLX uses public GET with no auth) ─────────

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent()
}

fn fetch_json(agent: &ureq::Agent, url: &str) -> Option<serde_json::Value> {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(500 * attempt));
        }
        let resp = match agent
            .get(url)
            .header("Accept", "application/json")
            .call()
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] HTTP GET failed: {e}");
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
            return serde_json::from_str(&text).ok();
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

// ── Lock ────────────────────────────────────────────────────────────

fn acquire_lock() -> fs::File {
    let path = data_dir().join("olx.lock");
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

// ── State ───────────────────────────────────────────────────────────

fn default_version() -> u64 {
    1
}

#[derive(Debug, Serialize, Deserialize)]
struct State {
    #[serde(default = "default_version")]
    version: u64,
    max_id: u64,
    initial_complete: bool,
    known_categories: Vec<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            max_id: 0,
            initial_complete: false,
            known_categories: Vec::new(),
        }
    }
}

fn state_path() -> PathBuf {
    data_dir().join("state.json")
}

fn load_state() -> State {
    fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(state: &State) {
    let path = state_path();
    let tmp = path.with_extension("tmp");
    if let Ok(json) = serde_json::to_string_pretty(state) {
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

// ── IO ──────────────────────────────────────────────────────────────

fn write_record(file: &mut impl Write, line: &str) {
    if let Err(e) = writeln!(file, "{line}") {
        eprintln!("[ERROR] Failed to write record: {e}");
    }
}

fn flush_output(file: &mut impl Write) {
    if let Err(e) = file.flush() {
        eprintln!("[ERROR] Failed to flush: {e}");
    }
}

// ── API helpers ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: Option<Vec<serde_json::Value>>,
}

fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

fn extract_category_id(offer: &serde_json::Value) -> Option<u64> {
    offer
        .get("category")
        .and_then(|c| c.get("id"))
        .and_then(|v| v.as_u64())
}

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

// ── Phase 1: Initial full collection via BFS over categories ────────

fn phase1_initial_collection(agent: &ureq::Agent, state: &mut State) {
    eprintln!("[INFO] === Phase 1: Initial full collection ===");

    let out_path = data_dir().join("olx_export.jsonl");
    let mut out_file =
        fs::File::create(&out_path).unwrap_or_else(|e| panic!("Failed to create {}: {e}", out_path.display()));

    let mut seen_ids: HashSet<u64> = HashSet::new();
    let mut all_known_cats: HashSet<u64> = HashSet::new();

    // Round 0: default listing (seed categories)
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
            write_record(&mut out_file, &serde_json::to_string(offer).unwrap());
        }
        flush_output(&mut out_file);
        if !has_more {
            break;
        }
        offset += PAGE_SIZE;
        std::thread::sleep(POLL_DELAY);
    }

    eprintln!(
        "[INFO] Discovered {} categories, max_id = {}",
        all_known_cats.len(),
        state.max_id
    );

    // BFS: paginate each discovered category
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
                state.max_id = state.max_id.max(oid);
                write_record(&mut out_file, &serde_json::to_string(offer).unwrap());
            }
            flush_output(&mut out_file);
            if !has_more || offset >= MAX_OFFSET {
                break;
            }
            offset += PAGE_SIZE;
            std::thread::sleep(POLL_DELAY);
        }
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

// ── Phase 2: Poll for new posts ─────────────────────────────────────

fn phase2_poll_new(agent: &ureq::Agent, state: &mut State) -> u32 {
    let out_path = data_dir().join("olx_export.jsonl");
    let mut out_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
        .expect("Failed to open output file");

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
            write_record(&mut out_file, &serde_json::to_string(offer).unwrap());
            new_count += 1;
        }

        if all_old || !has_more || offset >= MAX_OFFSET {
            break;
        }
        offset += PAGE_SIZE;
        std::thread::sleep(POLL_DELAY);
    }

    if new_count > 0 {
        state.max_id = cycle_max;
    }
    new_count
}

// ── Main ─────────────────────────────────────────────────────────────

fn main() {
    let dir = data_dir();
    fs::create_dir_all(&dir).expect("Failed to create data dir");
    let _lock = acquire_lock();

    let poll_interval: u64 = std::env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let agent = build_agent();
    let mut state = load_state();

    if !state.initial_complete {
        phase1_initial_collection(&agent, &mut state);
        save_state(&state);
        eprintln!("[INFO] Initial collection done. Exiting.");
        return;
    }

    if poll_interval > 0 {
        eprintln!("[INFO] Daemon mode (poll interval = {poll_interval}ms)");
        loop {
            let n = phase2_poll_new(&agent, &mut state);
            save_state(&state);
            eprintln!("[INFO] Poll: {n} new posts (max_id = {})", state.max_id);
            std::thread::sleep(Duration::from_millis(poll_interval));
        }
    } else {
        let n = phase2_poll_new(&agent, &mut state);
        save_state(&state);
        eprintln!("[INFO] Poll: {n} new posts (max_id = {})", state.max_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_id() {
        let offer = serde_json::json!({"id": 12345});
        assert_eq!(extract_id(&offer), Some(12345));
    }

    #[test]
    fn test_extract_id_missing() {
        assert_eq!(extract_id(&serde_json::json!({})), None);
    }

    #[test]
    fn test_extract_category_id() {
        let offer = serde_json::json!({"category": {"id": 42}});
        assert_eq!(extract_category_id(&offer), Some(42));
    }

    #[test]
    fn test_state_default() {
        let state = State::default();
        assert_eq!(state.version, 1);
        assert_eq!(state.max_id, 0);
        assert!(!state.initial_complete);
        assert!(state.known_categories.is_empty());
    }
}

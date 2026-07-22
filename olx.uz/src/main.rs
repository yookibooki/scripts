use olx_watch::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::time::Duration;

// Note: the API rejects limit values > 50 but always returns 52 items per page.
// PAGE_SIZE is set to 50 (the max the API allows) and used as the offset step
// to advance one full page at a time.
const PAGE_SIZE: u64 = 50;
const MAX_OFFSET: u64 = 1000;
const POLL_DELAY_MS: u64 = 100;

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
    format!("{}/olx_export.jsonl", data_dir().display())
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
    for key in &["id", "url", "title", "business", "created_time", "last_refresh_time"] {
        if let Some(v) = offer.get(*key) {
            r.insert(key.to_string(), v.clone());
        }
    }

    // Price: only converted_value in UZS
    if let Some(params) = offer.get("params").and_then(|v| v.as_array()) {
        for p in params {
            if p.get("key").and_then(|v| v.as_str()) == Some("price") {
                if let Some(cv) = p.get("value").and_then(|v| v.get("converted_value")).and_then(|v| v.as_f64()) {
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
        for (flat, src) in &[("location_city", "city"), ("location_district", "district"), ("location_region", "region")] {
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
        Some(cid) => format!("{API}/?offset={offset}&limit={PAGE_SIZE}&category_id={cid}"),
        None => format!("{API}/?offset={offset}&limit={PAGE_SIZE}"),
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

    let poll_interval: u64 = std::env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let agent = ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(30)))
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
        eprintln!(
            "[INFO] Daemon mode started (poll interval = {poll_interval}ms)"
        );
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

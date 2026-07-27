use base64::Engine as _;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const API: &str = "https://api.birbir.uz/api/frontoffice/1.3.5.0";
const ORIGIN: &str = "https://birbir.uz";
const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const PAGE_SIZE: u64 = 40;
const MAX_PAGE: u64 = 10000;
const POLL_DELAY: Duration = Duration::from_millis(100);

fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/birbir")
}

// ── HTTP helpers (local — birbir uses Bearer auth + custom headers) ──

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .new_agent()
}

// ── Lock ────────────────────────────────────────────────────────────

fn acquire_lock() -> fs::File {
    let path = data_dir().join("birbir.lock");
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
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            max_id: 0,
            initial_complete: false,
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

// ── API types ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FeedResponse {
    content: Option<FeedContent>,
}

#[derive(Debug, Deserialize)]
struct FeedContent {
    items: Option<Vec<serde_json::Value>>,
    paginator: Option<Paginator>,
}

#[derive(Debug, Deserialize)]
struct Paginator {
    #[serde(rename = "nextPageExists")]
    next_page_exists: bool,
}

// ── Auth ────────────────────────────────────────────────────────────

/// ponytail: duplicated JWT expiry check is fine — birbir and uzum
/// decode JWT payloads differently (birbir uses agent-browser, uzum
/// uses env vars), and the check itself is 12 stable lines.
fn is_token_expired(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return true;
    }
    let bytes = match base64::engine::general_purpose::STANDARD.decode(parts[1]) {
        Ok(b) => b,
        Err(_) => return true,
    };
    let json_str = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return true,
    };
    let val: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(_) => return true,
    };
    let exp = val.get("exp").and_then(|v| v.as_u64()).unwrap_or(0);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    exp <= now + 60
}

/// Parse a session cookie value and extract the Bearer access token.
/// Handles both raw (j:JSON) and URL-encoded (j%3A...) formats.
fn parse_session_token(raw: &str) -> Option<String> {
    let decoded = if raw.starts_with("j%3A") || raw.contains('%') {
        urlencoding::decode(raw).ok()?.to_string()
    } else {
        raw.to_string()
    };
    let without_prefix = decoded.strip_prefix("j:")?;
    let data: serde_json::Value = serde_json::from_str(without_prefix).ok()?;
    data.get("accessToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Scan raw HTTP response headers for Set-Cookie: session=...
fn scan_headers_for_token(headers: &str) -> Option<String> {
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if !lower.trim().starts_with("set-cookie:") {
            continue;
        }
        let rest = line.split_once(':').map(|(_, v)| v).unwrap_or(line).trim();
        let Some(cookie_val) = rest.strip_prefix("session=") else {
            continue;
        };
        let end = cookie_val.find(';').unwrap_or(cookie_val.len());
        let val = &cookie_val[..end];
        if !val.starts_with("j%") && !val.starts_with("j:") {
            continue;
        }
        return parse_session_token(val);
    }
    None
}

/// Try to fetch the session via agent-browser CLI.
fn fetch_token_via_agent_browser() -> Option<String> {
    let output = std::process::Command::new("agent-browser")
        .args(["cookies", "get", "--domain", "birbir.uz", "--json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let cookies: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).ok()?;
    for cookie in &cookies {
        let name = cookie.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.eq_ignore_ascii_case("session") {
            if let Some(val) = cookie.get("value").and_then(|v| v.as_str()) {
                if let Some(token) = parse_session_token(val) {
                    return Some(token);
                }
            }
        }
    }
    None
}

/// Try to fetch the session via direct HTTP curl (bypasses Cloudflare sometimes).
fn fetch_token_via_curl() -> Option<String> {
    for attempt in 0..5 {
        if attempt > 0 {
            eprintln!(
                "[INFO] Retrying direct HTTP session fetch (attempt {})...",
                attempt + 1
            );
            std::thread::sleep(Duration::from_millis(1000 * attempt));
        }
        let output = std::process::Command::new("curl")
            .args([
                "-s",
                "-L",
                "-A",
                USER_AGENT,
                "-H",
                "Accept-Language: uz,ru;q=0.9,en;q=0.8",
                "-D",
                "-",
                "-o",
                "/dev/null",
                "https://birbir.uz/",
            ])
            .output()
            .ok()?;
        let headers = String::from_utf8_lossy(&output.stdout);
        if let Some(token) = scan_headers_for_token(&headers) {
            return Some(token);
        }
    }
    None
}

fn cache_token(path: &PathBuf, token: &str) {
    fs::create_dir_all(data_dir()).ok();
    let tmp = path.with_extension("tmp");
    if fs::write(&tmp, token).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

/// Get auth token through layered fallbacks: cached → agent-browser → curl → stale.
/// Deliberately defensive — Cloudflare interruptions and token expiry are expected.
fn get_token() -> String {
    let path = data_dir().join("token.txt");

    // 1. Cached token (fast path)
    if let Ok(token) = fs::read_to_string(&path) {
        let token = token.trim().to_string();
        if !token.is_empty() && !is_token_expired(&token) {
            eprintln!("[INFO] Using cached auth token");
            return token;
        }
    }

    // 2. agent-browser (real Chrome, handles Cloudflare JS challenge)
    eprintln!("[INFO] Fetching auth token via agent-browser...");
    if let Some(token) = fetch_token_via_agent_browser() {
        cache_token(&path, &token);
        return token;
    }

    // 3. Direct HTTP via curl (bypasses Cloudflare for some routes)
    eprintln!("[INFO] Trying direct HTTP session fetch...");
    if let Some(token) = fetch_token_via_curl() {
        cache_token(&path, &token);
        return token;
    }

    // 4. Stale fallback — use cached token even if expired
    if let Ok(stale) = fs::read_to_string(&path) {
        let stale = stale.trim().to_string();
        if !stale.is_empty() {
            eprintln!("[WARN] All auth methods failed; reusing stale cache as degraded path");
            return stale;
        }
    }

    eprintln!("[ERROR] Failed to obtain auth token. Exiting.");
    std::process::exit(1);
}

// ── API ─────────────────────────────────────────────────────────────

fn fetch_page(
    agent: &ureq::Agent,
    token: &str,
    page: u64,
) -> (Vec<serde_json::Value>, bool) {
    let body = serde_json::json!({
        "page": page,
        "perPage": PAGE_SIZE,
        "region": "all",
        "sort": 2,
    });
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(500 * attempt));
        }
        let resp = match agent
            .post(&format!("{API}/offer/feed"))
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/json")
            .header("Referer", ORIGIN)
            .send_json(&body)
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] HTTP POST failed: {e}");
                continue;
            }
        };
        let status = resp.status();
        if status == 200 {
            let text = match resp.into_body().read_to_string() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read response: {e}");
                    continue;
                }
            };
            let feed: FeedResponse = match serde_json::from_str(&text) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[ERROR] Parse feed: {e}");
                    return (vec![], false);
                }
            };
            let items = feed
                .content
                .as_ref()
                .and_then(|c| c.items.clone())
                .unwrap_or_default();
            let has_more = feed
                .content
                .as_ref()
                .and_then(|c| c.paginator.as_ref())
                .map(|p| p.next_page_exists)
                .unwrap_or(false);
            return (items, has_more);
        }
        if status == 401 {
            eprintln!("[WARN] HTTP 401 — token expired");
            let _ = fs::remove_file(data_dir().join("token.txt"));
            return (vec![], false);
        }
        let text = resp.into_body().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() {
            "(empty)"
        } else {
            &text[..text.len().min(200)]
        };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    (vec![], false)
}

// ── Phases ─────────────────────────────────────────────────────────

fn phase1_initial_collection(agent: &ureq::Agent, state: &mut State) {
    eprintln!("[INFO] === Phase 1: Initial full collection ===");
    let token = get_token();
    let out_path = data_dir().join("birbir_export.jsonl");
    let mut out_file = fs::File::create(&out_path).expect("Failed to create output file");

    for page in 1..=MAX_PAGE {
        let (offers, has_more) = fetch_page(agent, &token, page);
        if offers.is_empty() {
            break;
        }
        for offer in &offers {
            let Some(oid) = offer.get("id").and_then(|v| v.as_u64()) else {
                continue;
            };
            state.max_id = state.max_id.max(oid);
            write_record(&mut out_file, &serde_json::to_string(offer).unwrap());
        }
        flush_output(&mut out_file);
        if !has_more {
            break;
        }
        std::thread::sleep(POLL_DELAY);
    }
    state.initial_complete = true;
    eprintln!("[INFO] Phase 1 complete, max_id = {}", state.max_id);
}

fn phase2_poll_new(agent: &ureq::Agent, state: &mut State) -> u32 {
    let token = get_token();
    let out_path = data_dir().join("birbir_export.jsonl");
    let mut out_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
        .expect("Failed to open output file");

    let mut new_count = 0u32;
    let mut cycle_max = state.max_id;
    let (offers, _) = fetch_page(agent, &token, 1);
    for offer in &offers {
        let Some(oid) = offer.get("id").and_then(|v| v.as_u64()) else {
            continue;
        };
        if oid <= state.max_id {
            continue;
        }
        cycle_max = cycle_max.max(oid);
        write_record(&mut out_file, &serde_json::to_string(offer).unwrap());
        new_count += 1;
    }
    if new_count > 0 {
        state.max_id = cycle_max;
    }
    new_count
}

// ── Main ───────────────────────────────────────────────────────────

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
    fn test_state_default() {
        let state = State::default();
        assert_eq!(state.version, 1);
        assert_eq!(state.max_id, 0);
        assert!(!state.initial_complete);
    }

    #[test]
    fn test_token_expired_malformed() {
        assert!(is_token_expired(""));
        assert!(is_token_expired("abc"));
        assert!(is_token_expired("a.b"));
    }

    #[test]
    fn test_token_expired_future() {
        let payload = serde_json::json!({ "exp": 9999999999u64 });
        let b64 = base64::engine::general_purpose::STANDARD.encode(payload.to_string());
        let token = format!("header.{b64}.sig");
        assert!(!is_token_expired(&token));
    }
}

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

pub const API: &str = "https://api.birbir.uz/api/frontoffice/1.3.5.0";
pub const ORIGIN: &str = "https://birbir.uz";
pub const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// Response wrapper for the feed endpoint.
#[derive(Debug, Deserialize)]
pub struct FeedResponse {
    pub content: Option<FeedContent>,
}

#[derive(Debug, Deserialize)]
pub struct FeedContent {
    pub items: Option<Vec<serde_json::Value>>,
    pub paginator: Option<Paginator>,
}

#[derive(Debug, Deserialize)]
pub struct Paginator {
    pub step: u64,
    pub current: u64,
    // BirBir API signals whether more pages exist via this field.
    // This is an external API contract, not an internal design choice.
    #[serde(rename = "nextPageExists")]
    pub next_page_exists: bool,
}

/// Returns ~/.local/share/birbir, creating a cross-platform PathBuf.
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/birbir")
}

// ── Auth token extraction (via agent-browser) ──────────────────────────

/// Try to read a cached token from disk.
fn read_cached_token() -> Option<String> {
    let path = data_dir().join("token.txt");
    let token = fs::read_to_string(path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Write token to disk cache.
fn cache_token(token: &str) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("token.txt");
    let tmp = format!("{}.tmp", path.display());
    if std::fs::write(&tmp, token).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Delete a cached token (e.g. after 401).
pub fn invalidate_cached_token() {
    let path = data_dir().join("token.txt");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(format!("{}.tmp", path.display()));
}

/// Check if a JWT token is expired or about to expire (within 60s).
fn is_token_expired(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() < 2 {
        return true;
    }
    // Quick check: decode the base64 payload just enough to find "exp"
    let payload_b64 = parts[1];
    let bytes = match simple_b64_decode(payload_b64) {
        Some(b) => b,
        None => return true,
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
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    exp <= now + 60
}

/// Minimal base64 decode (standard alphabet, handles padding).
fn simple_b64_decode(input: &str) -> Option<Vec<u8>> {
    // Map base64 char to value
    let val = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            b'=' => None, // padding
            _ => return None,
        }
    };

    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u8;
    for &b in &bytes {
        let v = val(b)?;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Fetch the session cookie from `birbir.uz` and extract the Bearer
/// access token.
///
/// Uses `agent-browser` CLI (a real Chrome-based browser) to get past
/// Cloudflare's JS challenge.  Falls back to cached token if available
/// and agent-browser is not installed.
pub fn extract_token() -> Option<String> {
    // Fallback: try cached token first (fastest path)
    if let Some(token) = read_cached_token() {
        if is_token_expired(&token) {
            eprintln!("[INFO] Cached token expired, re-fetching...");
            invalidate_cached_token();
        } else {
            eprintln!("[INFO] Using cached auth token ({} chars)", token.len());
            return Some(token);
        }
    }

    // Try agent-browser (real browser, handles Cloudflare)
    let t0 = std::time::Instant::now();
    let output = std::process::Command::new("agent-browser")
        .args(["cookies", "get", "--domain", "birbir.uz", "--json"])
        .output()
        .ok();

    if let Some(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(token) = parse_cookie_json(&stdout) {
                eprintln!(
                    "[INFO] Auth token obtained via agent-browser ({:?})",
                    t0.elapsed()
                );
                cache_token(&token);
                return Some(token);
            }
        }
    }

    // Last resort: try direct HTTP (retries with backoff handled inside direct_token_fetch)
    eprintln!("[INFO] Trying direct HTTP session fetch...");
    if let Some(token) = direct_token_fetch() {
        cache_token(&token);
        return Some(token);
    }
    None
}

/// Parse agent-browser's cookie JSON output and extract the session cookie.
fn parse_cookie_json(json_str: &str) -> Option<String> {
    // agent-browser cookies get --domain --json returns a JSON array of cookies
    let cookies: Vec<serde_json::Value> = serde_json::from_str(json_str).ok()?;
    for cookie in &cookies {
        let name = cookie.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.eq_ignore_ascii_case("session") {
            let val = cookie.get("value").and_then(|v| v.as_str())?;
            return parse_session_token(val);
        }
        // Also check the raw cookie format
        if name == "session" || name == "" {
            if let Some(val) = cookie.get("value").and_then(|v| v.as_str()) {
                if val.starts_with("j:") {
                    return parse_session_token(val);
                }
            }
        }
    }
    None
}

/// Try to fetch the session via direct HTTP (bypasses Cloudflare sometimes).
fn direct_token_fetch() -> Option<String> {
    for attempt in 0..5 {
        if attempt > 0 {
            eprintln!(
                "[INFO] Retrying direct HTTP session fetch (attempt {})...",
                attempt + 1
            );
            std::thread::sleep(std::time::Duration::from_millis(1000 * attempt));
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

        for line in headers.lines() {
            let lower = line.to_ascii_lowercase().trim().to_string();
            if lower.starts_with("set-cookie:") {
                let rest = line
                    .trim_start_matches(|c: char| c != ':')
                    .trim_start_matches(':')
                    .trim();
                if let Some(cookie_val) = rest.strip_prefix("session=") {
                    let end = cookie_val.find(';').unwrap_or(cookie_val.len());
                    let val = &cookie_val[..end];
                    if val.starts_with("j%3A") || val.starts_with("j:") {
                        let decoded = url_decode(val);
                        if let Some(token) = parse_session_token(&decoded) {
                            return Some(token);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Parse the session cookie value into an access token.
///
/// The cookie is URL-encoded JSON with a `j:` prefix.
fn parse_session_token(raw: &str) -> Option<String> {
    let decoded = if raw.starts_with("j%3A") || raw.contains('%') {
        url_decode(raw)
    } else {
        raw.to_string()
    };
    let without_prefix = decoded.strip_prefix("j:")?;
    let data: serde_json::Value = serde_json::from_str(without_prefix).ok()?;
    data.get("accessToken")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Minimal URL-decode (only handles %XX and + → space).
fn url_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hi = chars.next().and_then(|c| c.to_digit(16)).unwrap_or(0);
            let lo = chars.next().and_then(|c| c.to_digit(16)).unwrap_or(0);
            out.push(char::from((hi * 16 + lo) as u8));
        } else if ch == '+' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

// ── HTTP helpers ───────────────────────────────────────────────────────

/// Fetch JSON via GET with Bearer auth. Retries on failure.
pub fn fetch_json(agent: &ureq::Agent, url: &str, token: &str) -> Option<serde_json::Value> {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }

        let mut resp = match agent
            .get(url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/json")
            .header("Referer", ORIGIN)
            .call()
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] HTTP GET failed: {e}");
                continue;
            }
        };

        let status = resp.status().as_u16();
        if status == 200 {
            let text = match resp.body_mut().read_to_string() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read response body: {e}");
                    continue;
                }
            };
            return match serde_json::from_str(&text) {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("[ERROR] JSON parse error: {e}");
                    None
                }
            };
        }

        if status == 401 {
            eprintln!("[WARN] HTTP 401 — token expired, invalidating cache");
            invalidate_cached_token();
            return None;
        }

        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() {
            "(empty)"
        } else {
            &text[..text.len().min(200)]
        };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    None
}

/// POST JSON body with Bearer auth. Retries on failure.
pub fn post_json(
    agent: &ureq::Agent,
    url: &str,
    body: &serde_json::Value,
    token: &str,
) -> Option<serde_json::Value> {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }

        let mut resp = match agent
            .post(url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/json")
            .header("Referer", ORIGIN)
            .send_json(body)
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] HTTP POST failed: {e}");
                continue;
            }
        };

        let status = resp.status().as_u16();
        if status == 200 {
            let text = match resp.body_mut().read_to_string() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read response body: {e}");
                    continue;
                }
            };
            return match serde_json::from_str(&text) {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("[ERROR] JSON parse error: {e}");
                    None
                }
            };
        }

        if status == 401 {
            eprintln!("[WARN] HTTP 401 — token expired, invalidating cache");
            invalidate_cached_token();
            return None;
        }

        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() {
            "(empty)"
        } else {
            &text[..text.len().min(200)]
        };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    None
}

// ── 401 detection ─────────────────────────────────────────────────────

/// POST JSON and return `Err(true)` specifically when the server replies 401.
/// `Err(false)` means some other transient error; `Ok(val)` is success.
///
/// HTTP 401 indicates an expired token — this is the BirBir API's mechanism
/// for signalling that a new session token is needed. This is an external API
/// contract, not an internal design choice.
pub fn post_json_401(
    agent: &ureq::Agent,
    url: &str,
    body: &serde_json::Value,
    token: &str,
) -> Result<Option<serde_json::Value>, bool> {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }

        let mut resp = match agent
            .post(url)
            .header("Authorization", &format!("Bearer {token}"))
            .header("Accept", "application/json")
            .header("Referer", ORIGIN)
            .send_json(body)
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] HTTP POST failed: {e}");
                continue;
            }
        };

        let status = resp.status().as_u16();
        if status == 200 {
            let text = match resp.body_mut().read_to_string() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read response body: {e}");
                    continue;
                }
            };
            return match serde_json::from_str(&text) {
                Ok(v) => Ok(Some(v)),
                Err(e) => {
                    eprintln!("[ERROR] JSON parse error: {e}");
                    Ok(None)
                }
            };
        }

        if status == 401 {
            eprintln!("[WARN] HTTP 401 — token expired, invalidating cache");
            invalidate_cached_token();
            return Err(true);
        }

        let text = resp.body_mut().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() {
            "(empty)"
        } else {
            &text[..text.len().min(200)]
        };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    Err(false)
}

// ── Offer helpers ──────────────────────────────────────────────────────

/// Extract the numeric ID from an offer.
pub fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

const PAGE_SIZE: u64 = 40;
// Safety upper bound — BirBir API pagination is governed by the
// nextPageExists field in the feed response; this exists only as a
// defence-in-depth limit against runaway pagination.
const MAX_PAGE: u64 = 10000;
const POLL_DELAY_MS: u64 = 100;

// ── Lock file ──────────────────────────────────────────────────────────────

/// Acquire an exclusive lock on the data directory.
/// Exits immediately if another instance already holds the lock.
fn acquire_lock() -> fs::File {
    let dir = data_dir();
    let path = dir.join("birbir.lock");
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenStatus {
    Ok,
    Expired,
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
            version: 1,
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

/// Strip noise from a raw API offer, keeping only useful fields — flat format.
fn trim_offer(offer: &serde_json::Value) -> String {
    use serde_json::map::Map;

    let mut r = Map::new();

    // Top-level fields worth keeping
    for key in &[
        "id",
        "title",
        "price",
        "publishedAt",
        "webUri",
        "urgentSale",
        "courierDelivery",
        "business",
        "agency",
        "closed",
    ] {
        if let Some(v) = offer.get(*key) {
            r.insert(key.to_string(), v.clone());
        }
    }

    // Region: flatten titlePath + coordinates
    if let Some(region) = offer.get("region") {
        if let Some(tp) = region.get("titlePath") {
            r.insert("titlePath".to_string(), tp.clone());
        }
        if let Some(loc) = region.get("location") {
            if let Some(coords) = loc.get("coordinates") {
                r.insert("coordinates".to_string(), coords.clone());
            }
        }
    }

    // Seller: flatten to flat keys (seller_*)
    if let Some(seller) = offer.get("seller") {
        for (flat, src) in &[
            ("seller_uuid", "uuid"),
            ("seller_name", "name"),
            ("seller_verified", "verified"),
            ("seller_business", "business"),
            ("seller_agency", "agency"),
            ("seller_offerActiveCount", "offerActiveCount"),
        ] {
            if let Some(v) = seller.get(*src) {
                r.insert(flat.to_string(), v.clone());
            }
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
        });
        assert_eq!(state.version, 1);
        assert_eq!(state.max_id, 0);
        assert!(!state.initial_complete);
    }

    #[test]
    fn test_state_load_legacy_no_version() {
        let json = r#"{"max_id": 42, "initial_complete": true}"#;
        let state: State = serde_json::from_str(json).unwrap();
        assert_eq!(state.version, 1);
        assert_eq!(state.max_id, 42);
        assert!(state.initial_complete);
    }

    #[test]
    fn test_state_load_current_with_version() {
        let json = r#"{"version": 1, "max_id": 99, "initial_complete": false}"#;
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

// ── Pagination ──────────────────────────────────────────────────────────────

/// Fetch one page of offers from the feed.
/// Returns (offers, has_more, token_status).
fn fetch_page(
    agent: &ureq::Agent,
    token: &str,
    page: u64,
) -> (Vec<serde_json::Value>, bool, TokenStatus) {
    let url = format!("{API}/offer/feed");
    let body = serde_json::json!({
        "page": page,
        "perPage": PAGE_SIZE,
        "region": "all",
        "sort": 2,
    });

    let raw = match post_json_401(agent, &url, &body, token) {
        Ok(Some(v)) => v,
        Ok(None) => return (vec![], false, TokenStatus::Ok),
        Err(true) => return (vec![], false, TokenStatus::Expired),
        Err(false) => return (vec![], false, TokenStatus::Ok),
    };

    let parsed: FeedResponse = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] Parse error: {e}");
            return (vec![], false, TokenStatus::Ok);
        }
    };

    let content = match parsed.content {
        Some(c) => c,
        None => return (vec![], false, TokenStatus::Ok),
    };

    let offers = content.items.unwrap_or_default();
    let has_more = content
        .paginator
        .map(|p| p.next_page_exists)
        .unwrap_or(false);

    (offers, has_more, TokenStatus::Ok)
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

    let mut token = obtain_token();

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
        let (offers, has_more, status) = fetch_page(agent, &token, page);

        if status == TokenStatus::Expired {
            eprintln!("[INFO] Token expired during phase 1, refreshing...");
            token = obtain_token();
            continue; // retry same page with fresh token
        }

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
            let line = trim_offer(offer);
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
    let mut token = obtain_token();
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
    let old_max = state.max_id;

    loop {
        let t1 = std::time::Instant::now();
        let (offers, has_more, status) = fetch_page(agent, &token, page);
        eprintln!("[TIMING] fetch_page page={page}: {:?}", t1.elapsed());

        if status == TokenStatus::Expired {
            eprintln!("[INFO] Token expired during poll, refreshing...");
            token = obtain_token();
            continue; // retry same page with fresh token
        }

        if offers.is_empty() {
            break;
        }

        let mut all_old = true;

        for offer in &offers {
            let Some(oid) = extract_id(offer) else {
                continue;
            };
            if oid <= old_max {
                continue;
            }
            all_old = false;
            if oid > state.max_id {
                state.max_id = oid;
            }

            let line = trim_offer(offer);
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

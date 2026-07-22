use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

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
    if token.is_empty() { None } else { Some(token) }
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
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
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
        .args([
            "cookies",
            "get",
            "--domain",
            "birbir.uz",
            "--json",
        ])
        .output()
        .ok();

    if let Some(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(token) = parse_cookie_json(&stdout) {
                eprintln!("[INFO] Auth token obtained via agent-browser ({:?})", t0.elapsed());
                cache_token(&token);
                return Some(token);
            }
        }
    }

    // Last resort: try direct HTTP (retries with backoff)
    for attempt in 0..5 {
        if attempt > 0 {
            eprintln!("[INFO] Retrying direct HTTP session fetch (attempt {})...", attempt + 1);
            std::thread::sleep(std::time::Duration::from_millis(1000 * attempt));
        }
        eprintln!("[INFO] Trying direct HTTP session fetch...");
        if let Some(token) = direct_token_fetch() {
            cache_token(&token);
            return Some(token);
        }
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
            eprintln!("[INFO] Retrying direct HTTP session fetch (attempt {})...", attempt + 1);
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

// ── Offer helpers ──────────────────────────────────────────────────────

/// Extract the numeric ID from an offer.
pub fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

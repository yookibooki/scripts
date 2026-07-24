use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use ureq::http::StatusCode;

// ── Token source ────────────────────────────────────────────────────

/// Resolve the bearer token using the configured sources in priority order:
/// 1. UZUM_TOKEN env var
/// 2. Browser cookie database (Brave/Chrome/Chromium) — auto-extracts and decrypts
pub fn resolve_token() -> Option<String> {
    // 1. Env var
    if let Ok(t) = std::env::var("UZUM_TOKEN") {
        let t = t.trim().to_string();
        if !t.is_empty() {
            let token = normalize_token(t);
            cache_token(&token);
            return Some(token);
        }
    }
    // 2. Token cache on disk (avoids re-decrypting)
    if let Some(t) = read_cached_token() {
        return Some(t);
    }
    // 3. Browser cookie — decrypt from libsecret key
    browser_token()
}

/// Re-attempt token resolution after a 401, useful for recovery.
pub fn refresh_token() -> Option<String> {
    let cache = cache_path();
    if cache.exists() {
        let _ = fs::remove_file(&cache);
    }
    resolve_token()
}

/// Ensure the token has the "Bearer " prefix.
fn normalize_token(t: String) -> String {
    if t.starts_with("Bearer ") {
        t
    } else {
        format!("Bearer {t}")
    }
}

fn cache_path() -> PathBuf {
    let dir = data_dir();
    dir.join("token.cache")
}

fn cache_token(token: &str) {
    let path = cache_path();
    let tmp = format!("{}.tmp", path.display());
    if fs::write(&tmp, token).is_ok() {
        let _ = fs::rename(&tmp, &path);
    }
}

fn read_cached_token() -> Option<String> {
    let path = cache_path();
    if !path.exists() {
        return None;
    }
    let token = fs::read_to_string(&path).ok()?;
    let token = token.trim().to_string();
    if token.is_empty() || !token.starts_with("Bearer ") {
        return None;
    }
    Some(token)
}

/// Extract access_token from browser via `browser-cookie3` Python library.
fn browser_token() -> Option<String> {
    let script = r#"import sys
try:
    import browser_cookie3
    cj = browser_cookie3.brave(domain_name='uzum.uz')
    for c in cj:
        if c.name == 'access_token' and c.value.startswith('eyJ'):
            sys.stdout.write(c.value)
            sys.exit(0)
    sys.exit(1)
except Exception:
    sys.exit(1)
"#;
    for python in &["/tmp/cryptovenv/bin/python3", "/usr/bin/python3"] {
        if !std::path::Path::new(python).exists() { continue }
        let out = std::process::Command::new(python).arg("-c").arg(script).output().ok()?;
        if out.status.success() {
            let token = String::from_utf8(out.stdout).ok()?.trim().to_string();
            if !token.is_empty() && token.starts_with("eyJ") {
                let result = normalize_token(token);
                cache_token(&result);
                return Some(result);
            }
        }
    }
    None
}

pub const REST_API: &str = "https://api.uzum.uz/api";
pub const GRAPHQL_API: &str = "https://graphql.uzum.uz/";
pub const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

/// GraphQL query for product listings with pagination.
/// Mirrors the `MakeSearch_ItemsAndFilters` operation from the Uzum SPA.
pub const GRAPHQL_QUERY: &str = r#"query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) {
  makeSearch(query: $queryInput) {
    queryText
    category {
      id
      title
      title_ru
      title_uz
      parent { id title }
    }
    items {
      catalogCard {
        id
        title
        adult
        buyingOptions {
          isBestPrice
          priceBlock {
            sellPrice { amount description }
            finalPrice { amount description }
            fullPrice { amount description }
          }
          defaultSkuId
          isSingleSku
          deliveryOptions {
            shortDate
            stockType
          }
        }
        discount { discountPrice }
        minFullPrice
        minSellPrice
        photos {
          key
          link(trans: PRODUCT_540) {
            high
            low
          }
        }
        feedbackQuantity
        rating
        discovery {
          id
          productId
          title
          adult
        }
      }
    }
    total
  }
}"#;

/// Returns ~/.local/share/uzum, creating a cross-platform PathBuf.
pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".local/share/uzum")
}

/// Fetch and deserialize the JSON body from a REST GET endpoint.
/// Retries on transient HTTP errors and handles 429 rate limiting with backoff.
/// Returns (body, http_status) — http_status is None on connection errors.
pub fn fetch_rest_json(agent: &ureq::Agent, url: &str, token: &str) -> (Option<serde_json::Value>, Option<u16>) {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }

        let resp = match agent
            .get(url)
            .header("Accept", "application/json")
            .header("Authorization", token)
            .call()
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] HTTP request failed: {e}");
                continue;
            }
        };

        let status = resp.status().as_u16();
        if status == 200 {
            let text = match resp.into_body().read_to_string() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read response body: {e}");
                    continue;
                }
            };
            return match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => (Some(v), Some(status)),
                Err(e) => {
                    eprintln!("[ERROR] JSON parse error: {e}");
                    (None, Some(status))
                }
            };
        }

        if status == 429 {
            let delay = 500 * (1 << attempt);
            eprintln!("[WARN] Rate limited (429), retrying in {delay}ms...");
            std::thread::sleep(std::time::Duration::from_millis(delay));
            continue;
        }

        if status == 401 || status == 403 {
            eprintln!("[WARN] HTTP {status}: token expired or invalid — stop retrying");
            return (None, Some(status));
        }

        let text = resp.into_body().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() { "(empty)" } else { &text[..text.len().min(200)] };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    (None, None)
}

/// Execute a GraphQL query with auth and rate-limit handling.
/// Retries on 429 with exponential backoff.
/// Returns (body, http_status) — http_status is None on connection errors.
pub fn fetch_graphql(
    agent: &ureq::Agent,
    query: &str,
    variables: &serde_json::Value,
    token: &str,
) -> (Option<serde_json::Value>, Option<u16>) {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }

        let body = serde_json::json!({
            "operationName": "MakeSearch_ItemsAndFilters",
            "variables": variables,
            "query": query,
        });

        let body_bytes = serde_json::to_vec(&body).unwrap_or_default();

        let resp = match agent
            .post(GRAPHQL_API)
            .header("Content-Type", "application/json")
            .header("Authorization", token)
            .send(body_bytes.as_slice())
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] GraphQL request failed: {e}");
                continue;
            }
        };

        let status = resp.status().as_u16();
        if status == 200 {
            let text = match resp.into_body().read_to_string() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("[ERROR] Failed to read response body: {e}");
                    continue;
                }
            };
            return match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => (Some(v), Some(status)),
                Err(e) => {
                    eprintln!("[ERROR] JSON parse error: {e}");
                    (None, Some(status))
                }
            };
        }

        if status == 429 {
            let delay = 500 * (1 << attempt);
            eprintln!("[WARN] Rate limited (429), retrying in {delay}ms...");
            std::thread::sleep(std::time::Duration::from_millis(delay));
            continue;
        }

        if status == 401 || status == 403 {
            eprintln!("[WARN] HTTP {status}: token expired or invalid — stop retrying");
            return (None, Some(status));
        }

        let text = resp.into_body().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() { "(empty)" } else { &text[..text.len().min(200)] };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    (None, None)
}

/// In Uzum's GraphQL response, the product ID is nested under `catalogCard.id`.
pub fn extract_id(item: &serde_json::Value) -> Option<u64> {
    item.get("catalogCard")
        .and_then(|c| c.get("id"))
        .and_then(|v| v.as_u64())
}

// ── Lock file (single-instance enforcement) ─────────────────

/// Acquire an exclusive lock on the data directory.
/// Returns the lock file handle (must be kept alive) or exits on conflict.
pub fn acquire_lock() -> File {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("uzum.lock");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(false)
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

// ── Graceful shutdown ───────────────────────────────────────

/// Install signal handlers for SIGTERM and SIGINT.
/// Returns an `Arc<AtomicBool>` that becomes `true` when a shutdown is requested.
pub fn install_shutdown_handler() -> Arc<AtomicBool> {
    let flag = Arc::new(AtomicBool::new(false));
    let flag_clone = flag.clone();

    // SAFETY: signal_hook::flag::register is safe to call
    signal_hook::flag::register(signal_hook::consts::SIGTERM, flag_clone.clone())
        .expect("failed to register SIGTERM handler");
    signal_hook::flag::register(signal_hook::consts::SIGINT, flag.clone())
        .expect("failed to register SIGINT handler");

    flag
}

/// Returns true if the shutdown flag has been set.
pub fn shutdown_requested(flag: &AtomicBool) -> bool {
    flag.load(Ordering::Relaxed)
}

// ── Health reporting ────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct HealthReport {
    pub timestamp: u64,
    pub status: String,
    pub phase: String,
    pub max_id: u64,
    pub total_products: u64,
    pub poll_count: u32,
    pub new_since_last_poll: u32,
}

/// Write a health.json file atomically (write to .tmp, then rename).
pub fn write_health(report: &HealthReport) {
    let path = data_dir().join("health.json");
    let tmp = data_dir().join("health.json.tmp");
    if let Ok(json) = serde_json::to_string_pretty(report)
        && fs::write(&tmp, &json).is_ok()
    {
        let _ = fs::rename(&tmp, &path);
    }
}

/// Write a log line to both stderr and the log file.
pub fn log_to_file(log_file: &mut Option<BufWriter<File>>, msg: &str) {
    eprintln!("{msg}");
    if let Some(f) = log_file {
        let _ = writeln!(f, "{msg}");
        let _ = f.flush();
    }
}

/// Open (or create) the log file in append mode with rotation support.
/// Rotates if the file exceeds max_size_bytes.
pub fn open_log_file(max_size_bytes: u64) -> Option<BufWriter<File>> {
    let dir = data_dir();
    let path = dir.join("uzum.log");

    // Rotate if oversized
    if let Ok(meta) = fs::metadata(&path)
        && meta.len() > max_size_bytes
    {
        let rotated = dir.join(format!(
            "uzum-{}.log",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        let _ = fs::rename(&path, &rotated);
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .truncate(false)
        .open(&path)
        .ok()?;
    Some(BufWriter::new(file))
}

// ── Token expiry detection ──────────────────────────────────

/// Check if a response status indicates token expiry (401/403).
/// Returns true if the token is expired.
pub fn is_token_expired(status: u16) -> bool {
    status == 401 || status == 403
}

/// Convenience: convert StatusCode to u16 and check expiry.
pub fn status_expired(status: &StatusCode) -> bool {
    let code: u16 = status.as_u16();
    code == 401 || code == 403
}

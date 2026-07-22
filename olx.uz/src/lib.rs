use serde::Deserialize;
use std::path::PathBuf;

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
        let preview = if text.is_empty() { "(empty)" } else { &text[..text.len().min(200)] };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    None
}

/// Extract the numeric ID from an offer.
pub fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

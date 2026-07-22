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

/// Remove HTML tags and collapse whitespace.
pub fn strip_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_space = false;
    for ch in out.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                collapsed.push(' ');
                prev_space = true;
            }
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }
    collapsed.trim().to_string()
}

/// Extract the price from an offer's params array.
pub fn format_price(offer: &serde_json::Value) -> String {
    if let Some(params) = offer.get("params").and_then(|v| v.as_array()) {
        for p in params {
            if p.get("key").and_then(|v| v.as_str()) == Some("price") {
                if let Some(val) = p.get("value") {
                    if val.is_object() {
                        if let Some(v) = val.get("value") {
                            if let Some(s) = v.as_str() {
                                return s.to_string();
                            }
                            if let Some(n) = v.as_f64() {
                                return format!("{}", n as u64);
                            }
                        }
                    }
                }
                break;
            }
        }
    }
    "-".to_string()
}

/// Extract the numeric ID from an offer.
pub fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

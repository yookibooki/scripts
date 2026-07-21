use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::time::{Duration, Instant};

const API: &str = "https://www.olx.uz/api/v1/offers";
const PAGES: u32 = 2;
const PAGE_LIMIT: u32 = 40;
const STATE_FILE: &str = "state.json";
const OUTPUT_FILE: &str = "olx_posts.txt";

const MIN_SLEEP_MS: u64 = 1000;
const ADAPTIVE_EMPTY_ROUNDS: u32 = 3;
const MAX_POLL_INTERVAL_MS: u64 = 300_000;

const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: Option<Vec<serde_json::Value>>,
}

// ---------------------------------------------------------------------------
// State persistence
// ---------------------------------------------------------------------------

fn load_state() -> BTreeSet<u64> {
    fs::read_to_string(STATE_FILE)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<u64>>(&s).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

fn save_state(ids: &BTreeSet<u64>) {
    let tmp = format!("{}.tmp", STATE_FILE);
    let sorted: Vec<&u64> = ids.iter().collect();
    if let Ok(json) = serde_json::to_string(&sorted) {
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, STATE_FILE);
        }
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn append_post(title: &str, price: &str, desc: &str) {
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(OUTPUT_FILE)
    {
        let _ = writeln!(f, "{title}\n{price}\n{desc}\n");
    }
}

// ---------------------------------------------------------------------------
// HTTP helpers
// ---------------------------------------------------------------------------

fn make_agent() -> ureq::Agent {
    let config = ureq::config::Config::builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(20)))
        .build();
    ureq::Agent::new_with_config(config)
}

fn fetch_json(agent: &ureq::Agent, url: &str) -> Option<serde_json::Value> {
    let response = match agent
        .get(url)
        .header("Accept", "application/json")
        .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8,uz;q=0.7")
        .header("Referer", "https://www.olx.uz/")
        .header("User-Agent", USER_AGENT)
        .call()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] HTTP request failed: {} — {}", url, e);
            return None;
        }
    };
    let text = match response.into_body().read_to_string() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[ERROR] Failed to read response body: {} — {}", url, e);
            return None;
        }
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!(
                "[ERROR] JSON parse error for {}: {} (preview: {}…)",
                url,
                e,
                &text[..text.len().min(200)]
            );
            None
        }
    }
}

fn get_offers(agent: &ureq::Agent, offset: u32, limit: u32) -> Vec<serde_json::Value> {
    let url = format!("{API}/?offset={offset}&limit={limit}&query=");
    fetch_json(agent, &url)
        .and_then(|v| serde_json::from_value::<ApiResponse>(v).ok())
        .and_then(|r| r.data)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Data extraction & formatting
// ---------------------------------------------------------------------------

fn strip_html(raw: &str) -> String {
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

fn format_price(offer: &serde_json::Value) -> String {
    if let Some(params) = offer.get("params").and_then(|v| v.as_array()) {
        for p in params {
            if p.get("key").and_then(|v| v.as_str()) == Some("price") {
                if let Some(val) = p.get("value") {
                    if val.is_object() {
                        // Prefer converted_value, fall back to raw value
                        let v = val
                            .get("converted_value")
                            .filter(|v| !v.is_null())
                            .or_else(|| val.get("value"));
                        if let Some(n) = v.and_then(|x| x.as_f64()) {
                            if n == n.floor() {
                                return format!("{}", n as u64);
                            }
                            return format!("{}", n);
                        }
                        if let Some(s) = v.and_then(|x| x.as_str()) {
                            return s.to_string();
                        }
                    }
                }
                break;
            }
        }
    }
    eprintln!("[WARN] No price found for offer");
    "-".to_string()
}

fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

// ---------------------------------------------------------------------------
// Poll loop
// ---------------------------------------------------------------------------

fn poll(agent: &ureq::Agent, seen: &mut BTreeSet<u64>) -> u32 {
    let mut count = 0;

    for pg in 1..=PAGES {
        let offset = (pg - 1) * PAGE_LIMIT;
        let offers = get_offers(agent, offset, PAGE_LIMIT);
        if offers.is_empty() {
            break;
        }

        for offer in &offers {
            let Some(oid) = extract_id(offer) else { continue };
            if seen.contains(&oid) {
                return count;
            }
            seen.insert(oid);

            let title = offer
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let price = format_price(offer);
            let desc = offer
                .get("description")
                .and_then(|v| v.as_str())
                .map(strip_html)
                .unwrap_or_default();

            append_post(&title, &price, &desc);
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let poll_interval_ms: u64 = std::env::var("POLL_INTERVAL")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| option_env!("POLL_INTERVAL").and_then(|s| s.parse().ok()))
        .unwrap_or(15000);

    let agent = make_agent();
    let mut seen = load_state();
    let mut empty_rounds = 0u32;
    let mut current_interval = poll_interval_ms;

    eprintln!("Tracking {} IDs.", seen.len());

    loop {
        let t0 = Instant::now();
        let n = poll(&agent, &mut seen);

        if n > 0 {
            save_state(&seen);
        }

        // Adaptive poll interval: back off when nothing new
        if n == 0 {
            empty_rounds += 1;
            if empty_rounds >= ADAPTIVE_EMPTY_ROUNDS {
                current_interval = (current_interval * 2).min(MAX_POLL_INTERVAL_MS);
                empty_rounds = 0;
            }
        } else {
            empty_rounds = 0;
            current_interval = poll_interval_ms;
        }

        let elapsed = t0.elapsed();
        eprintln!(
            "Round: {} new in {:.1}s | tracked: {} | interval: {}s",
            n,
            elapsed.as_secs_f64(),
            seen.len(),
            current_interval / 1000
        );

        let sleep_ms = (current_interval as f64 - elapsed.as_secs_f64() * 1000.0)
            .max(MIN_SLEEP_MS as f64) as u64;
        std::thread::sleep(Duration::from_millis(sleep_ms));
    }
}

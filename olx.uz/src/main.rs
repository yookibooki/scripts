use serde::Deserialize;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::time::{Duration, Instant};

// ─── Config ─────────────────────────────────────────────────────────

const API: &str = "https://www.olx.uz/api/v1/offers";
const PAGES: u32 = 2;
const STATE_FILE: &str = "state.json";
const OUTPUT_FILE: &str = "olx_posts.txt";

const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

// ─── Data structures ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct PhonesData {
    phones: Option<Vec<String>>,
}

// ─── State ──────────────────────────────────────────────────────────

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

fn append_post(title: &str, price: &str, phone: &str, desc: &str) {
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(OUTPUT_FILE)
    {
        let _ = writeln!(f, "{title}\n{price}\n{phone}\n{desc}\n");
    }
}

// ─── HTTP helpers ───────────────────────────────────────────────────

fn fetch_json(url: &str) -> Option<serde_json::Value> {
    let config = ureq::config::Config::builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(20)))
        .build();
    let agent = ureq::Agent::new_with_config(config);
    let response = agent
        .get(url)
        .header("Accept", "application/json")
        .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.8,uz;q=0.7")
        .header("Referer", "https://www.olx.uz/")
        .header("User-Agent", USER_AGENT)
        .call()
        .ok()?;
    let text = response.into_body().read_to_string().ok()?;
    serde_json::from_str::<serde_json::Value>(&text).ok()
}

fn get_offers(offset: u32, limit: u32) -> Vec<serde_json::Value> {
    let url = format!("{API}/?offset={offset}&limit={limit}&query=");
    fetch_json(&url)
        .and_then(|v| serde_json::from_value::<ApiResponse>(v).ok())
        .and_then(|r| r.data)
        .unwrap_or_default()
}

fn get_phones(ad_id: &serde_json::Value) -> Vec<String> {
    let url = format!("{API}/{ad_id}/limited-phones/");
    fetch_json(&url)
        .and_then(|v| {
            serde_json::from_value::<PhonesData>(v.get("data")?.clone()).ok()
        })
        .and_then(|d| d.phones)
        .unwrap_or_default()
}

// ─── Strip HTML tags ────────────────────────────────────────────────

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

// ─── Format phone string ────────────────────────────────────────────

fn format_phone(phones: &[String]) -> String {
    if phones.is_empty() {
        "-".to_string()
    } else {
        phones.join(", ")
    }
}

// ─── Format price string ────────────────────────────────────────────

fn format_price(offer: &serde_json::Value) -> String {
    if let Some(params) = offer.get("params").and_then(|v| v.as_array()) {
        for p in params {
            if p.get("key").and_then(|v| v.as_str()) == Some("price") {
                if let Some(val) = p.get("value") {
                    if val.is_object() {
                        let v = val
                            .get("converted_value")
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
    "-".to_string()
}

// ─── Extract fields ─────────────────────────────────────────────────

fn extract_id(offer: &serde_json::Value) -> Option<u64> {
    offer.get("id").and_then(|v| v.as_u64())
}

// ─── Poll ───────────────────────────────────────────────────────────

fn poll(seen: &mut BTreeSet<u64>) -> u32 {
    let mut count = 0;

    for pg in 1..=PAGES {
        let offset = (pg - 1) * 40;
        let offers = get_offers(offset, 40);
        if offers.is_empty() {
            break;
        }

        for offer in &offers {
            let Some(oid) = extract_id(offer) else {
                continue;
            };
            if seen.contains(&oid) {
                return count;
            }
            seen.insert(oid);

            // Title
            let title = offer
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();

            // Price
            let price = format_price(offer);

            // Phone
            let rec_id = offer.get("id").cloned().unwrap_or(serde_json::Value::Null);
            std::thread::sleep(Duration::from_millis(400));
            let phones = get_phones(&rec_id);
            let phone = format_phone(&phones);

            // Description
            let desc = offer
                .get("description")
                .and_then(|v| v.as_str())
                .map(strip_html)
                .unwrap_or_default();

            append_post(&title, &price, &phone, &desc);
            count += 1;
        }
    }
    count
}

// ─── Main ───────────────────────────────────────────────────────────

fn main() {
    let poll_interval_ms: u64 = option_env!("POLL_INTERVAL")
        .and_then(|s| s.parse().ok())
        .unwrap_or(15000);

    let mut seen = load_state();
    eprintln!("Tracking {} IDs.", seen.len());

    loop {
        let t0 = Instant::now();

        let n = poll(&mut seen);
        save_state(&seen);

        let elapsed = t0.elapsed();
        eprintln!(
            "Round: {} new in {:.1}s | tracked: {}",
            n,
            elapsed.as_secs_f64(),
            seen.len()
        );

        let sleep_ms =
            (poll_interval_ms as f64 - elapsed.as_secs_f64() * 1000.0).max(1000.0);
        std::thread::sleep(Duration::from_millis(sleep_ms as u64));
    }
}

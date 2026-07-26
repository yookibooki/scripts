use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;

pub const GRAPHQL_URL: &str = "https://graphql.uzum.uz/";
pub const REST_BASE: &str = "https://api.uzum.uz/api";
pub const USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
pub const BATCH_SIZE: u64 = 100;
pub const OFFSET_CAP: u64 = 9900;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Deserialize)]
pub struct GqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GqlError>>,
}

#[derive(Debug, Deserialize)]
pub struct GqlError {
    pub message: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CategoryNode {
    pub id: u64,
    pub title: Option<String>,
    pub children: Option<Vec<CategoryNode>>,
}

#[derive(Debug, Deserialize)]
pub struct CategoriesResponse {
    pub payload: Option<Vec<CategoryNode>>,
}

#[derive(Debug, Deserialize)]
pub struct MakeSearchData {
    pub make_search: Option<SearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct SearchResult {
    pub items: Option<Vec<SearchItem>>,
    pub total: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct SearchItem {
    pub catalog_card: Option<ProductCard>,
}

#[derive(Debug, Deserialize)]
pub struct ProductCard {
    pub product_id: u64,
    pub title: Option<String>,
    pub min_full_price: Option<u64>,
    pub min_sell_price: Option<u64>,
    pub feedback_quantity: Option<u64>,
    pub rating: Option<f64>,
    pub buying_options: Option<BuyingOptions>,
    pub discount: Option<Discount>,
    pub promo_future_info: Option<PromoFutureInfo>,
    pub badges: Option<Vec<Badge>>,
}

#[derive(Debug, Deserialize)]
pub struct BuyingOptions {
    pub is_single_sku: Option<bool>,
    pub delivery_options: Option<Vec<DeliveryOptions>>,
}

#[derive(Debug, Deserialize)]
pub struct PriceBlock {
    pub sell_price: Option<Price>,
    pub full_price: Option<Price>,
    pub seller_price: Option<Price>,
}

#[derive(Debug, Deserialize)]
pub struct Price {
    pub amount: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct DeliveryOptions {
    pub short_date: Option<String>,
    pub stock_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Discount {
    pub discount_price: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PromoFutureInfo {
    pub min_future_price: Option<u64>,
    pub min_future_price_date: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Badge {
    pub id: Option<u64>,
    pub text: Option<String>,
    pub background_color: Option<String>,
    pub text_color: Option<String>,
    #[serde(rename = "iconLink")]
    pub icon_link: Option<String>,
    #[serde(rename = "endDate")]
    pub end_date: Option<u64>,
    #[serde(rename = "timerType")]
    pub timer_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CategoryProgress {
    pub total: u64,
    pub offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StateFile {
    pub version: u64,
    pub categories: HashMap<String, CategoryProgress>,
    pub item_count: u64,
    pub updated_at: String,
}

pub fn data_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local/share/uzum")
}

pub fn output_file() -> PathBuf {
    data_root().join("uzum_data.jsonl")
}

pub fn state_file() -> PathBuf {
    data_root().join("state.json")
}

pub fn lock_file() -> PathBuf {
    data_root().join("uzum.lock")
}

pub fn read_auth() -> (Option<String>, Option<String>) {
    (
        std::env::var("UZUM_ACCESS_TOKEN").ok(),
        std::env::var("UZUM_INSTALL_ID").ok(),
    )
}

pub fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent(USER_AGENT)
        .http_status_as_error(false)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .new_agent()
}

pub fn fetch_category_tree(agent: &ureq::Agent) -> Vec<CategoryNode> {
    let url = format!("{REST_BASE}/main/root-categories");
    let (token, _) = read_auth();
    let mut req = agent.get(&url).header("Accept", "application/json");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let resp = match req.call() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[ERROR] Categories fetch failed: {e}");
            return vec![];
        }
    };
    if resp.status() != 200 {
        eprintln!("[ERROR] Categories HTTP {}", resp.status());
        return vec![];
    }
    let text = match resp.into_body().read_to_string() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("[ERROR] Read categories response: {e}");
            return vec![];
        }
    };
    match serde_json::from_str::<CategoriesResponse>(&text) {
        Ok(r) => r.payload.unwrap_or_default(),
        Err(e) => {
            eprintln!("[ERROR] Parse category tree: {e}");
            vec![]
        }
    }
}

pub fn leaf_nodes(nodes: &[CategoryNode]) -> Vec<&CategoryNode> {
    let mut out = Vec::new();
    for n in nodes {
        match &n.children {
            Some(children) if !children.is_empty() => out.extend(leaf_nodes(children)),
            _ => out.push(n),
        }
    }
    out
}

pub fn graphql_request(
    agent: &ureq::Agent,
    query: &str,
    vars: &serde_json::Value,
    op_name: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut body = serde_json::json!({ "query": query, "variables": vars });
    if let Some(name) = op_name {
        body["operationName"] = serde_json::json!(name);
    }
    let body_str = serde_json::to_string(&body).map_err(|e| format!("Serialize: {e}"))?;
    let (token, iid) = read_auth();
    let mut req = agent
        .post(GRAPHQL_URL)
        .header("Content-Type", "application/json")
        .header("apollographql-client-name", "web-customers")
        .header("apollographql-client-version", "1.63.2")
        .header("city-id", "1");
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    if let Some(i) = iid {
        req = req.header("X-Iid", i);
    }
    let resp = req.send(&body_str).map_err(|e| format!("HTTP: {e}"))?;
    let text = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Body: {e}"))?;
    let parsed: GqlResponse<serde_json::Value> =
        serde_json::from_str(&text).map_err(|e| format!("JSON: {e}"))?;
    if let Some(errs) = parsed.errors {
        let msg: Vec<String> = errs.into_iter().map(|e| e.message).collect();
        return Err(msg.join("; "));
    }
    parsed.data.ok_or_else(|| "No data".into())
}

pub fn search_category(
    agent: &ureq::Agent,
    category_id: u64,
    offset: u64,
    limit: u64,
) -> Result<SearchResult, String> {
    let q = r#"
        query MakeSearch_ItemsAndFilters($input: MakeSearchQueryInput!) {
            makeSearch(query: $input) {
                items {
                    catalogCard {
                        productId title
                        minFullPrice minSellPrice
                        feedbackQuantity rating
                        priceBlock {
                            sellPrice { amount }
                            fullPrice { amount }
                            sellerPrice { amount }
                        }
                        buyingOptions {
                            isSingleSku
                            deliveryOptions { shortDate stockType }
                        }
                        discount { discountPrice }
                        promoFutureInfo { minFuturePrice minFuturePriceDate }
                        badges { id text backgroundColor textColor iconLink endDate timerType }
                    }
                }
                total
            }
        }
    "#;
    let vars = serde_json::json!({
        "input": {
            "categoryId": category_id.to_string(),
            "showAdultContent": "TRUE",
            "filters": [],
            "sort": "BY_ORDERS_NUMBER_DESC",
            "pagination": { "offset": offset, "limit": limit },
            "correctQuery": false,
            "getFastCategories": false,
        }
    });
    let data = graphql_request(agent, q, &vars, Some("MakeSearch_ItemsAndFilters"))?;
    let ms: MakeSearchData = serde_json::from_value(data).map_err(|e| format!("Parse: {e}"))?;
    Ok(ms.make_search.unwrap_or(SearchResult {
        items: None,
        total: None,
    }))
}

pub fn iso_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let s = d.as_secs();
    let (y, mo, dy) = civil_from_days((s / 86400) as i64);
    let h = (s % 86400) / 3600;
    let m = (s % 3600) / 60;
    let sec = s % 60;
    format!("{y:04}-{mo:02}-{dy:02}T{h:02}:{m:02}:{sec:02}.000Z")
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn main() {
    let is_refresh = std::env::args().any(|a| a == "--refresh");

    let root = data_root();
    fs::create_dir_all(&root).expect("Failed to create data directory");

    let lock_file = File::create(lock_file()).expect("Failed to create lock file");
    fs2::FileExt::try_lock_exclusive(&lock_file).expect("Another instance is already running");

    let agent = build_agent();
    let start = Instant::now();

    eprintln!("[INFO] Fetching category tree...");
    let tree = fetch_category_tree(&agent);
    if tree.is_empty() {
        eprintln!("[ERROR] No categories loaded. Check network or auth.");
        std::process::exit(1);
    }
    let leaves = leaf_nodes(&tree);
    eprintln!("[INFO] {} leaf categories", leaves.len());

    let ts = iso_now();

    let state: Option<StateFile> = fs::read_to_string(state_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let categories: Arc<Mutex<HashMap<String, CategoryProgress>>> = Arc::new(Mutex::new(
        state
            .as_ref()
            .map(|s| s.categories.clone())
            .unwrap_or_default(),
    ));

    let item_count = Arc::new(AtomicU64::new(
        state.as_ref().map(|s| s.item_count).unwrap_or(0),
    ));

    let out = output_file();
    let writer: Arc<Mutex<BufWriter<File>>> = if is_refresh && out.exists() {
        Arc::new(Mutex::new(BufWriter::new(
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&out)
                .unwrap(),
        )))
    } else {
        let file = File::create(&out).unwrap();
        let mut w = BufWriter::new(file);
        writeln!(
            w,
            "{}",
            serde_json::to_string(&serde_json::json!({
                "exportedAt": &ts,
                "totalProducts": 0,
                "version": "1.0.0",
                "source": "uzum.uz"
            }))
            .unwrap()
        )
        .ok();
        Arc::new(Mutex::new(w))
    };

    let leaves = Arc::new(leaves);
    let total = leaves.len();
    let ts = Arc::new(ts);
    let done = Arc::new(AtomicU64::new(0));
    let index = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    eprintln!("[INFO] Scanning...");

    std::thread::scope(|s| {
        for _ in 0..5 {
            let agent = agent.clone();
            let writer = writer.clone();
            let categories = categories.clone();
            let item_count = item_count.clone();
            let done = done.clone();
            let index = index.clone();
            let leaves = leaves.clone();
            let ts = ts.clone();

            s.spawn(move || loop {
                let i = index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= total {
                    break;
                }
                let node = &leaves[i];
                let cid = node.id;
                let ctitle = node.title.as_deref().unwrap_or("?");
                let key = cid.to_string();

                if is_refresh {
                    let map = categories.lock();
                    if let Some(p) = map.get(&key) {
                        if p.offset >= OFFSET_CAP || p.offset >= p.total {
                            done.fetch_add(1, Ordering::SeqCst);
                            continue;
                        }
                    }
                }

                let first = match search_category(&agent, cid, 0, BATCH_SIZE) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("[WARN] {ctitle} ({cid}): {e}");
                        done.fetch_add(1, Ordering::SeqCst);
                        continue;
                    }
                };

                let api_total = first.total.unwrap_or(0);
                if api_total == 0 {
                    done.fetch_add(1, Ordering::SeqCst);
                    continue;
                }

                if is_refresh {
                    let map = categories.lock();
                    if let Some(p) = map.get(&key) {
                        if p.total == api_total && p.offset >= api_total.min(OFFSET_CAP) {
                            drop(map);
                            done.fetch_add(1, Ordering::SeqCst);
                            continue;
                        }
                    }
                }

                let limit = api_total.min(OFFSET_CAP);

                let page_0_cards: Vec<_> = first
                    .items
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|i| i.catalog_card.as_ref())
                    .collect();

                {
                    let mut f = writer.lock();
                    for card in &page_0_cards {
                        let p = build_product(card, ctitle, cid, &ts);
                        let _ = writeln!(f, "{}", serde_json::to_string(&p).unwrap());
                        item_count.fetch_add(1, Ordering::SeqCst);
                    }
                }

                let mut page = 1u64;
                loop {
                    let offset = page * BATCH_SIZE;
                    if offset >= limit {
                        break;
                    }
                    match search_category(&agent, cid, offset, BATCH_SIZE) {
                        Ok(r) => {
                            let cards: Vec<_> = r
                                .items
                                .as_deref()
                                .unwrap_or_default()
                                .iter()
                                .filter_map(|i| i.catalog_card.as_ref())
                                .collect();
                            if cards.is_empty() {
                                break;
                            }
                            let mut f = writer.lock();
                            for card in &cards {
                                let p = build_product(card, ctitle, cid, &ts);
                                let _ = writeln!(f, "{}", serde_json::to_string(&p).unwrap());
                                item_count.fetch_add(1, Ordering::SeqCst);
                            }
                            page += 1;
                        }
                        Err(e) => {
                            if e.contains("too big query offset") {
                                break;
                            }
                            eprintln!("[WARN] {ctitle} ({cid}) offset {offset}: {e}");
                            break;
                        }
                    }
                }

                {
                    let mut map = categories.lock();
                    map.insert(
                        key,
                        CategoryProgress {
                            total: api_total,
                            offset: (page * BATCH_SIZE).min(limit),
                        },
                    );
                }

                let d = done.fetch_add(1, Ordering::SeqCst) + 1;
                if d % 50 == 0 || d == total as u64 {
                    let cnt = item_count.load(Ordering::SeqCst);
                    let el = start.elapsed();
                    eprintln!(
                        "[INFO] {d}/{total} ({ctitle}) — {cnt} items [{}.{:03}s]",
                        el.as_secs(),
                        el.subsec_millis()
                    );
                    persist(&categories, &item_count);
                }
            });
        }
    });

    persist(&categories, &item_count);

    let cnt = item_count.load(Ordering::SeqCst);
    let el = start.elapsed();
    eprintln!(
        "[INFO] Done: {cnt} items in {}.{:03}s",
        el.as_secs(),
        el.subsec_millis()
    );
    eprintln!("[INFO] Output: {}", output_file().display());
}

fn persist(categories: &Arc<Mutex<HashMap<String, CategoryProgress>>>, item_count: &AtomicU64) {
    let map = categories.lock();
    let state = StateFile {
        version: 1,
        categories: map.clone(),
        item_count: item_count.load(Ordering::SeqCst),
        updated_at: iso_now(),
    };
    drop(map);
    let tmp = state_file().with_extension("tmp");
    if let Ok(s) = serde_json::to_string(&state) {
        let _ = fs::write(&tmp, &s);
        let _ = fs::rename(&tmp, state_file());
    }
}

fn build_product(card: &ProductCard, _cat: &str, cat_id: u64, _ts: &str) -> serde_json::Value {
    let full = card.min_full_price.unwrap_or(0);
    let sell = card.min_sell_price.unwrap_or(0);
    let disc = if full > 0 && sell < full {
        ((1.0 - sell as f64 / full as f64) * 100.0).round() as u64
    } else {
        0
    };

    // priceBlock: nested { sellPrice: { amount }, fullPrice: { amount }, sellerPrice: { amount } }
    let price_block = serde_json::json!({
        "sellPrice": { "amount": card.min_sell_price },
        "fullPrice": { "amount": card.min_full_price },
        "sellerPrice": { "amount": card.min_sell_price },
    });

    // isSingleSku from buying_options
    let is_single_sku = card
        .buying_options
        .as_ref()
        .and_then(|bo| bo.is_single_sku);

    // deliveryOptions: first option's shortDate and stockType
    let delivery_options: Option<serde_json::Value> = card
        .buying_options
        .as_ref()
        .and_then(|bo| bo.delivery_options.as_ref())
        .and_then(|d| d.first())
        .map(|d| {
            serde_json::json!({
                "shortDate": d.short_date,
                "stockType": d.stock_type,
            })
        });

    // promoFutureInfo: { minFuturePrice, minFuturePriceDate }
    let promo_future_info = card.promo_future_info.as_ref().map(|pfi| {
        serde_json::json!({
            "minFuturePrice": pfi.min_future_price,
            "minFuturePriceDate": pfi.min_future_price_date,
        })
    });

    // badges: Vec of { id, text, backgroundColor, textColor }, filter out entries with no id AND no text
    let badges: Vec<serde_json::Value> = card
        .badges
        .as_ref()
        .map(|b| {
            b.iter()
                .filter(|b| b.id.is_some() || b.text.is_some())
                .map(|b| {
                    serde_json::json!({
                        "id": b.id,
                        "text": b.text,
                        "backgroundColor": b.background_color,
                        "textColor": b.text_color,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    serde_json::json!({
        "productId": card.product_id,
        "title": card.title.as_deref().unwrap_or(""),
        "categoryId": cat_id,
        "minFullPrice": card.min_full_price,
        "minSellPrice": card.min_sell_price,
        "priceBlock": price_block,
        "discountPercent": disc,
        "feedbackQuantity": card.feedback_quantity,
        "rating": card.rating,
        "isSingleSku": is_single_sku,
        "badges": badges,
        "promoFutureInfo": promo_future_info,
        "deliveryOptions": delivery_options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_product_discount() {
        let card = ProductCard {
            product_id: 1,
            title: Some("Widget".into()),
            min_full_price: Some(1000),
            min_sell_price: Some(700),
            feedback_quantity: Some(10),
            rating: Some(4.0),
            buying_options: None,
            discount: None,
            promo_future_info: None,
            badges: None,
        };
        let p = build_product(&card, "Test", 99, "2026-07-25T00:00:00.000Z");
        assert_eq!(p["productId"], 1);
        assert_eq!(p["minSellPrice"], 700);
        assert_eq!(p["discountPercent"], 30);
    }

    #[test]
    fn test_build_product_no_discount() {
        let card = ProductCard {
            product_id: 2,
            title: None,
            min_full_price: Some(500),
            min_sell_price: Some(500),
            feedback_quantity: None,
            rating: None,
            buying_options: None,
            discount: None,
            promo_future_info: None,
            badges: None,
        };
        let p = build_product(&card, "T", 1, "2026-07-25T00:00:00.000Z");
        assert_eq!(p["discountPercent"], 0);
        assert_eq!(p["rating"], serde_json::Value::Null);
    }

    #[test]
    fn test_iso_format() {
        let s = iso_now();
        assert!(s.len() > 20);
        assert!(s.ends_with('Z'));
    }

    #[test]
    fn test_build_product_price_block() {
        let card = ProductCard {
            product_id: 3,
            title: Some("Gadget".into()),
            min_full_price: Some(2000),
            min_sell_price: Some(1500),
            feedback_quantity: Some(5),
            rating: Some(4.5),
            buying_options: None,
            discount: None,
            promo_future_info: None,
            badges: None,
        };
        let p = build_product(&card, "G", 2, "2026-07-25T00:00:00.000Z");
        assert_eq!(p["priceBlock"]["sellPrice"]["amount"], 1500);
        assert_eq!(p["priceBlock"]["fullPrice"]["amount"], 2000);
    }
}

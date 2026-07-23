use std::path::PathBuf;

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
pub fn fetch_rest_json(agent: &ureq::Agent, url: &str, token: &str) -> Option<serde_json::Value> {
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

        if status == 429 {
            let delay = 500 * (1 << attempt);
            eprintln!("[WARN] Rate limited (429), retrying in {delay}ms...");
            std::thread::sleep(std::time::Duration::from_millis(delay));
            continue;
        }

        let text = resp.into_body().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() { "(empty)" } else { &text[..text.len().min(200)] };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    None
}

/// Execute a GraphQL query with auth and rate-limit handling.
/// Retries on 429 with exponential backoff.
pub fn fetch_graphql(
    agent: &ureq::Agent,
    query: &str,
    variables: &serde_json::Value,
    token: &str,
) -> Option<serde_json::Value> {
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
        }

        let body = serde_json::json!({
            "operationName": "MakeSearch_ItemsAndFilters",
            "variables": variables,
            "query": query,
        });

        let resp = match agent
            .post(GRAPHQL_API)
            .header("Content-Type", "application/json")
            .header("Authorization", token)
            .send_json(&body)
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[ERROR] GraphQL request failed: {e}");
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

        if status == 429 {
            let delay = 500 * (1 << attempt);
            eprintln!("[WARN] Rate limited (429), retrying in {delay}ms...");
            std::thread::sleep(std::time::Duration::from_millis(delay));
            continue;
        }

        let text = resp.into_body().read_to_string().unwrap_or_default();
        let preview = if text.is_empty() { "(empty)" } else { &text[..text.len().min(200)] };
        eprintln!("[WARN] HTTP {status}: {preview}");
    }
    None
}

/// Extract the numeric ID from a product item.
/// In Uzum's GraphQL response, the product ID is nested under `catalogCard.id`.
pub fn extract_id(item: &serde_json::Value) -> Option<u64> {
    item.get("catalogCard")
        .and_then(|c| c.get("id"))
        .and_then(|v| v.as_u64())
}

fn main() {
    let token = std::fs::read_to_string(
        std::path::Path::new(&std::env::var("HOME").unwrap())
            .join(".local/share/uzum/token.cache"),
    )
    .unwrap()
    .trim()
    .to_string();

    let agent = ureq::Agent::config_builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36")
        .http_status_as_error(false)
        .timeout_connect(Some(std::time::Duration::from_secs(15)))
        .timeout_global(Some(std::time::Duration::from_secs(30)))
        .build()
        .new_agent();

    let query = r#"query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) {
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

    let variables = serde_json::json!({
        "queryInput": {
            "categoryId": "10",
            "showAdultContent": "NONE",
            "filters": [],
            "sort": "BY_RELEVANCE_DESC",
            "pagination": {
                "offset": 0,
                "limit": 48
            },
            "correctQuery": false,
            "getFastCategories": true,
            "fastCategoriesLimit": 11,
            "fastCategoriesLevelOffset": 1,
            "getPromotionItems": true,
            "getFastFacets": true,
            "fastFacetsLimit": 10
        }
    });

    let body = serde_json::json!({
        "operationName": "MakeSearch_ItemsAndFilters",
        "variables": variables,
        "query": query,
    });

    let body_str = serde_json::to_string_pretty(&body).unwrap();
    println!("Body length: {} bytes", body_str.len());
    eprintln!("BODY_JSON: {body_str}");

    // Try 1: send_json
    eprintln!("--- Attempt 1: send_json ---");
    let resp = agent
        .post("https://graphql.uzum.uz/")
        .header("Content-Type", "application/json")
        .header("Authorization", &token)
        .send_json(&body);
    match resp {
        Ok(r) => {
            eprintln!("Status: {}", r.status());
            let text = r.into_body().read_to_string().unwrap_or_default();
            eprintln!("Body: {text}");
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }

    // Try 2: send with explicit body
    eprintln!("--- Attempt 2: send body ---");
    use ureq::SendBody;
    let sb = SendBody::from_json(&serde_json::json!({
        "operationName": "MakeSearch_ItemsAndFilters",
        "variables": variables,
        "query": query,
    }))
    .unwrap();
    let resp2 = agent
        .post("https://graphql.uzum.uz/")
        .header("Content-Type", "application/json")
        .header("Authorization", &token)
        .send(sb);
    match resp2 {
        Ok(r) => {
            eprintln!("Status: {}", r.status());
            let text = r.into_body().read_to_string().unwrap_or_default();
            eprintln!("Body: {text}");
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }
}
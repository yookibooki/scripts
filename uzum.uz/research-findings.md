# Uzum.uz API Investigation Findings

## Overview

Uzum.uz is a product marketplace (not classifieds like OLX). The frontend is a Vue.js SPA that communicates
with two backend services:

- **REST API**: `https://api.uzum.uz/api` — categories, promo, user data, popups
- **GraphQL API**: `https://graphql.uzum.uz/` — product listings, search, cart

## Authentication

- **Method**: Bearer JWT token in `Authorization` header
- **Token source**: Obtained from browser session cookies (user is logged in via Brave)
- **Token format**: `Bearer eyJraWQiOi...` (EdDSA-signed JWT)
- **No anonymous access**: Both REST and GraphQL APIs return 401 without a valid token
- **Token expiry**: ~10 hours (e.g., `exp: 1784753969` in the JWT payload)
- **Rate limiting**: HTTP 429 from the `search-gateway` subgraph when too many requests are made

## REST API Endpoints (`https://api.uzum.uz/api`)

### GET `/api/main/root-categories?eco=false`
Returns the full category tree. No pagination — all categories in one response.

**Response shape:**
```json
{
  "payload": [
    {
      "id": 2894,
      "title": "Mebel",
      "icon": null,
      "iconSvg": "<svg ...>",
      "iconLink": "https://static.uzum.uz/banners/mebel902.png",
      "children": [
        {
          "id": 17616,
          "title": "Karavot va matraslar",
          "productAmount": 1,
          "adult": false,
          "eco": false,
          "children": [ ... ],
          "path": [1, 2894, 17616]
        }
      ],
      "path": [1, 2894]
    }
  ],
  "timestamp": "2026-07-22T15:06:09.67510845"
}
```

Key fields:
- `id` — numeric category ID (u64)
- `title` — category name in Uzbek
- `children` — nested subcategories (same structure)
- `path` — array of ancestor IDs from root (e.g., `[1, 2894, 17616, 14557]`)
- `productAmount` — number of products in this category (leaf categories only)
- `adult` — whether the category contains adult content
- `eco` — whether it's an eco-friendly category

### GET `/api/main/promo-categories`
Returns promotional categories (small set, e.g., "Arzon narxlar kafolati").

**Response shape:**
```json
{
  "payload": [
    {
      "id": 391,
      "title": "Arzon narxlar kafolati",
      "subtitle": null,
      "iconLink": "https://static.uzum.uz/baner/gnc1405.png",
      "deepLink": "https://uzum.uz/category/garantiya-nizkikh-cen--937"
    }
  ],
  "timestamp": "..."
}
```

### GET `/api/user/purchases/preview`
Returns user purchase preview (returns `null` for anonymous/unauthenticated users).

### GET `/api/popup/active?installId=...&token=...`
Returns active popup notifications.

### GET `/api/nav-bar/entry-point`
Returns navigation bar data.

### POST `/api/analytics/v2/events`
Analytics events endpoint (not relevant for scraping).

## GraphQL API (`https://graphql.uzum.uz/`)

### Product Listing: `MakeSearch_ItemsAndFilters`

This is the **primary product listing query**. It uses offset-based pagination.

**Request:**
```json
{
  "operationName": "MakeSearch_ItemsAndFilters",
  "variables": {
    "queryInput": {
      "categoryId": "10020",
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
  },
  "query": "query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) { makeSearch(query: $queryInput) { ... } }"
}
```

**Key variables:**
- `categoryId` — string (e.g., `"10020"` for Electronics)
- `pagination.offset` — integer, starts at 0
- `pagination.limit` — integer, max observed: 48 (may accept higher)
- `sort` — enum: `BY_RELEVANCE_DESC`, `BY_PRICE_ASC`, `BY_PRICE_DESC`, `BY_RATING`, `BY_NEW`
- `filters` — array of filter objects (for faceted search)
- `showAdultContent` — `"NONE"`, `"ADULT_ONLY"`, `"ALL"`

**Response shape:**
```json
{
  "data": {
    "makeSearch": {
      "queryText": "",
      "category": {
        "id": 10020,
        "title": "Elektronika",
        "title_ru": "Электроника",
        "title_uz": "Eletronika",
        "parent": { "id": 10020, "title": "..." },
        "seo": { ... }
      },
      "items": [
        {
          "catalogCard": {
            "id": 123456,
            "title": "Product name",
            "adult": false,
            "buyingOptions": {
              "isBestPrice": true,
              "priceBlock": {
                "sellPrice": { "amount": "150000", "description": "so'm" },
                "finalPrice": { "amount": "150000", "description": "so'm" },
                "fullPrice": { "amount": "200000", "description": "so'm" },
                "sellerPrice": { "amount": "...", "description": "..." }
              },
              "defaultSkuId": 789,
              "isSingleSku": true,
              "deliveryOptions": {
                "shortDate": "1-2 kun",
                "stockType": "IN_STOCK"
              }
            },
            "discount": { "discountPrice": "50000" },
            "minFullPrice": 200000,
            "minSellPrice": 150000,
            "photos": [
              {
                "key": "abc123",
                "link": {
                  "high": "https://images.uzum.uz/.../product_540x540.jpg",
                  "low": "https://images.uzum.uz/.../product_120x120.jpg"
                }
              }
            ],
            "feedbackQuantity": 42,
            "rating": 4.5,
            "discovery": {
              "id": 123456,
              "productId": 123456,
              "title": "Product name",
              "adult": false,
              "__typename": "DiscoveryProductCard"
            },
            "__typename": "ProductCard"
          },
          "cpoAdvVersion": null,
          "cpoId": null,
          "bidId": null,
          "__typename": "CatalogItem"
        }
      ],
      "facets": [ ... ],
      "total": 1234
    }
  }
}
```

**Key product fields (from `catalogCard`):**
| Field | Type | Description |
|-------|------|-------------|
| `id` | u64 | Product ID (numeric) |
| `title` | string | Product name |
| `discovery.id` | u64 | Discovery product ID |
| `discovery.productId` | u64 | Product ID (same as `id` for single-SKU) |
| `buyingOptions.priceBlock.sellPrice.amount` | string | Selling price in UZS (tiyin → divide by 100) |
| `buyingOptions.priceBlock.finalPrice.amount` | string | Final price after discounts |
| `buyingOptions.priceBlock.fullPrice.amount` | string | Original/full price |
| `buyingOptions.priceBlock.sellPrice.description` | string | Currency ("so'm" = UZS) |
| `discount.discountPrice` | string | Discount amount |
| `minFullPrice` | u64 | Minimum full price (for SKU groups) |
| `minSellPrice` | u64 | Minimum sell price (for SKU groups) |
| `photos[0].link.high` | string | High-res image URL (540x540) |
| `photos[0].link.low` | string | Low-res image URL (120x120) |
| `feedbackQuantity` | u64 | Number of reviews |
| `rating` | f64 | Product rating (e.g., 4.5) |
| `buyingOptions.defaultSkuId` | u64 | Default SKU ID |
| `buyingOptions.isSingleSku` | bool | Whether product has a single SKU |
| `buyingOptions.deliveryOptions.shortDate` | string | Delivery time (e.g., "1-2 kun") |
| `buyingOptions.deliveryOptions.stockType` | string | Stock status (e.g., "IN_STOCK") |
| `adult` | bool | Adult content flag |

**Price handling:**
- Prices are stored as strings in UZS (e.g., `"150000"` = 150,000 UZS)
- The `description` field is `"so'm"` (UZS currency)
- Prices are NOT in tiyin — they appear to be in the main currency unit already
- `sellPrice` = current price, `fullPrice` = original price, `finalPrice` = final price after all discounts
- For SKU groups (multiple variants), `minSellPrice` and `minFullPrice` give the lowest prices across variants

**Pagination:**
- Offset-based: `pagination: { offset: N, limit: M }`
- `total` field gives the total number of products in the category
- Stop when `items` array is empty or fewer than `limit` items returned

**Rate limiting:**
- HTTP 429 from `search-gateway` subgraph when too many requests are made
- Need to handle 429 with exponential backoff
- Recommended delay: 200-500ms between requests

### Other GraphQL Queries

- `Cart_Summary` — cart contents
- `getMainContent` — homepage content (carousels, banners) with `products(page, size)`
- `FetchReviewForModal` — product reviews
- `getPageableVerticalOffer` — specific offer/carousel listings (uses `verticalOfferV2` with `offerId`)

## GraphQL Fragments (from JS bundle analysis)

### ProductCardFragment (on CatalogCard)
Composed of:
- `ProductCard_Identity`: `discovery { id, productId, title, adult }`, `__typename`
- `ProductCard_Commerce`: `buyingOptions { isBestPrice, priceBlock { ... } }`, `discount { discountPrice }`, `minFullPrice`, `minSellPrice`
- `ProductCard_Media`: `photos { key, link(trans: PRODUCT_540) { high, low } }`
- `ProductCard_Social`: `feedbackQuantity`, `rating`
- `ProductCard_Checkout`: `buyingOptions { defaultSkuId, isSingleSku, deliveryOptions { shortDate, stockType } }`

### PriceBlockFragment (on PriceBlock)
- `sellPrice` — current selling price
- `finalPrice` — final price after discounts
- `fullPrice` — original price
- `sellerPrice` — seller's price
Each price has: `amount` (string), `description` (currency string)

### CategoryShortFragment (on Category)
- `id`, `title`, `title_ru`, `title_uz`
- `parent { id, title, title_ru, title_uz }`
- `seo { permanentLinksSeo { ... } }`

## Key Differences from OLX

| Aspect | OLX | Uzum |
|--------|-----|------|
| API type | REST | GraphQL + REST |
| Auth | None | Bearer JWT token |
| Product ID | Top-level `id` field | `catalogCard.id` (nested) |
| Pagination | `offset` + `limit` query params | `pagination: {offset, limit}` in GraphQL variables |
| Price | `params[].value.converted_value` | `buyingOptions.priceBlock.sellPrice.amount` |
| Categories | Inline with products | Separate `/api/main/root-categories` endpoint |
| Category ID | `category.id` | `categoryId` in query variables |
| Image | `photos[0].link` | `photos[0].link.high` |
| Rating | Not available | `rating` (f64) |
| Reviews | Not available | `feedbackQuantity` (u64) |
| URL | `offer.url` | Construct from slug: `https://uzum.uz/product/{id}` |
| Rate limit | None observed | 429 from search-gateway |

## Architecture Implications for Uzum Scraper

1. **No anonymous access** — requires a valid JWT token (from a logged-in browser session)
2. **GraphQL for products** — must POST GraphQL queries to `https://graphql.uzum.uz/`
3. **REST for categories** — use `/api/main/root-categories?eco=false` to get the full category tree
4. **Offset-based pagination** — use `pagination: {offset, limit}` in GraphQL variables
5. **Rate limiting** — handle 429 with exponential backoff, use 200-500ms delays
6. **Category BFS** — same strategy as OLX: get root categories, then paginate each category
7. **No separate category endpoint needed** — categories are returned in the root-categories response with nested children
8. **Product ID** — use `catalogCard.id` as the unique identifier
9. **Price** — `buyingOptions.priceBlock.sellPrice.amount` is the current price in UZS
10. **URL** — construct as `https://uzum.uz/product/{id}` (Uzum uses numeric IDs in URLs)

NOTE: If you'd like to dig deeper, feel free to use tools like agent-browser, curl, or Python—whatever is at your disposal. You have the capability to handle any task, so don't hesitate to ask me questions or discuss your next steps whenever you need guidance.

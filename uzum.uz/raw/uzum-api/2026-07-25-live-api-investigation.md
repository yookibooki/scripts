# Live API Investigation — Uzum.uz Marketplace

**Source URL**: https://uzum.uz/
**Collected**: 2026-07-25
**Published**: Unknown

## Method
Live browser investigation via Chrome DevTools MCP while logged into uzum.uz. Requests captured from the production web application at https://uzum.uz/.

## REST Endpoint: Category Tree

**URL**: `GET https://api.uzum.uz/api/main/root-categories`

**Response**: JSON wrapper `{ "payload": [...] }` containing nested category nodes.

**Current live data** (2026-07-25):
- Top-level categories: **23**
- Total leaf categories: **1627**
- Each node has: `id` (u64), `title`, `slug`, `image`, `children` (recursive)
- Leaf categories have empty or absent `children` array

**Sample leaf IDs from live response**: 14557 (Matraslar), 17421 (Karavotlar), 17633 (Yotoqxona uchun mebel to'plamlari), 17574 (Mebel uchun butlovchi qismlar), 17573 (Mebel uchun furnitura), 16777 (Ofis kreslolari), 16776 (Geymerlar uchun kreslolari), 63 (Seyflar), 17486 (Partalar), 17635 (Ofis mebeli to'plamlari).

## GraphQL Endpoint

**URL**: `POST https://graphql.uzum.uz/`

### Auth Headers (live capture)
- `Authorization: Bearer <JWT token>` (from `access_token` cookie)
- `X-Iid: <installId>` (from `clickstream-client.installId` cookie)
- `apollographql-client-name: web-customers`
- `apollographql-client-version: 1.63.2`
- `city-id: 1`
- Also sends: `city-longitude`, `city-latitude`, `longitude`, `latitude`, `accept-language: uz-UZ`

### Query: MakeSearch_ItemsAndFilters

Variables shape:
```json
{
  "queryInput": {
    "categoryId": "75",
    "showAdultContent": "TRUE",
    "filters": [],
    "sort": "BY_ORDERS_NUMBER_DESC",
    "pagination": { "offset": 0, "limit": 100 },
    "correctQuery": false,
    "getFastCategories": false
  }
}
```

Fields returned per item: `id` (UUID), `productId` (int), `title`, `adult`, `minFullPrice`, `minSellPrice`, `feedbackQuantity`, `rating`, `discount { discountPrice }`.

**Live test**: Category 75 returned total=9248, 5 items in first page.

### Offset Limit
- `offset=9900, limit=100` → Error: `"validate: too big query offset: validation error"`
- Max safe offset: 9900 (with limit 100, stays under the 10000 hard cap).

## CatalogCard Type (Extended)
The full `CatalogCard` type (seen in `getMainContent` query) includes additional fields not used by `MakeSearch_ItemsAndFilters`:
- `buyingOptions { isBestPrice, priceBlock, defaultSkuId, isSingleSku, deliveryOptions }`
- `badges` (array of promotional badges)
- `photos { key, link { high, low } }`
- `infoLabel { color, title }`
- `offer { due, icon }`
- `promoFutureInfo { minFuturePrice, minFuturePriceDate }`

These are not needed for the simple collection use case.

## Other Observations
- The analytics endpoint `customer-resources.uzum.uz/api/analytics/v2/events` is used for telemetry.
- Sentry is configured at `sentry.infra.cluster.daymarket.uz`.
- The web app version is `1.63.2` (matches `apollographql-client-version` header).

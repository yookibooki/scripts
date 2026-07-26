# Uzum Marketplace Collector (uzum-watch)

> Sources: uzum.uz, 2026-07-26 (live run & debug session)
> Raw: [Live API Investigation](../../raw/uzum-api/2026-07-25-live-api-investigation.md); [API End-to-End Analysis](../../raw/uzum-api/2026-07-25-api-end-to-end-analysis.md)
> Updated: 2026-07-26

## Overview

A Rust CLI tool that scrapes the full product catalog from `uzum.uz` (Uzbekistan's largest marketplace). Runs as a systemd user timer for daily incremental updates. Outputs data to `~/.local/share/uzum/uzum_data.jsonl` in JSON Lines format. Persists scan progress in `state.json` so each run only fetches new or changed products.

## Architecture

The binary is single-threaded and scans leaf categories sequentially. For each category it sends a single GraphQL query for the first page, writes matching products to the output file, then paginates through remaining pages. Output is JSON Lines with one `ProductRecord` per line.

**Two modes:**
- **Full** (no args): Scans every leaf category from scratch. Creates a new output file with a JSON header line.
- **Refresh** (`--refresh`): Compares saved API totals against current totals per category. Only deep-scans categories where the total increased or collection didn't complete. Appends to the existing output file.

**Lock file:** `~/.local/share/uzum/uzum.lock` -- flock-based exclusive lock prevents concurrent runs. Exits immediately if lock cannot be acquired.

**Atomic saves:** State is written to `state.json.tmp` then renamed to `state.json`.

**Progress:** Every 50 categories the tool logs elapsed time, category count, and item count to stderr, then persists `state.json`. A final persist runs on completion.

## API Endpoints

| Endpoint | Type | Purpose |
|----------|------|---------|
| `GET /api/main/root-categories` | REST | Fetch full category tree (no pagination) |
| `POST https://graphql.uzum.uz/` | GraphQL | `MakeSearch_ItemsAndFilters` query for product search |

### Auth Headers (live capture)

- `Authorization: Bearer <JWT>` (from `access_token` cookie)
- `x-iid: <installId>` (from `clickstream-client.installId` cookie)
- `apollographql-client-name: web-customers`
- `apollographql-client-version: 1.63.2`
- `city-id: 1`

### GraphQL Query

The tool uses the full `ProductCardFragment` with all available subfields:

```graphql
query MakeSearch_ItemsAndFilters($input: MakeSearchQueryInput!) {
  makeSearch(query: $input) {
    items {
      catalogCard {
        productId title
        minFullPrice minSellPrice
        feedbackQuantity rating
        buyingOptions {
          isSingleSku
          deliveryOptions { shortDate stockType }
        }
        promoFutureInfo { minFuturePrice minFuturePriceDate }
        badges { id text backgroundColor textColor }
      }
    }
    total
  }
}
```

Variables:

```json
{
  "categoryId": "123",
  "showAdultContent": "TRUE",
  "filters": [],
  "sort": "BY_ORDERS_NUMBER_DESC",
  "pagination": { "offset": 0, "limit": 100 },
  "correctQuery": false,
  "getFastCategories": false
}
```

### Known Limits

- **Offset cap:** GraphQL rejects `offset + limit > 10000` with error `"too big query offset"`. Max safe offset is 9900 (with batch size 100). Categories with >10K items are truncated to the first ~10K.
- **Token expiry:** JWT tokens expire approximately 10 hours after issue (`exp` claim). The tool reads `UZUM_ACCESS_TOKEN` from environment; stale tokens must be refreshed from the browser session (`id.uzum.uz/api/auth/token` POST returns 204 with `Set-Cookie`).
- **Rate limiting:** Sequential requests avoid the 429 rate limit. The API search-gateway returns 429 when hit concurrently. The tool is deliberately single-threaded to stay under this limit.

## Output Format

`~/.local/share/uzum/uzum_data.jsonl` -- JSON Lines with a header line:

```json
{"exportedAt":"2026-07-26T12:00:00.000Z","totalProducts":0,"version":"1.0.0","source":"uzum.uz"}
{"productId":2105667,"title":"Topper-matras, 8 sm","categoryId":14557,"minFullPrice":1300000,"minSellPrice":747000,"discountPercent":43,"feedbackQuantity":4,"rating":5.0,"isSingleSku":false,"badges":[{"id":41,"text":"52 912 сум/мес","backgroundColor":"#FFFF00","textColor":"#1F1F26"},{"id":468,"text":"Aksiya","backgroundColor":"#4d4dff","textColor":"#ffffff"}],"promoFutureInfo":null,"deliveryOptions":{"shortDate":"Завтра","stockType":"FBO"}}
```

Fields:
- `productId` (u64) -- Uzum product ID
- `title` (string) -- product title (Uzbek/Cyrillic)
- `categoryId` (u64) -- leaf category ID
- `minFullPrice` (u64 or null) -- full price before discount
- `minSellPrice` (u64 or null) -- current selling price
- `discountPercent` (u64) -- computed discount: `round((1 - sell/full) * 100)`; 0 when no discount
- `feedbackQuantity` (u64 or null) -- review count
- `rating` (f64 or null) -- average rating out of 5.0
- `isSingleSku` (bool or null) -- whether product has a single SKU
- `badges` (array) -- promotional badges; each has `id` (u64 or null), `text`, `backgroundColor`, `textColor`
- `promoFutureInfo` (object or null) -- future price drop info: `minFuturePrice`, `minFuturePriceDate` (epoch ms)
- `deliveryOptions` (object or null) -- delivery info: `shortDate` (relative string like "Завтра"), `stockType` ("FBO" or "FBS")

## State Schema

`~/.local/share/uzum/state.json`:

```json
{
  "version": 1,
  "categories": {
    "123": { "total": 500, "offset": 500 },
    "456": { "total": 100, "offset": 100 }
  },
  "item_count": 248668,
  "updated_at": "2026-07-26T12:00:00.000Z"
}
```

- `categories`: keyed by category ID string. `total` = API-reported total, `offset` = furthest offset reached (capped at 9900).
- `item_count`: running total of all products written.

## Performance

Measured on a Celeron N4000 with 100 Mbps internet:

- **First 50 categories:** 15,196 items in ~97s (~1.9s per category, including pagination)
- **Estimated full scan** (1627 leaf categories): ~52 minutes, ~500K items
- **Per-category timing:** ~0.5s for categories with 0 products (single request), up to several seconds for categories with thousands of products (pagination across multiple pages)

## Bugs Fixed During Development

1. **Serde camelCase mismatch** -- All GraphQL deserialization structs needed `#[serde(rename_all = "camelCase")]`. Without it, fields like `makeSearch`, `catalogCard`, `minFullPrice` silently failed to deserialize into their snake_case Rust equivalents.
2. **DeliveryOptions typed as Vec** -- `deliveryOptions` in the API response is a single object, not an array. The struct `BuyingOptions.delivery_options` was `Option<Vec<DeliveryOptions>>` causing `"invalid type: map, expected a sequence"` on every category.
3. **Concurrency causing 429** -- Early versions used 5 concurrent worker threads, which immediately triggered rate limiting (`HTTP 429 Too Many Requests`) on every single GraphQL query. Switching to sequential processing solved this.

## Installation

```bash
cargo build --release
cp target/release/uzum-watch ~/.local/bin/

# Auth setup
cat > ~/.config/uzum/env << 'EOF'
UZUM_ACCESS_TOKEN=your_jwt_token
UZUM_INSTALL_ID=your_install_id
EOF
chmod 600 ~/.config/uzum/env
```

## Systemd Timer

```bash
cp systemd/uzum-watch.service systemd/uzum-watch.timer ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now uzum-watch.timer
```

Runs daily via `OnCalendar=daily`. First run does a full collection; subsequent runs use `--refresh` automatically.

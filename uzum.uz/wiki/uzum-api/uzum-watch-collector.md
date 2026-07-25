# Uzum Marketplace Collector (uzum-watch)

> Sources: uzum.uz, 2026-07-25 (live API investigation)
> Raw: [Live API Investigation](../../raw/uzum-api/2026-07-25-live-api-investigation.md); [API End-to-End Analysis](../../raw/uzum-api/2026-07-25-api-end-to-end-analysis.md)
> Updated: 2026-07-25

## Overview

A Rust CLI tool that continuously scrapes the full product catalog from `uzum.uz` (Uzbekistan's largest marketplace). Runs as a systemd user timer for daily incremental updates. Outputs data to `~/.local/share/uzum/uzum_data.jsonl` in JSON Lines format. Persists scan progress in `state.json` so each run only fetches new or changed products.

## Architecture

The binary is single-threaded with 5 concurrent worker threads (to stay under rate limits). Each worker claims a leaf category via atomic index, scans all pages of products via GraphQL, writes to the shared output file under a mutex, and records the category's progress.

**Two modes:**
- **Full** (no args): Scans every leaf category from scratch. Creates a new output file with a JSON header line.
- **Refresh** (`--refresh`): Compares saved API totals against current totals per category. Only deep-scans categories where the total increased or collection didn't complete. Appends to the existing output file.

**Lock file:** `~/.local/share/uzum/uzum.lock` — flock-based exclusive lock prevents concurrent runs. Exits immediately if lock cannot be acquired.

**Atomic saves:** State is written to `state.json.tmp` then renamed to `state.json`.

## API Endpoints

| Endpoint | Type | Purpose |
|----------|------|---------|
| `GET /api/main/root-categories` | REST | Fetch full category tree (no pagination, with optional `?eco=false` param) |
| `GET /api/main/promo-categories` | REST | Promotional category cards |
| `GET /api/popup/active` | REST | Active popup content (auth required) |
| `GET /api/user/purchases/preview` | REST | User purchase preview (auth required) |
| `GET /api/user/name` | REST | User name (auth required) |
| `GET /api/user/contacts` | REST | User contacts (auth required) |
| `POST https://graphql.uzum.uz/` | GraphQL | `MakeSearch_ItemsAndFilters` query for product search |
| `POST https://graphql.uzum.uz/` | GraphQL | `MakeSearch_Categories` query for subcategory tree |
| `POST https://graphql.uzum.uz/` | GraphQL | `getMainContent` query for homepage content |
| `POST https://graphql.uzum.uz/` | GraphQL | `RecommendationBlocks` query for similar products |

### Auth Headers (live capture)

- `Authorization: Bearer <JWT>` (from `access_token` cookie)
- `x-iid: <installId>` (from `clickstream-client.installId` cookie)
- `apollographql-client-name: web-customers`
- `apollographql-client-version: 1.63.2`
- `city-id: 1`
- `city-longitude`, `city-latitude`, `longitude`, `latitude` (geolocation)
- `accept-language: uz-UZ`
- `sentry-trace` and `baggage` headers for Sentry distributed tracing

### GraphQL Query

```graphql
query MakeSearch_ItemsAndFilters($input: MakeSearchQueryInput!) {
  makeSearch(query: $input) {
    items {
      catalogCard {
        productId title
        minFullPrice minSellPrice
        feedbackQuantity rating
      }
    }
    total
  }
}
```

Full variable set (web uses more fields):

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

Web category page uses `sort: "BY_RELEVANCE_DESC"`, `limit: 48`, `getFastCategories: true`.

### Known Limits

- **Offset cap:** GraphQL rejects `offset + limit > 10000` with error `"too big query offset"`. Max safe offset is 9900 (with batch size 100). Categories with >10K items are truncated to the first ~10K.
- **Auth-restricted categories:** ~85 categories return `total` but 0 items without auth headers.
- **Mid-scan failures:** ~18 categories fail intermittently at specific offsets (API instability). The tool skips these with a warning.

## Output Format

`~/.local/share/uzum/uzum_data.jsonl` — JSON Lines with a header line:

```json
{"exportedAt":"2026-07-25T12:00:00.000Z","totalProducts":0,"version":"1.0.0","source":"uzum.uz"}
{"id":123456,"title":"Product Name","price":80000,"oldPrice":100000,"discountPercent":20,"rating":4.5,"reviewCount":42,"category":"Kategoriya nomi","categoryId":12345,"firstSeen":"...","lastSeen":"..."}
```

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
  "updated_at": "2026-07-25T12:00:00.000Z"
}
```

- `categories`: keyed by category ID string. `total` = API-reported total, `offset` = furthest offset reached (capped at 9900).
- `item_count`: running total of all products written.

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

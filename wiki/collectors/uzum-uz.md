# Uzum.uz — API Reference & Collector

> Sources: uzum.uz WIKI, 2026-07-27; uzum.uz AGENTS, 2026-07-27; uzum.uz API end-to-end analysis, 2026-07-25; uzum.uz live API investigation, 2026-07-25; uzum.uz live page snapshot, 2026-07-25
> Raw: [Uzum.uz WIKI](../../raw/collectors/2026-07-27-uzum-wiki.md); [Uzum.uz AGENTS](../../raw/collectors/2026-07-27-uzum-agents.md); [Uzum e2e analysis](../../raw/collectors/2026-07-27-uzum-wiki.md); [Uzum live API](../../raw/collectors/2026-07-27-uzum-wiki.md); [Uzum page snapshot](../../raw/collectors/2026-07-27-uzum-wiki.md)
> Updated: 2026-07-27

## Overview

Uzum.uz is an Uzbekistan-based e-commerce marketplace. The API consists of a GraphQL endpoint (`graphql.uzum.uz`) for product discovery and a REST API (`api.uzum.uz`) for auxiliary operations. The collector (`uzum-watch`) is a single-threaded Rust CLI that scans leaf categories sequentially via GraphQL queries.

## Infrastructure

| Component | Value |
|-----------|-------|
| **Domain** | `uzum.uz` |
| **Web app** | `1.63.2` |
| **Build** | `884252` |
| **API** | `graphql.uzum.uz` (GraphQL/Apollo), `api.uzum.uz` (REST) |
| **CDN** | `images.uzum.uz` (products), `static.uzum.uz` (static assets) |
| **Sentry** | `sentry.infra.cluster.daymarket.uz` |
| **Feature flags** | GrowthBook (`cdn.growthbook.io/api/features`) |

## Common Headers

**REST**: `Authorization: Bearer <JWT>`, `x-iid: <installId>`, `accept-language: uz-UZ`, `sentry-trace`, `baggage`

**GraphQL** (adds): `apollographql-client-name: web-customers`, `apollographql-client-version: 1.63.2` (homepage/category) or `1.34.6` (PDP), `city-id`, `city-latitude`, `city-longitude`, `latitude`, `longitude`, `x-context: null` (PDP only)

## REST Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /api/main/root-categories?eco=false` | Full category tree (recursive children, path ancestors) |
| `GET /api/main/promo-categories` | Promotional category cards |
| `GET /api/popup/active` | Active popups |
| `GET /api/user/purchases/preview` | Purchase preview (auth) |
| `GET /api/user/name` | Display name (auth) |
| `GET /api/user/contacts` | Contact info (auth) |
| `POST /api/main/cities/city-by-location` | Geo-location |

### Category node fields

`id`, `title`, `icon`, `iconSvg`, `iconLink`, `productAmount`, `adult`, `eco`, `seoMetaTag`, `seoHeader`, `children` (recursive), `path` (ancestor chain).

## GraphQL

Single endpoint: `POST https://graphql.uzum.uz/`

### Homepage queries

`Cart_Summary` — cart state; `getMainContent` — banners, promo blocks, carousels, today's deals; `FetchReviewForModal` — pending review modal.

### Category page queries

**`MakeSearch_ItemsAndFilters`** — Product search with pagination, facets, fast categories. Key parameters: `$queryInput: MakeSearchQueryInput!` with `offerCategoryId`, `showAdultContent`, `filters`, `sort`, `pagination`, `correctQuery`, `getFastCategories`, `getPromotionItems`, `getFastFacets`.

### PDP queries

`GetTabs`, `ProductPage` (installment widget), `Feedbacks`, `FeedbackPhotos`, `RecommendationBlocks`, `getViewedProducts`, `Suggestions`, `addViewedProduct` (mutation).

### Notable differences from earlier assumptions

- Variable name is `$queryInput` (not `$input`), field is `offerCategoryId` (not `categoryId`)
- `getFastCategories` defaults to `true`; `limit: 48` for category page UI
- `deliveryOptions` is a single object, not an array
- Sort modes: `BY_RELEVANCE_DESC`, `BY_ORDERS_NUMBER_DESC`

## CatalogCard (Product Card)

Union of `ProductCard` (single-SKU) and `SkuGroupCard` (multi-variant). Includes shared fragments for identity, commerce, marketing, media, social, and checkout data. PriceBlock includes badge types: StickerBadge, FomoTimerBadge, BottomTextBadge, UzumInstallmentTitleBadge, BottomIconTextBadge. Delivery stock types: FBS (seller), FBO (Uzum).

## Collector: uzum-watch

### Architecture

Single-threaded, sequential. Scans leaf categories one at a time. For each: send `MakeSearch_ItemsAndFilters` for first page, write products, paginate through remaining pages.

**Two modes**:
- **Full** (no args): Scans every leaf category from scratch
- **Refresh** (`--refresh`): Only deep-scans categories where total increased

### Known limits

- **Offset cap**: `offset + limit > 10000` → error `"too big query offset"`. Max safe offset 9900 with batch 100
- **Token expiry**: ~10h. Read from `UZUM_ACCESS_TOKEN` env
- **Rate limiting**: Sequential only. Concurrent >1 → 429
- Serde camelCase mismatch fixed (all gql structs need `#[serde(rename_all = "camelCase")]`)

### Output

`~/.local/share/uzum/uzum_data.jsonl` — JSON Lines with header line, then one raw `catalogCard` JSON object per line.

### State

`~/.local/share/uzum/state.json`:
```json
{"version":1,"categories":{"123":{"total":500,"offset":500}},"item_count":248668,"updated_at":"2026-07-26T12:00:00.000Z"}
```

### Performance

Celeron N4000, 100 Mbps — first 50 categories: 15,196 items in ~97s. Estimated full scan (1627 leaf categories): ~52 min, ~500K items. Per-category: ~0.5s to several seconds.

## Changelog

| Date | Change |
|------|--------|
| 2026-07-25 | Initial API investigation; collector architecture; query shapes |
| 2026-07-26 | Corrected to sequential (removed concurrent → 429); fixed serde camelCase + deliveryOptions type |
| 2026-07-27 | Merged into single WIKI.md |

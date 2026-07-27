# Uzum.uz — API Reference & Collector

> Sources: uzum.uz WIKI, 2026-07-27; uzum.uz AGENTS, 2026-07-27; uzum.uz API end-to-end analysis, 2026-07-25; uzum.uz live API investigation, 2026-07-25; uzum.uz live page snapshot, 2026-07-25
> Raw: [uzum.uz_WIKI.md](../../raw/collectors/uzum.uz_WIKI.md); [uzum.uz_AGENTS.md](../../raw/collectors/uzum.uz_AGENTS.md); [uzum-e2e-analysis.md](../../uzum.uz/raw/uzum-api/2026-07-25-api-end-to-end-analysis.md); [uzum-live-api.md](../../uzum.uz/raw/uzum-api/2026-07-25-live-api-investigation.md); [uzum-page-snapshot.txt](../../uzum.uz/raw/uzum-api/2026-07-25-live-page-snapshot.txt)
> Updated: 2026-07-27

## Overview

Uzum.uz is Uzbekistan's largest online supermarket. Its API uses GraphQL (Apollo) for product queries and REST for categories and user data. The collector binary (`uzum-watch`) scrapes the full catalog via GraphQL, writing raw `catalogCard` JSON pass-through to a local JSONL file with a header line.

## Infrastructure

- **Domain**: `uzum.uz` | **Web app**: `1.63.2` | **Build**: `884252`
- **API**: `graphql.uzum.uz` (GraphQL/Apollo), `api.uzum.uz` (REST)
- **CDN**: `images.uzum.uz` (products), `static.uzum.uz` (static assets)
- **Sentry**: `sentry.infra.cluster.daymarket.uz` (key `948fdc05a99a018e8d3ab003bee4b5a3`)
- **Analytics**: `customer-resources.uzum.uz` (events, performance)
- **Feature flags**: GrowthBook (`cdn.growthbook.io/api/features`, SDK `sdk-Ndq9uu11eUCoTda`)

## Common Headers

**REST**: `Authorization: Bearer <JWT>`, `x-iid: <installId>`, `accept-language: uz-UZ`, `sentry-trace`, `baggage`

**GraphQL** (adds):
- `apollographql-client-name: web-customers`
- `apollographql-client-version: 1.63.2` (homepage/category) or `1.34.6` (PDP)
- `city-id: 1`, `city-latitude`, `city-longitude`, `latitude`, `longitude`
- `x-context: null` (PDP only)

## REST Endpoints

| Endpoint | Purpose |
|----------|---------|
| `GET /api/main/root-categories?eco=false` | Full category tree (recursive `children`, `path` ancestors) |
| `GET /api/main/promo-categories` | Promotional category cards |
| `GET /api/popup/active?installId=&token=` | Active popups |
| `GET /api/user/purchases/preview` | Purchase preview (auth) |
| `GET /api/user/name` | Display name (auth) |
| `GET /api/user/contacts` | Contact info (auth) |
| `POST /api/main/cities/city-by-location` | Geo-location |

### Category node fields

`id`, `title`, `icon`, `iconSvg`, `iconLink`, `productAmount`, `adult`, `eco`, `seoMetaTag`, `seoHeader`, `children` (recursive), `path` (ancestor chain `[rootId, ..., categoryId]`)

## GraphQL

Single endpoint: `POST https://graphql.uzum.uz/`

### Homepage queries

`Cart_Summary` — cart state with `amount`, `sku` details
`getMainContent` — banners, promo blocks, carousels, today's deals (fragments: BannerBlock, PromoImageBlock, ExtendableOffer, ImageOffer, CarouselOffer, StyledCarouselOffer, TodayDealsCarouselOffer, PageableVerticalOfferBlock, InlineBanner, BannerGrid)
`FetchReviewForModal` — pending review modal

### Category page queries

**`MakeSearch_ItemsAndFilters`** — Product search with pagination, facets, fast categories
`$queryInput: MakeSearchQueryInput!`

```json
{
  "offerCategoryId": "10068",
  "showAdultContent": "TRUE",
  "filters": [],
  "sort": "BY_RELEVANCE_DESC",
  "pagination": {"offset": 0, "limit": 48},
  "correctQuery": false,
  "getFastCategories": true,
  "fastCategoriesLimit": 11,
  "fastCategoriesLevelOffset": 2,
  "getPromotionItems": true,
  "getFastFacets": false,
  "fastFacetsLimit": 0
}
```

Returns: `items[]` (each with `catalogCard`), `facets`, `fastFacets`, `fastCategories`, `total`, `category`, `bannersV2`, `todayDealOfferSelectors`, `permanentLinkSeo`, `token`, `queryText`, etc.

**`MakeSearch_Categories`** — Subcategory tree with product counts for sidebar

**`MakeSearch_ItemsAndFilters` (facets-only)** — `pagination: {limit: 0}` → metadata-only, no items

### PDP queries

`GetTabs($productId: Int!)` — product tabs (INSTRUCTION, DESCRIPTION, CHARACTERISTICS HTML)
`ProductPage($productId: Int!)` — installment widget
`Feedbacks($productPageId, $page, $size, $sort)` — paginated feedbacks
`FeedbackPhotos($productPageId)` — photo gallery
`RecommendationBlocks($query: RecommendationQueryInput!)` — similar products with ad data
`getViewedProducts($offset, $limit)` — recently viewed
`Suggestions($GetSuggestionsInput)` — search suggestions
`addViewedProduct($id: Int!)` — track page view (mutation)

### Notable differences from earlier assumptions

- Variable name is `$queryInput` (not `$input`), field is `offerCategoryId` (not `categoryId`)
- `getFastCategories` defaults to `true` in observed calls, `limit: 48` for category page UI (but API accepts `100`)
- `deliveryOptions` is a single object `{shortDate, stockType}`, not an array
- Sort modes: `BY_RELEVANCE_DESC`, `BY_ORDERS_NUMBER_DESC`

## CatalogCard (Product Card)

Union of `ProductCard` (single-SKU) and `SkuGroupCard` (multi-variant).

### Shared fragments

**ProductCard_Identity**: `id`, `productId`, `title`, `adult`
**ProductCard_Commerce**: `buyingOptions{isBestPrice, priceBlock{badges, sellPrice, finalPrice, fullPrice, sellerPrice, icon}}`, `discount{discountPrice}`, `minFullPrice`, `minSellPrice`, `promoFutureInfo{minFuturePrice, minFuturePriceDate}`, `carrierCode`
**ProductCard_Marketing**: `badges[]`, `offer{due, icon}`, `infoLabel{color, title}`
**ProductCard_Media**: `photos{key, link(trans: PRODUCT_540){high, low}}`
**ProductCard_Social**: `feedbackQuantity`, `rating`
**ProductCard_Checkout**: `buyingOptions{defaultSkuId, isSingleSku, deliveryOptions{shortDate, stockType}}`, `characteristicValues`

### PriceBlock

`badges[]` (types: StickerBadge with `iconLink`, FomoTimerBadge with `endDate`/`timerType`, BottomTextBadge, BottomIconTextBadge, UzumInstallmentTitleBadge), `sellPrice`, `finalPrice`, `fullPrice`, `sellerPrice` (each: `amountColor`, `amount`, `description`, `descriptionColor`), `icon`

### Badge types

| Type | ID | Purpose |
|------|-----|---------|
| BottomTextBadge | 424 | "ARZON NARX KAFOLATI" |
| UzumInstallmentTitleBadge | 41 | Installment monthly text |
| StickerBadge | 468 | "Aksiya" with icon |
| FomoTimerBadge | 469 | "Aksiya" countdown timer |
| BottomIconTextBadge | 421 | "ORIGINAL" with icon |

### Delivery

Stock types: `FBS` (seller), `FBO` (Uzum). `deliveryOptions` is a single object, not array.

### Image transformations

`https://images.uzum.uz/<key>/` + suffix:
- `t_product_540_high.jpg` / `_low.jpg` — standard search result (540px)
- `t_product_80_high.jpg` — purchase preview thumbnail
- `t_product_720_high.jpg` / `_low.jpg` — larger view (not confirmed)
- `feedback_40.jpg` — review images
- `main_page_banner.jpg` — homepage banners

## Collector: uzum-watch

Rust CLI tool. Single `src/main.rs`. Scrapes full Uzum catalog via GraphQL.

### Architecture

Single-threaded, sequential. Scans leaf categories one at a time. For each: send `MakeSearch_ItemsAndFilters` for first page, write products, paginate through remaining pages.

**Two modes**:
- **Full** (no args): Scans every leaf category from scratch. Creates new output with header line.
- **Refresh** (`--refresh`): Compares saved totals. Only deep-scans categories where total increased or collection incomplete. Appends.

### Known limits

- **Offset cap**: `offset + limit > 10000` → error `"too big query offset"`. Max safe offset 9900 with batch 100.
- **Token expiry**: ~10h. Read from `UZUM_ACCESS_TOKEN` env. Refresh via `id.uzum.uz/api/auth/token` POST.
- **Rate limiting**: Sequential only. Concurrent >1 → 429.
- Serde camelCase mismatch fixed (all gql structs need `#[serde(rename_all = "camelCase")]`)

### GraphQL query

```graphql
query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) {
  makeSearch(query: $queryInput) {
    items {
      catalogCard {
        productId title minFullPrice minSellPrice
        feedbackQuantity rating
        buyingOptions { isSingleSku deliveryOptions { shortDate stockType } }
        promoFutureInfo { minFuturePrice minFuturePriceDate }
        badges { id text backgroundColor textColor }
      }
    }
    total
  }
}
```

### Variables

```json
{
  "categoryId": "123", "showAdultContent": "TRUE",
  "filters": [], "sort": "BY_ORDERS_NUMBER_DESC",
  "pagination": {"offset": 0, "limit": 100},
  "correctQuery": false, "getFastCategories": false
}
```

### Output

`~/.local/share/uzum/uzum_data.jsonl` — JSON Lines with header:
```json
{"exportedAt":"2026-07-26T12:00:00.000Z","totalProducts":0,"version":"1.0.0","source":"uzum.uz"}
```

Each data line: raw `catalogCard` JSON pass-through.

### State

`~/.local/share/uzum/state.json`:
```json
{"version":1,"categories":{"123":{"total":500,"offset":500}},"item_count":248668,"updated_at":"2026-07-26T12:00:00.000Z"}
```

### Configuration

| Env | Description |
|-----|-------------|
| `UZUM_ACCESS_TOKEN` | JWT token for auth |
| `UZUM_INSTALL_ID` | Install ID from cookie |

### Performance (Celeron N4000, 100 Mbps)

- First 50 categories: 15,196 items in ~97s
- Estimated full scan (1627 leaf categories): ~52 min, ~500K items
- Per-category: ~0.5s (empty) to several seconds (thousands of products)

### Operational details

- **Data dir**: `~/.local/share/uzum/`
- **Lock**: `uzum.lock` (flock exclusive)
- **State writes**: atomic via `.tmp` + rename
- **Progress**: every 50 categories logs time + count + persists state

### Auth setup

```bash
cat > ~/.config/uzum/env << 'EOF'
UZUM_ACCESS_TOKEN=your_jwt_token
UZUM_INSTALL_ID=your_install_id
EOF
chmod 600 ~/.config/uzum/env
```

### Installation

```bash
cargo build --release
cp target/release/uzum-watch ~/.local/bin/

# systemd daily timer
# Service: uzum-watch.service (Type=oneshot, --refresh)
# Timer: uzum-watch.timer (OnCalendar=daily)
```

## See Also

- [OLX.uz Collector](../collectors/olx-uz.md) — parallel classifieds collector
- [BirBir.uz Collector](../collectors/birbir-uz.md) — parallel classifieds collector
- [Project Overview](../project.md) — overarching project goals

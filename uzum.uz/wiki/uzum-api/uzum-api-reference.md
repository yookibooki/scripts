# Uzum API Reference

> Sources: uzum.uz, 2026-07-25 (live API investigation); 2026-07-26 (live run & DevTools fact-check)
> Raw: [API End-to-End Analysis](../../raw/uzum-api/2026-07-25-api-end-to-end-analysis.md)
> Updated: 2026-07-26

## Overview

Comprehensive reference of the Uzum.uz marketplace API as observed from live browser traffic. Covers REST endpoints, GraphQL schema (observed queries), infrastructure, and data formats.

## Infrastructure

- **Web app version**: 1.63.2
- **Build**: Build-Number: 884252
- **Server**: ycalb
- **CDN**: images.uzum.uz (product images), static.uzum.uz (static assets)
- **Sentry**: sentry.infra.cluster.daymarket.uz (`sentry_key=948fdc05a99a018e8d3ab003bee4b5a3`)
- **Analytics**: customer-resources.uzum.uz (`POST /api/analytics/v2/events`, `POST /api/analytics/v2/system/performance`)
- **Feature flags**: cdn.growthbook.io/api/features (GrowthBook SDK key: sdk-Ndq9uu11eUCoTda)

## API Domains

| Domain | Purpose |
|--------|---------|
| `graphql.uzum.uz` | Primary GraphQL API (Apollo) |
| `api.uzum.uz` | REST API |
| `customer-resources.uzum.uz` | Analytics/telemetry |
| `images.uzum.uz` | Product images CDN |
| `static.uzum.uz` | Static assets |

## Common Request Headers

### REST
- `Authorization: Bearer <JWT>` (from `access_token` cookie)
- `x-iid: <installId>` (from `clickstream-client.installId` cookie)
- `accept-language: uz-UZ`
- `sentry-trace` / `baggage` for Sentry distributed tracing

### GraphQL (shared + additional)
- All REST headers (`Authorization`, `x-iid`, `accept-language`, `sentry-trace`, `baggage`) sent on GraphQL too
- `apollographql-client-name: web-customers`
- `apollographql-client-version: 1.63.2` (homepage/category) or `1.34.6` (PDP)
- `city-id: 1`
- `city-latitude`, `city-longitude`, `latitude`, `longitude` (on every request, not just PDP)
- `x-context: null` (PDP only)

## REST Endpoints

### `GET /api/main/root-categories`
- Returns full category tree with `payload` wrapper
- Parameters: `?eco=false` (observed on homepage and category page)
- Category node fields: `id`, `title`, `icon`, `iconSvg`, `iconLink`, `productAmount`, `adult`, `eco`, `seoMetaTag`, `seoHeader`, `children` (recursive), `path` (ancestor chain `[rootId, ..., categoryId]`)

### `GET /api/main/promo-categories`
- Returns promotional category cards: `id`, `title`, `subtitle`, `iconLink`, `deepLink`
- Response includes `"timestamp": "2026-07-26T05:52:18.648145364"`

### `GET /api/popup/active`
- Parameters: `installId`, `token` (JWT)
- Returns active popup content or empty body

### `GET /api/user/purchases/preview`
- Authenticated: user purchase preview
- Returns `{"payload":{"title":"Xarid qilgansiz","subTitle":"Qaytadan buyurtma qilmoqchi bo‘lsangiz","imageUrls":[...]}}`

### `GET /api/user/name`
- Authenticated: user display name

### `GET /api/user/contacts`
- Authenticated: user contact info

### `POST /api/main/cities/city-by-location`
- Geo-location endpoint called on page load

## GraphQL Endpoint

Single endpoint: `POST https://graphql.um.uz/`

### Homepage Queries

**Cart_Summary** — `query Cart_Summary { cart { ...CartSummaryFragment } }`
- Cart state with `amount`, `sku` (id, availableAmount, product id)
- Response shape: `{"data":{"cart":[]}}` (empty array when no items)

**getMainContent** — `query getMainContent($type: DisplayType!, $page: Int!, $size: Int!, $offerSize: Int!, $offset: Int!, $rowWidth: Int!)`
- Complex homepage: banners, promo blocks, carousels, today's deals
- Variables: `{"type":"DESKTOP","page":0,"size":10,"offerSize":10,"offset":0,"rowWidth":5}`
- Fragment types: BannerBlock (sub-type TopBanner), PromoImageBlock, ExtendableOffer, ImageOffer, CarouselOffer, StyledCarouselOffer, TodayDealsCarouselOffer, PageableVerticalOfferBlock, InlineBanner, BannerGrid
- ProductCard fragments include full `PriceBlock` with `badges`, `sellPrice`, `finalPrice`, `fullPrice`, `sellerPrice`, `icon`

**FetchReviewForModal** — `query FetchReviewForModal { reviewModal { ... } }`
- Pending review modal data
- Response shape: `{"data":{"reviewModal":[]}}` (empty array when nothing pending)

### Category Page Queries

**MakeSearch_ItemsAndFilters** — `query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!)`
- Product search with pagination, facets, fast categories, banners
- Returns: `items` (each with `catalogCard` + `cpoAdvVersion`/`cpoId`/`bidId`), `facets`, `fastFacets`, `fastCategories`, `total`, `category`, `bannersV2`, `todayDealOfferSelectors`, `permanentLinkSeo`, `token`, `queryText`, `mayHaveAdultContent`, `categoryFullMatch`, `offerCategory`, `correctedQueryText`, `categoryWasPredicted`

Input variable is `$queryInput`, whose fields are:

```json
{
  "offerCategoryId": "10068",
  "showAdultContent": "TRUE",
  "filters": [],
  "sort": "BY_RELEVANCE_DESC",
  "pagination": { "offset": 0, "limit": 48 },
  "correctQuery": false,
  "getFastCategories": true,
  "fastCategoriesLimit": 11,
  "fastCategoriesLevelOffset": 2,
  "getPromotionItems": true,
  "getFastFacets": false,
  "fastFacetsLimit": 0
}
```

Key differences from earlier assumptions:
- **Variable name**: `$queryInput` (not `$input`) — top-level variable is `$queryInput: MakeSearchQueryInput!`
- **Field name**: `offerCategoryId` (not `categoryId`)
- **`getFastCategories` is `true`** in all observed calls
- **`limit: 48`** for category page, but the scraper uses `limit: 100` (works but bypasses UI convention)

Sort modes: `BY_RELEVANCE_DESC`, `BY_ORDERS_NUMBER_DESC`

**MakeSearch_Categories** — `query MakeSearch_Categories($queryInput: MakeSearchQueryInput!)`
- `makeSearch { categoryTree { category { ...CategoryFragment } total } }`
- Returns subcategory tree with product counts for the sidebar

**MakeSearch_ItemsAndFilters (facets-only)** — same query with `pagination: { offset: 0, limit: 0 }`
- Returns `items: []` but full facets/fastCategories — effectively "metadata-only" mode

### Product Detail Page Queries

**GetTabs** — `query GetTabs($productId: Int!) { product(id: $productId) { comments { commentType type value } } }`
- Product tabs: INSTRUCTION, DESCRIPTION, CHARACTERISTICS (HTML content)

**ProductPage** — `query ProductPage($productId: Int!) { productPage(id: $productId) { installmentWidget { ... } } }`
- Installment/payment plan widget per SKU

**Feedbacks** — `query Feedbacks($productPageId: Int!, $page: Int!, $size: Int!, $sort: FeedbackSortType!)`
- Paginated feedbacks with customer info, rating, photos, shop reply, SKU details

**FeedbackPhotos** — `query FeedbackPhotos($productPageId: Int!) { productPage(id: $productPageId) { feedbacksByPhotoInfo { photosCount photoInfos } } }`

**RecommendationBlocks** — `query RecommendationBlocks($query: RecommendationQueryInput!)`
- Similar products recommendations with ad data (cpoId, cpoVersion)

**getViewedProducts** — `query getViewedProducts($offset: Int!, $limit: Int!) { recentlyViewedProducts { total items { ...ProductCardFragment } } }`

**Suggestions** — `query Suggestions($GetSuggestionsInput: GetSuggestionsInput!) { getSuggestions { blocks { ... } } }`
- Search suggestions: popular, text, category, shop, recommended

**addViewedProduct** — `mutation addViewedProduct($id: Int!) { addRecentlyViewedProduct(id: $id) }`
- Track product page view

## CatalogCard (Product Card)

`CatalogCard` is the GraphQL union type for product cards. It can be:
- **`ProductCard`** — single-SKU product with fields: `carrierCode`, `cpoId`, `cpoVersion`
- **`SkuGroupCard`** — multi-variant product with `characteristicValues` — MORE COMMON in search results

Shared fragments (all on `CatalogCard`):

```graphql
ProductCard_Identity:  id, productId, title, adult
ProductCard_Commerce:  buyingOptions { isBestPrice, priceBlock { badges, sellPrice, finalPrice, fullPrice, sellerPrice, icon } }, discount { discountPrice }, minFullPrice, minSellPrice, promoFutureInfo { minFuturePrice, minFuturePriceDate }, carrierCode
ProductCard_Marketing:  badges [], offer { due, icon }, infoLabel { color, title }
ProductCard_Media:     photos { key, link(trans: PRODUCT_540) { high, low } }
ProductCard_Social:    feedbackQuantity, rating
ProductCard_Checkout:  buyingOptions { defaultSkuId, isSingleSku, deliveryOptions { shortDate, stockType } }, characteristicValues
```

**Important**: `carrierCode` appears on both `ProductCard` and `SkuGroupCard` via inline fragments in `ProductCard_Commerce`. `cpoId`/`cpoVersion` only on `ProductCard`. `characteristicValues` only on `SkuGroupCard`.

### PriceBlock

```graphql
fragment PriceBlockFragment on PriceBlock {
  badges {
    id text textColor backgroundColor
    ... on StickerBadge { iconLink }
    ... on FomoTimerBadge { endDate timerType }
  }
  sellPrice { ...priceFragment }
  finalPrice { ...priceFragment }
  fullPrice { ...priceFragment }
  sellerPrice { ...priceFragment }
  icon
}
fragment priceFragment on Price {
  amountColor amount description descriptionColor
}
```

### Delivery

Delivery stock types observed: `FBS` (fulfilled by seller), `FBO` (fulfilled by Uzum)

**`deliveryOptions` is a single object** (`{ shortDate, stockType }`), not an array.

### Badge Types

| Type | ID | Purpose |
|------|-----|---------|
| BottomTextBadge | 424 | "ARZON NARX KAFOLATI" (low price guarantee) |
| UzumInstallmentTitleBadge | 41 | Installment monthly payment text |
| StickerBadge | 468 | "Aksiya" (promotion) with iconLink |
| FomoTimerBadge | 469 | "Aksiya" with countdown timer (`endDate`, `timerType`) |
| BottomIconTextBadge | 421 | "ORIGINAL" badge with iconLink |

## Image Transformations

Base URL: `https://images.uzum.uz/<key>/`

| Suffix | Purpose |
|--------|---------|
| `t_product_540_high.jpg` / `_low.jpg` | Product card (540px) — standard search result |
| `t_product_80_high.jpg` | Purchase preview thumbnail |
| `t_product_720_high.jpg` / `_low.jpg` | Larger product view (not confirmed) |
| `feedback_40.jpg` | Feedback/review images |
| `main_page_banner.jpg` | Homepage banners |

**Note**: Products use `link(trans: PRODUCT_540)` in GraphQL, which maps to `t_product_540_high.jpg`.

## Response Security Headers

- `access-control-allow-origin: *` (GraphQL) or `access-control-allow-origin: https://uzum.uz` (REST, also allows cookies)
- `access-control-allow-credentials: true` (REST only)
- `access-control-allow-headers: ...` extensive allowlist (REST)
- `strict-transport-security: max-age=31536000 ; includeSubDomains`
- `x-content-type-options: nosniff`
- `x-frame-options: DENY`
- `x-xss-protection: 1; mode=block` (REST) or `0` (GraphQL)
- `set-cookie: _yasc=...` (session, SameSite=None, Secure, domain=.api.uzum.uz or .graphql.uzum.uz)
- `build-info: Build-Number: 884252; Commit:` (REST)

## Observations

- Single GraphQL endpoint for all queries — no batching per page load
- Category page UI limit is 48 items, but the GraphQL API accepts `limit: 100` (and even `limit: 0` for metadata-only)
- PDP uses older client version `1.34.6` vs `1.63.2` for other pages
- `x-context: null` only on PDP requests
- `root-categories` called on every page navigation
- `city-by-location` called on every page load
- `sentry-trace` / `baggage` headers sent on both REST and GraphQL requests
- No single product detail query — split into GetTabs, ProductPage, Feedbacks, RecommendationBlocks
- `access-control-allow-origin: *` on GraphQL (not restricted to uzum.uz)

# Uzum API Reference

> Sources: uzum.uz, 2026-07-25 (live API investigation); 2026-07-26 (live run verification)
> Raw: [API End-to-End Analysis](../../raw/uzum-api/2026-07-25-api-end-to-end-analysis.md)
> Updated: 2026-07-26

## Overview

Comprehensive reference of the Uzum.uz marketplace API as observed from live browser traffic. Covers REST endpoints, GraphQL schema (observed queries), infrastructure, and data formats.

## Infrastructure

- **Web app version**: 1.63.2
- **Build**: Build-Number: 884252
- **Server**: ycalb
- **CDN**: images.uzum.uz (product images), static.uzum.uz (static assets)
- **Sentry**: sentry.infra.cluster.daymarket.uz
- **Analytics**: customer-resources.uzum.uz (events + system/performance)
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

### GraphQL (additional)
- `apollographql-client-name: web-customers`
- `apollographql-client-version: 1.63.2` (homepage/category) or `1.34.6` (PDP)
- `city-id: 1`
- `city-latitude`, `city-longitude`, `latitude`, `longitude`
- `x-context: null` (PDP only)

## REST Endpoints

### `GET /api/main/root-categories`
- Returns full category tree with `payload` wrapper
- Parameters: `?eco=false` (observed on homepage and category page)
- Category node fields: `id`, `title`, `icon`, `iconSvg`, `iconLink`, `productAmount`, `adult`, `eco`, `seoMetaTag`, `seoHeader`, `children` (recursive), `path` (ancestor chain `[rootId, ..., categoryId]`)

### `GET /api/main/promo-categories`
- Returns promotional category cards: `id`, `title`, `subtitle`, `iconLink`, `deepLink`
- Response includes `timestamp`

### `GET /api/popup/active`
- Parameters: `installId`, `token` (JWT)
- Returns active popup content or empty body

### `GET /api/user/purchases/preview`
- Authenticated: user purchase preview

### `GET /api/user/name`
- Authenticated: user display name

### `GET /api/user/contacts`
- Authenticated: user contact info

## GraphQL Endpoint

Single endpoint: `POST https://graphql.uzum.uz/`

### Homepage Queries

**Cart_Summary** — `query Cart_Summary { cart { ...CartSummaryFragment } }`
- Cart state with `amount`, `sku` (id, availableAmount, product id)

**getMainContent** — `query getMainContent($type: DisplayType!, $page: Int!, $size: Int!, $offerSize: Int!, $offset: Int!, $rowWidth: Int!)`
- Complex homepage: banners, promo blocks, carousels, today's deals
- Fragment types: BannerBlock, PromoImageBlock, ExtendableOffer, ImageOffer, CarouselOffer, StyledCarouselOffer, TodayDealsCarouselOffer, PageableVerticalOfferBlock, InlineBanner, BannerGrid

**FetchReviewForModal** — `query FetchReviewForModal { reviewModal { ... } }`
- Pending review modal data

### Category Page Queries

**MakeSearch_ItemsAndFilters** — `query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!)`
- Product search with pagination, facets, fast categories, banners
- Returns: `items` (ProductCardFragment), `facets`, `fastFacets`, `fastCategories`, `total`, `category`, `bannersV2`, `todayDealOfferSelectors`, `permanentLinkSeo`, `token`

Variables:
```json
{
  "categoryId": "10068",
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

Sort modes: `BY_RELEVANCE_DESC`, `BY_ORDERS_NUMBER_DESC`

**MakeSearch_Categories** — `query MakeSearch_Categories($queryInput: MakeSearchQueryInput!)`
- `makeSearch { categoryTree { category { ...CategoryFragment } total } }`
- Returns subcategory tree with product counts for the sidebar

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

## ProductCardFragment

Shared fragment across all GraphQL queries. Five sub-fragments:

```graphql
ProductCard_Identity:  id, productId, title, adult
ProductCard_Commerce:  buyingOptions { isBestPrice, priceBlock { badges, sellPrice, finalPrice, fullPrice, sellerPrice, icon } }, discount { discountPrice }, minFullPrice, minSellPrice, promoFutureInfo { minFuturePrice, minFuturePriceDate }, carrierCode
ProductCard_Marketing: badges [], offer { due, icon }, infoLabel { color, title }
ProductCard_Media:     photos { key, link { high, low } }
ProductCard_Social:    feedbackQuantity, rating
ProductCard_Checkout:  buyingOptions { defaultSkuId, isSingleSku, deliveryOptions { shortDate, stockType } }, characteristicValues
```

Delivery stock types: `FBS` (fulfilled by seller), `FBO` (fulfilled by uzum)

**Note:** `deliveryOptions` is a single `DeliveryOptions` object (`{ shortDate, stockType }`), not an array. The collector previously typed it as `Vec<DeliveryOptions>` which caused deserialization failures.

### Badge Types

| Type | ID | Purpose |
|------|-----|---------|
| BottomTextBadge | 424 | "ARZON NARX KAFOLATI" (low price guarantee) |
| UzumInstallmentTitleBadge | 41 | Installment monthly payment text |
| StickerBadge | 468 | "Aksiya" (promotion) with icon |
| FomoTimerBadge | 469 | "Aksiya" with countdown timer |

## Image Transformations

Base URL: `https://images.uzum.uz/<key>/`

| Suffix | Purpose |
|--------|---------|
| `t_product_540_high.jpg` / `_low.jpg` | Product card (540px) |
| `t_product_720_high.jpg` / `_low.jpg` | Larger product view |
| `feedback_40.jpg` | Feedback/ review images |
| `main_page_banner.jpg` | Homepage banners |

## Response Security Headers

- `access-control-allow-origin: https://uzum.um`
- `strict-transport-security: max-age=31536000 ; includeSubDomains`
- `x-content-type-options: nosniff`
- `x-frame-options: DENY`
- `x-xss-protection: 0`
- `set-cookie: _yasc=...` (session, SameSite=None, Secure, domain=.api.uzum.uz or .graphql.uzum.uz)

## Observations

- Single GraphQL endpoint for all queries — no batching per page load
- Category page limit is 48 (vs collector's 100 batch size)
- PDP uses older client version 1.34.6 vs 1.63.2 for other pages
- `x-context: null` only on PDP requests
- `root-categories` called on every page navigation
- No single product detail query — split into GetTabs, ProductPage, Feedbacks, RecommendationBlocks

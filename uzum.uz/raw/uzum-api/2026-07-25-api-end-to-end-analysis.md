# API End-to-End Analysis — Uzum.uz Marketplace

> Source: Live browser investigation via Chrome DevTools MCP while logged into uzum.uz
> Collected: 2026-07-25
> Published: Unknown

## Method

Live browser investigation with authenticated session. Captured all HTTP requests across three page types: homepage, category listing, and product detail page (PDP).

## Infrastructure

- **Web app version**: 1.63.2 (matches `apollographql-client-version` header)
- **Build**: Build-Number: 884252
- **Server**: ycalb
- **CDN**: images.uzum.uz (product images), static.uzum.uz (static assets)
- **Sentry**: sentry.infra.cluster.daymarket.uz (APM/error tracking)
- **Analytics**: customer-resources.uzum.uz/api/analytics/v2/events and /system/performance
- **Feature flags**: cdn.growthbook.io/api/features (GrowthBook SDK)
- **Auth**: JWT-based via `access_token` cookie, `clickstream-client.installId` cookie for install ID

## API Domains

| Domain | Purpose |
|--------|---------|
| `graphql.uzum.uz` | Primary GraphQL API (Apollo) |
| `api.uzum.uz` | REST API |
| `customer-resources.uzum.uz` | Analytics/telemetry |
| `images.uzum.uz` | Product images CDN |
| `static.uzum.uz` | Static assets (CSS/JS) |
| `sentry.infra.cluster.daymarket.uz` | Sentry error tracking |

## Common Request Headers (live capture)

### REST requests
- `Authorization: Bearer <JWT>` (from `access_token` cookie)
- `x-iid: <installId>` (from `clickstream-client.installId` cookie)
- `accept-language: uz-UZ`
- `origin: https://uzum.uz`
- `referer: https://uzum.uz/`
- Sentry tracing: `sentry-trace` and `baggage` headers

### GraphQL requests
All REST headers plus:
- `apollographql-client-name: web-customers`
- `apollographql-client-version: 1.63.2` (homepage, category page) or `1.34.6` (product page)
- `city-id: 1`
- `city-latitude`, `city-longitude`, `latitude`, `longitude` (geolocation)
- `x-context: null` (on product page)
- Note: product page uses older client version `1.34.6`

## Homepage (`/uz/`)

### REST Endpoints

**GET /api/main/root-categories?eco=false**
- Returns full category tree with `payload` wrapper
- Each category node now includes `productAmount`, `adult`, `eco`, `seoMetaTag`, `seoHeader`, `path` (ancestor chain), `iconSvg`
- Key change from earlier: query param `?eco=false` added

**GET /api/main/promo-categories**
- Returns promotional category cards (icon + deep link)
- Example: "Arzon narxlar kafolati", "Maishiy texnika"

**GET /api/user/purchases/preview**
- Authenticated: returns user purchase preview

**GET /api/popup/active?installId=...&token=...**
- Authenticated: returns active popup content (empty when none active)

### GraphQL Queries (homepage)

1. **Cart_Summary** — `query Cart_Summary { cart { ...CartSummaryFragment } }`
   - Returns cart state: `amount`, `sku` (id, availableAmount, product id)
   - Empty cart when not logged in

2. **getMainContent** — `query getMainContent($type: DisplayType!, $page: Int!, $size: Int!, $offerSize: Int!, $offset: Int!, $rowWidth: Int!)`
   - Complex main page content: banners, promo blocks, carousels, today's deals
   - Uses `ProductCardFragment` which returns full product data including:
     - `buyingOptions { isBestPrice, priceBlock { badges, sellPrice, finalPrice, fullPrice, sellerPrice } }`
     - `discount`, `minFullPrice`, `minSellPrice`, `promoFutureInfo`
     - `badges` (multiple types: BottomTextBadge, UzumInstallmentTitleBadge, StickerBadge, FomoTimerBadge)
     - `offer`, `infoLabel`, `photos`, `feedbackQuantity`, `rating`

3. **FetchReviewForModal** — `query FetchReviewForModal { reviewModal { ... } }`
   - Returns review modal data (empty array when no pending review)

## Category Page (`/uz/category/<slug>-<id>`)

### REST Endpoints
Same as homepage: root-categories, promo-categories, popup/active

### GraphQL Queries

1. **Cart_Summary** — same as homepage

2. **MakeSearch_ItemsAndFilters** — `query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!)`
   - Main product search with pagination
   - Variables differ from the simple collector version:
     - `getFastCategories: true`, `fastCategoriesLimit: 11`, `fastCategoriesLevelOffset: 2`
     - `getPromotionItems: true`, `getFastFacets: false`, `fastFacetsLimit: 0`
     - Sort: `BY_RELEVANCE_DESC` (not `BY_ORDERS_NUMBER_DESC` as in the collector)
   - Returns much richer data:
     - `category` with parent info, SEO, localized titles
     - `items` with full `ProductCardFragment` (buyingOptions, badges, photos, etc.)
     - `facets` and `fastFacets` for filtering
     - `fastCategories` for quick subcategory navigation
     - `permanentLinkSeo` for SEO permanent links
     - `bannersV2` (Promo, Image, Countdown types)
     - `todayDealOfferSelectors` for deals
     - `token` for pagination
     - `total`, `correctedQueryText`, `categoryFullMatch`, `offerCategory`, `mayHaveAdultContent`

3. **MakeSearch_Categories** — `query MakeSearch_Categories($queryInput: MakeSearchQueryInput!)`
   - Returns `categoryTree` with subcategories and product counts per category
   - Uses `CategoryFragment` with localized titles, SEO, `adult` flag
   - Query: `makeSearch { categoryTree { category { ...CategoryFragment } total } }`

## Product Detail Page (`/uz/product/<slug>-<id>`)

### REST Endpoints
- `GET /api/user/name` — authenticated user name
- `GET /api/user/contacts` — authenticated user contacts
- `GET /api/main/root-categories` — note: without `?eco=false` param on PDP!
- (Previously also on homepage and category pages)

### GraphQL Queries

1. **Cart_Summary** — same pattern

2. **GetTabs** — `query GetTabs($productId: Int!) { product(id: $productId) { comments { commentType type value } } }`
   - Returns product tabs: INSTRUCTION, DESCRIPTION, CHARACTERISTICS as HTML content

3. **ProductPage** — `query ProductPage($productId: Int!) { productPage(id: $productId) { installmentWidget { ... } } }`
   - Returns installment/payment plan widget data
   - Fields: title, subtitle, icon, link, lockedIcon, userStatus, calculationsPairs (per SKU)

4. **Feedbacks** — `query Feedbacks($productPageId: Int!, ...) { productPage(id: $productPageId) { sortedFeedbacks { feedbacks { ... } } } }`
   - Paginated feedbacks sorted by relevance
   - Each feedback: anonymous, content, customerName, dateCreated, rating, pros/cons, photos, reply from shop
   - SKU info: characteristicValues, prices

5. **FeedbackPhotos** — `query FeedbackPhotos($productPageId: Int!) { productPage(id: $productPageId) { feedbacksByPhotoInfo { photosCount photoInfos { text photo } } } }`
   - Feedback photos summary

6. **RecommendationBlocks** — `query RecommendationBlocks($query: RecommendationQueryInput!) { recommendationBlocks(query: $query) { ... } }`
   - Returns "Similar products" recommendations (offerId 2)
   - Also includes ad data: `cpoId`, `cpoVersion` for promoted items

7. **getViewedProducts** — `query getViewedProducts { recentlyViewedProducts { total items { ...ProductCardFragment } } }`
   - Recently viewed products

8. **Suggestions** — `query Suggestions($GetSuggestionsInput: GetSuggestionsInput!) { getSuggestions(query: $...) { blocks { ... } } }`
   - Search suggestions: popular, text, category, shop, recommended product suggestions

9. **addViewedProduct** — `mutation addViewedProduct($id: Int!) { addRecentlyViewedProduct(id: $id) }`
   - Mutation to track product view

## ProductCardFragment (shared fragment)

Used across all GraphQL queries for product data. Fields:

```graphql
fragment ProductCardFragment on CatalogCard {
  ...ProductCard_Identity     # id, productId, title, adult
  ...ProductCard_Commerce     # buyingOptions, discount, minFullPrice, minSellPrice, promoFutureInfo
  ...ProductCard_Marketing    # badges, offer, infoLabel
  ...ProductCard_Media        # photos (key, link high/low)
  ...ProductCard_Social       # feedbackQuantity, rating
  ...ProductCard_Checkout     # buyingOptions (defaultSkuId, isSingleSku, deliveryOptions), characteristicValues
}
```

Badge types observed:
- **BottomTextBadge** (id=424): "ARZON NARX KAFOLATI" (low price guarantee)
- **UzumInstallmentTitleBadge** (id=41): installment monthly payment
- **StickerBadge** (id=468): "Aksiya" (promotion) with icon
- **FomoTimerBadge** (id=469): "Aksiya" with timer (endDate as epoch ms)

Stock types: `FBS` (fulfilled by seller), `FBO` (fulfilled by uzum)

## MakeSearch Query — Full Variable Set

```graphql
variables: {
  queryInput: {
    categoryId: "10068",
    showAdultContent: "TRUE",
    filters: [],
    sort: "BY_RELEVANCE_DESC",  # or BY_ORDERS_NUMBER_DESC for collector
    pagination: { offset: 0, limit: 48 },  # web uses 48, collector uses 100
    correctQuery: false,
    getFastCategories: true,
    fastCategoriesLimit: 11,
    fastCategoriesLevelOffset: 2,
    getPromotionItems: true,
    getFastFacets: false,
    fastFacetsLimit: 0
  }
}
```

Sort modes observed:
- `BY_RELEVANCE_DESC` (category page default)
- `BY_ORDERS_NUMBER_DESC` (used by collector)

## Image Transformations

Product images are served via `images.uzum.uz` with transformation path:
- `t_product_540_high.jpg` / `t_product_540_low.jpg` — product card (540px)
- `t_product_720_high.jpg` / `t_product_720_low.jpg` — larger product view
- `feedback_40.jpg` — feedback images
- `main_page_banner.jpg` — homepage banners

## Response Headers (security)

Common security headers on REST API responses:
- `access-control-allow-origin: https://uzum.uz`
- `strict-transport-security: max-age=31536000 ; includeSubDomains`
- `x-content-type-options: nosniff`
- `x-frame-options: DENY`
- `x-xss-protection: 0`
- `set-cookie: _yasc=...` (session cookie, SameSite=None, Secure)

## Observations

- GraphQL endpoint is a single `POST https://graphql.uzum.uz/` - no batching observed per page load
- Category page limit is 48 (vs collector's 100 batch size)
- Product page uses an older apollographql-client-version (1.34.6) vs homepage/category (1.63.2)
- `x-context: null` header only present on product page requests
- The `root-categories` endpoint is called on every page navigation (with and without `?eco=false`)
- GrowthBook feature flag service is used (sdk-Ndq9uu11eUCoTda key)
- No product detail GraphQL query returns full product data in a single call - instead it's split into multiple small queries (GetTabs, ProductPage, Feedbacks, etc.)

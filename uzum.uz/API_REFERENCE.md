# Uzum Marketplace API Reference

> API endpoints, GraphQL schema, auth, data model, and known quirks for `uzum.uz` (Uzbekistan's largest marketplace).

## Base URLs

| Service  | URL                              |
|----------|----------------------------------|
| GraphQL  | `https://graphql.uzum.uz/`       |
| REST     | `https://api.uzum.uz/api`        |

## Authentication

Two cookies drive API auth:

| Cookie                        | Header          | Scope          |
|-------------------------------|-----------------|----------------|
| `access_token`                | `Authorization: Bearer <token>` | GraphQL + REST |
| `clickstream-client.installId`| `X-Iid: <installId>`            | GraphQL only   |

GraphQL requests also send:

```
apollographql-client-name: web-customers
apollographql-client-version: 1.34.6
city-id: 1
```

The `access_token` is a standard JWT obtained by logging into `uzum.uz` in the browser. Without it, some categories (food, cosmetics, pharmacy, adult) return totals but return 0 items — the API still works, just restricted.

`X-Iid` is the raw cookie value, cleaned of surrounding double-quotes (`"` → stripped).

## REST Endpoints

### Get Category Tree

```
GET /api/main/root-categories
```

Returns the full nested category tree in `{ payload: [...] }`.

- No pagination — returns everything in one response (~400 KB).
- Each node has: `id`, `title`, `slug`, `image`, `children` (recursive).
- **Leaf categories** have no `children` (empty array or absent — the code checks `!n.children || !n.children.length`).
- Known: passing `?eco=false` returns HTTP 400. Do not use query params.

**Response shape (each node):**

```json
{
  "id": 75,
  "title": "Yuz parvarishi",
  "slug": "uz-parvarishi",
  "image": null,
  "children": [
    {
      "id": 76,
      "title": "Yuz terisini tozalash",
      "slug": "uz-terisini-tozalash",
      "image": null,
      "children": []
    }
  ]
}
```

## GraphQL

All queries hit `POST https://graphql.uzum.uz/` with `Content-Type: application/json` and auth headers (see above).

### MakeSearch_ItemsAndFilters

The primary product search query. Used for both scanning and total counts.

**Query:**

```graphql
query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) {
  makeSearch(query: $queryInput) {
    items {
      catalogCard {
        id
        productId
        title
        adult
        minFullPrice
        minSellPrice
        feedbackQuantity
        rating
        discount {
          discountPrice
        }
      }
    }
    total
  }
}
```

**Variables:**

```json
{
  "queryInput": {
    "categoryId": "12345",
    "showAdultContent": "TRUE",
    "filters": [],
    "sort": "BY_ORDERS_NUMBER_DESC",
    "pagination": {
      "offset": 0,
      "limit": 100
    },
    "correctQuery": false,
    "getFastCategories": false
  }
}
```

- `categoryId`: string, one of the leaf category IDs from the category tree.
- `showAdultContent`: always `"TRUE"` (string, not boolean).
- `sort`: `"BY_ORDERS_NUMBER_DESC"` — most-ordered first. This is the only sort used.
- `pagination.offset`: zero-based. See **Offset Limit** below.
- `pagination.limit`: max **100** per page. The code uses 100.
- `filters`: empty array `[]` — no filters applied.
- `correctQuery` / `getFastCategories`: always `false`.

**Response shape:**

```json
{
  "data": {
    "makeSearch": {
      "items": [
        {
          "catalogCard": {
            "id": "some-uuid",
            "productId": 123456,
            "title": "Product Name",
            "adult": false,
            "minFullPrice": 100000,
            "minSellPrice": 80000,
            "feedbackQuantity": 42,
            "rating": 4.5,
            "discount": {
              "discountPrice": 80000
            }
          }
        }
      ],
      "total": 500
    }
  }
}
```

- `total`: total number of products in this category (integer).
- `items[n].catalogCard.id`: UUID string (different from `productId`).
- `items[n].catalogCard.productId`: numeric product ID (primary key for storage).
- `items[n].catalogCard.minFullPrice`: original price (in UZS sum).
- `items[n].catalogCard.minSellPrice`: current/sale price.
- `items[n].catalogCard.adult`: boolean, adult content flag.
- `items[n].catalogCard.rating`: float or null.
- `items[n].catalogCard.feedbackQuantity`: review count (integer).

## Data Model (Products in IndexedDB)

Stored in `products` object store, keyed by `productId`.

```json
{
  "id": 123456,
  "title": "Product Name",
  "price": 80000,
  "oldPrice": 100000,
  "discountPercent": 20,
  "rating": 4.5,
  "reviewCount": 42,
  "category": "Kategoriya nomi",
  "categoryId": 12345,
  "firstSeen": "2026-07-25T12:00:00.000Z",
  "lastSeen": "2026-07-25T12:00:00.000Z"
}
```

After **slimming** (runs on startup), these fields are stripped:
- `url` — product page URL
- `images` — image array
- `priceHistory` — price change history

## Known Quirks & Limits

### Offset Limit (Critical)

GraphQL rejects offsets where `offset + limit > 10000` with `"too big query offset"`. T

| Batch Size | Max Safe Offset |
|------------|----------------|
| 48         | 9951           |
| 100        | 9901           |

Categories with >10K products are **truncated** to the first ~10K items. Three known large categories:

| Category            | Total Items | Collected |
|---------------------|-------------|-----------|
| Makiyaj             | ~21,000     | ~10,000   |
| Psixologiya         | ~19,000     | ~10,000   |
| Badiiy adabiyot     | ~15,000     | ~10,000   |

### Categories That Return 0 Items (Auth-Restricted)

~85 categories (as of 2026-07-25) consistently fail at offset 0 — the API returns a `total` count but `items` is empty. Observed categories:

**Personal care & cosmetics:** Soch uchun, Yuz parvarishi, Makiyaj, Tana parvarishi, Soqol olish, Depilyatsiya, Manikyur/pedikyur, Gel-laklar, Laklar, Tirnoqlar dizayni, Soch kosmetika to'plamlari, Pariklar, Soch aksessuarlari, Sartarosh asboblari, Soch bo'yash vositalari, etc.

**Perfumery:** Erkaklar parfyumlari, Atirlar, Parfyumlangan suv, Xushbo'ylangan suv, Yog'li atirlar, Quruq atirlar, Odekolonlar, Miniatyuralar, Parfyumeriya to'plami, Atomayzerlar, etc.

**Food & drinks:** Qahva/sutli ichimliklar, Choy, Souslar, Ziravorlar, Konfetlar, Yog'lar, Pechenye, Shokolad, Makaronlar, Yormalar, Konservalar, Gazlangan ichimliklar, Sharbatlar, Energetik ichimliklar, Suv, Yongoqlar, Asal, Murabbo, Chipslar, Qurt, etc.

**Household cleaning:** Kir yuvish kukunlari/gellari/kapsulalari, Konditsionerlar, Oqartirgichlar, Idish yuvish, Oyna tozalash, Pol parvarishi, Tualet qog'ozi, Salfetkalar, Havo tozalagichlar, Repelentlar, Insektitsidlar, etc.

**Tools & hardware:** Elektr asboblar, Kalitlar, Otvyotkalar, Arralash asboblari, Payvandlash, Nasoslar, Santexnika, Yoritish, Lampochkalar, Batareyalar, Quyosh panellari, etc.

**Home decor & stationery:** Pardalar, Gilamlar, Ko'rpalar, Yostiqlar, Adyollar, Sochiqlar, Ko'zgular, Rasmlar, Gullar, Shamlar, Daftarlar, Bloknotlar, Qog'oz, etc.

### Categories That Fail Mid-Scan (Partial Data)

~18 categories fail after collecting some pages (offset > 0). Observed offsets:

**Clothing & footwear:** Paypoqlar, pinetkalar (offset 677), Ko'ylaklar va sarafanlar (2850), Shimlar va ishtonlar (489), Yangi tug'ilgan chaqaloqlar uchun qalpoqchalar (579), Bodi va kombinezonlar (1809), Kostyumlar va to'plamlar (1355), Futbolkalar va polo (9257), Uy kiyimi (7577), Ko'ylaklar va yubkalar (196), Futbolkalar va polo (2002), Paypoq va kolgotkilar (1944), Ichki kiyim (162), Shlepanslar va slanslar (1082), Futbolkalar va maykalar (2777), Kiyim to'plamlari (6957), Futbolkalar va polo (7246), Sliponlar (829), Kiyim to'plamlari (4480), Sandalilar (221), Sport kiyimlari (3297), Mokasinlar (574), Sabo (138), Kiyim to'plamlari (5709), Funktsional poyabzal (87), Uy shippaklari (283), Topsayderlar (6), Uy shippaklari (867), Baletkalar (1011).

**Home & kitchen:** Ichimlik uchun aksessuarlar (200), Kontakt linzalar (800), Ovqat pishirish uchun idishlar (500), Pazandachilik aksessuarlari (500), Sochiqlar (100), Tana uchun massaj moslamalari (1100), Bir martali ishlatiladigan idishlar (600), Choyshablar (200), Oziq-ovqat mahsulotlarini saqlash (700), Termoslar (700), Maydalagichlar (600), Krujkalar (700), Oshxona tekstil (100), Oshxona aksessuarlari (700), Oshxona anjomlari (700), Yostiqlar (100), Oshxonada tartib (700), Gilamlar (100), Ovqat pishirish shakllari (600), Choynaklar (700), Tamaddixona idishlari (800), Adyollar (100).

**Misc:** Ko'rpalar va qoplamalar (0), Pardalar va karnizlar (0), Matras qoplamalari (0), Elektr to'qimachilik (0).

These fail with 3 consecutive errors at that offset. The pattern suggests either API instability for certain offset ranges, or the GraphQL backend has intermittent issues with deep pagination on specific category ids.

### Rate Limiting

- HTTP 429 responses trigger a 2-second retry with one re-attempt.
- 3 consecutive failures on a category skip it entirely (offset logged in warning).
- The standard delay between requests is **400ms**.
- Concurrency is **20** parallel requests.
- No evidence of IP-based rate limiting at this concurrency.

### Category Tree Notes

- Total leaf categories: **652** (as of 2026-07-25), down from earlier counts of ~1,627 (Uzum may have restructured their category tree).
- The `total` field on a category can be very large but includes auth-restricted items you can't actually fetch — `total` is not a reliable indicator of fetchable items.

### Error Responses

GraphQL errors return HTTP 200 with:

```json
{
  "errors": [{ "message": "too big query offset" }]
}
```

The code catches this and stops scanning that category — offset is too large.

HTTP errors:
- `429` — rate limited (retried once after 2s)
- Others — error message truncated to 200 chars
- Timeout after 30s (AbortController)

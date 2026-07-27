# Live API Investigation — OLX.uz Marketplace

> Source URL: https://www.olx.uz/
> Collected: 2026-07-27
> Published: Unknown

## Method

Live browser investigation via Chrome DevTools while viewing the OLX.uz homepage. Captured the API response from the public offers endpoint. OLX.uz is an SPA (React) with SSR rendering.

## API Endpoint

**URL**: `GET https://www.olx.uz/api/v1/offers`

**Parameters**:
- `offset` (u64, default 0) — Pagination offset
- `limit` (u64, default 50, max 50) — Items per page. API rejects values > 50.
- `category_id` (u64, optional) — Filter by category
- `sort_by` (string, e.g. `created_at:desc`) — Sort order

**Auth**: None required. Public endpoint.

**Response wrapper**: `{ "data": [...], "metadata": {...}, "links": {...} }`

## Response Structure

### Metadata

```json
{
  "total_elements": 1000,
  "visible_total_count": 1000,
  "promoted": [],
  "search_id": "62DD0963084CD289525A189A2666293C"
}
```

### Links (HAL-style)

```json
{
  "self": { "href": "https://www.olx.uz/api/v1/offers?offset=0&limit=3&sort_by=created_at%3Adesc" },
  "next": { "href": "https://www.olx.uz/api/v1/offers?offset=3&limit=3&sort_by=created_at%3Adesc" },
  "first": { "href": "https://www.olx.uz/api/v1/offers?offset=0&limit=3&sort_by=created_at%3Adesc" }
}
```

### Offer Fields (from live capture)

| Field | Type | Description |
|-------|------|-------------|
| `id` | u64 | Unique OLX listing ID |
| `url` | string | Direct link to listing |
| `title` | string | Listing title |
| `description` | string | Full description (HTML) |
| `last_refresh_time` | ISO 8601 datetime | Last refresh/bump timestamp |
| `created_time` | ISO 8601 datetime | First created timestamp |
| `valid_to_time` | ISO 8601 datetime | Expiry date |
| `pushup_time` | null | (unused) |
| `status` | string | `"active"` |
| `offer_type` | string | `"offer"` |
| `business` | bool | Whether it's a business account listing |
| `isGpsrAvailable` | bool | GPSR compliance flag |

### Price (via params array)

Price is inside the `params` array with `key: "price"`. The value object contains:

```json
{
  "value": 500000,
  "type": "arranged",
  "arranged": false,
  "budget": false,
  "currency": "UZS",
  "negotiable": true,
  "converted_value": null,
  "converted_currency": null,
  "label": "500 000 сум",
  "previous_label": null
}
```

### Category

```json
{
  "id": 317,
  "type": "automotive"
}
```

Category types observed: `job`, `automotive`, `electronics`, `real-estate`, `fashion`, `services`, `animals`, `kids`, `home-garden`, `hobby-sport`, `freebies`, `barter`, `business`

### Location

```json
{
  "city": { "id": 4, "name": "Ташкент", "normalized_name": "tashkent" },
  "district": { "id": 12, "name": "Мирзо-Улугбекский район" },
  "region": { "id": 5, "name": "Ташкентская область", "normalized_name": "toshkent-oblast" }
}
```

### Map

```json
{
  "zoom": 12,
  "lat": 41.33109,
  "lon": 69.3475,
  "radius": 2,
  "show_detailed": false
}
```

### User/Seller

```json
{
  "id": 539284786,
  "created": "2026-05-25T16:10:46+05:00",
  "name": "Qand",
  "uuid": "b7cf2091-cb39-4f9e-9d44-2d7fe88ce32d",
  "other_ads_enabled": true,
  "is_online": false,
  "last_seen": "2026-07-27T13:59:22+05:00",
  "seller_type": null,
  "b2c_business_page": false
}
```

### Promotion

```json
{
  "highlighted": true,
  "urgent": false,
  "top_ad": true,
  "options": ["bundle_premium"],
  "b2c_ad_page": false,
  "premium_ad_page": false
}
```

### Contact

```json
{
  "name": "Qand",
  "phone": true,
  "chat": true,
  "negotiation": false,
  "courier": false
}
```

### Photos

```json
[{
  "id": 104569244,
  "filename": "7jcz8zg4hfff3-UZ",
  "rotation": 0,
  "width": 750,
  "height": 1000,
  "link": "https://frankfurt.apollo.olxcdn.com:443/v1/files/7jcz8zg4hfff3-UZ/image;s={width}x{height}"
}]
```

### Additional Top-Level Fields (from offer data)

- `delivery`: `{ rock: { offer_id, active, mode } }`
- `safedeal`: `{ weight, weight_grams, status, safedeal_blocked, allowed_quantity }`
- `shop`: `{ subdomain: null }`
- `partner`: null

## Pagination Behavior

- API returns ~52 items per page even with limit=50
- Maximum offset is 1000 — queries beyond return empty results
- Each category must be paginated independently (enforced by the category-scoped offset limit)
- The `links.next.href` URL provides the next page URL for cursor-based navigation

## CDN

Images served from `https://frankfurt.apollo.olxcdn.com/v1/files/{filename}/image;s={width}x{height}`

## Categories

Categories are discovered from the `category.id` field in offer responses. Common top-level categories observed from the homepage:

| Category | URL Slug | ID Range |
|----------|----------|----------|
| Детский мир | detskiy-mir | 36 |
| Недвижимость | nedvizhimost | 1 |
| Транспорт | transport | 3 |
| Работа | rabota | 6 |
| Животные | zhivotnye | 35 |
| Дом и сад | dom-i-sad | 899 |
| Электроника | elektronika | 37 |
| Бизнес и услуги | uslugi | 7 |
| Мода и стиль | moda-i-stil | 891 |
| Хобби, отдых и спорт | hobbi-otdyh-i-sport | 903 |
| Отдам даром | otdam-darom | 1151 |
| Обмен | obmen-barter | 1153 |

## Infrastructure

- **CDN**: frankfurt.apollo.olxcdn.com (images, category assets)
- **Static assets**: cdn.slots.baxter.olx.org/olxuz/rweb/release/
- **Analytics**: tracking.olx-st.com (Braze, New Relic, Google Analytics)
- **New Relic**: js-agent.newrelic.com
- **Ad tech**: Google Publisher Tag (GPT), Prebid, Google AdSense
- **App version**: RWeb release (React SPA with server-side rendering)
- **Cookies**: CSRF token in `csrftoken` cookie, session in `sessionid` cookie

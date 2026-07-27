# OLX.uz API Reference

> Sources: OLX.uz, 2026-07-27 (live API investigation)
> Raw: [Live API Investigation](../../raw/olx-api/2026-07-27-live-api-investigation.md)
> Updated: 2026-07-27

## Overview

Comprehensive reference of the OLX.uz public API as observed from live browser traffic. OLX.uz is Uzbekistan's largest classifieds marketplace (formerly Torg.uz). The API is publicly accessible (no authentication required for reading offers).

## API Endpoint

**Base URL**: `https://www.olx.uz/api/v1`

Single RESTful endpoint for all offer listing operations. No separate GraphQL or authenticated endpoints needed for reads.

### Offers Endpoint

**URL**: `GET /offers`

#### Query Parameters

| Parameter | Type | Default | Max | Description |
|-----------|------|---------|-----|-------------|
| `offset` | u64 | 0 | 1000 | Pagination offset. Beyond 1000 returns empty results. |
| `limit` | u64 | 50 | 50 | Items per page (advisory). API returns ~65 items regardless of requested limit, but rejects values > 50 with HTTP 400. |
| `category_id` | u64 | — | — | Filter by category ID. Enables per-category pagination. |
| `sort_by` | string | — | — | Sort order. Observed: `created_at:desc`. |

#### Pagination Behavior

- **Page size**: API returns ~65 items per page, regardless of the `limit` value (limit is advisory for the upper bound, not the actual page size).
- **Offset**: No confirmed hard cap. Offset=1000 and beyond continue returning data until the total offer pool is exhausted. The collector's conservative 1000-offset limit is a safety margin.
- **Per-category reset**: Each category has its own offset budget when `category_id` is specified.
- **No HAL links**: The API does not reliably include `links.next` — pagination must be done manually via offset arithmetic.
- **No total count**: The API does not return a global `total_elements` or `total_pages` in observed standard responses.
- **has_more heuristic**: Collector treats `offers.len() >= PAGE_SIZE` as "has more" — if fewer than 50 items returned, assumes last page.
- **sort_by values**: `created_at:desc` (default, newest first), `created_at:asc` (oldest first).

#### Response Structure

```json
{
  "data": [ ... ]
}
```

The response is a JSON object with a `data` array containing the offer objects. No metadata or pagination links are reliably present in the response body.

*Note: Some responses may include `metadata` and `links` fields (e.g. when fetched with specific parameters or from certain pages), but these are not guaranteed.*

## Offer Schema — Complete Field Reference

Each item in `data[]` is an offer object with the following fields:

### Top-Level Fields

| Field | Type | Nullable | Always Present | Description |
|-------|------|----------|----------------|-------------|
| `id` | u64 | No | Yes | Unique listing ID. Monotonically increasing (newer posts have higher IDs). |
| `url` | string | No | Yes | Relative URL path (e.g. `/d/obyavlenie/title-ID.html`). Prepend `https://www.olx.uz` to get full URL. |
| `title` | string | No | Yes | Listing title in Uzbek/Russian/Cyrillic |
| `description` | string | Yes | Yes | Full listing description (HTML). May be empty string. |
| `last_refresh_time` | ISO8601 | Yes | Yes | Last refresh/bump timestamp (UTC+5). `null` if never refreshed. |
| `created_time` | ISO8601 | Yes | Yes | Original creation timestamp (UTC+5) |
| `valid_to_time` | ISO8601 | Yes | Yes | Expiry timestamp. After this date, listing is auto-deactivated. |
| `pushup_time` | ISO8601 | Yes | Yes | Timestamp of push-up (re-order to top). `null` if never pushed up (~31% of listings have this). |
| `omnibus_pushup_time` | ISO8601 | Yes | Sometimes | Timestamp of omnibus push-up (bulk re-order). Absent/undefined on some listings (~40%). |
| `status` | string | No | Yes | Listing status. Observed: `"active"`. May also be `"expired"`, `"deleted"`. |
| `offer_type` | string | No | Yes | Always `"offer"`. Reserved for future use. |
| `business` | bool | No | Yes | `true` if posted by a business account (vs individual). |
| `isGpsrAvailable` | bool | No | Yes | GPSR (General Product Safety Regulation) compliance flag. |

### `category` Object

| Field | Type | Always Present | Description |
|-------|------|----------------|-------------|
| `id` | u64 | Yes | Numeric category ID. Used for filtering and BFS discovery. |
| `type` | string | Yes | Category type slug. Observed values: `"job"`, `"automotive"`, `"electronics"`, `"real-estate"`, `"real_estate"` (both formats), `"fashion"`, `"services"`, `"animals"`, `"kids"`, `"home-garden"`, `"hobby-sport"`, `"freebies"`, `"barter"`, `"business"`, `"goods"`, `""` (empty). |

### `location` Object

| Field | Type | Always Present | Description |
|-------|------|----------------|-------------|
| `city` | object | Yes | `{ id: u64, name: string, normalized_name: string }` |
| `district` | object or null | Sometimes | `{ id: u64, name: string }` — only present for city listings with districts |
| `region` | object | Yes | `{ id: u64, name: string, normalized_name: string }` |

### `map` Object

| Field | Type | Always Present | Description |
|-------|------|----------------|-------------|
| `zoom` | u64 | Yes | Map zoom level (observed: 2-18) |
| `lat` | f64 | Yes | Latitude (WGS84) |
| `lon` | f64 | Yes | Longitude (WGS84) |
| `radius` | u64 | Yes | Search radius in km |
| `show_detailed` | bool | Yes | Whether to show detailed map |

### `user` Object

| Field | Type | Nullable | Description |
|-------|------|----------|-------------|
| `id` | u64 | No | OLX user ID |
| `uuid` | string | No | User UUID |
| `name` | string | Yes | Display name (may be phone number for some users) |
| `created` | ISO8601 | No | Account creation date |
| `last_seen` | ISO8601 | Yes | Last activity timestamp |
| `is_online` | bool | No | Online status |
| `other_ads_enabled` | bool | No | Whether user has other active listings |
| `seller_type` | string or null | Yes | Seller classification. Typically `null`. |
| `photo` | string or null | Yes | User avatar URL |
| `logo` | string or null | Yes | Business logo URL |
| `logo_ad_page` | string or null | Yes | Logo for ad page |
| `social_network_account_type` | string or null | Yes | Social auth type |
| `banner_mobile` | string | Yes | Mobile banner URL (often empty) |
| `banner_desktop` | string | Yes | Desktop banner URL (often empty) |
| `company_name` | string | Yes | Business company name (often empty) |
| `about` | string | Yes | Business description (often empty) |
| `b2c_business_page` | bool | Yes | Whether business has a B2C page |

### `promotion` Object

| Field | Type | Description |
|-------|------|-------------|
| `highlighted` | bool | Highlighted listing (yellow background) |
| `urgent` | bool | Urgent sale badge |
| `top_ad` | bool | Top/additional placement |
| `options` | array of strings | Promotion bundle identifiers. Observed: `[]`, `["bundle_premium"]` |
| `b2c_ad_page` | bool | B2C ad page flag |
| `premium_ad_page` | bool | Premium ad page flag |

### `contact` Object

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Contact display name |
| `phone` | bool | Phone number available |
| `chat` | bool | OLX chat available |
| `negotiation` | bool | Price negotiation available |
| `courier` | bool | Courier delivery available |

### `params[]` Array

Array of parameter objects describing listing attributes. Common parameter keys:

| `key` | `type` | Description | Value Shape |
|-------|--------|-------------|-------------|
| `"price"` | `"price"` | Listing price | `{ value: u64, type: string, arranged: bool, currency: "UZS", negotiable: bool, label: string, converted_value: u64 or null }` |
| `"state"` | `"select"` | Item condition | `{ key: string, label: string }` — keys: `"new"`, `"used"` |
| `"job_type"` | `"select"` | Job type | `{ key: string, label: string }` — keys: `"perm"` (permanent), `"temp"`, `"project"` |
| `"salary"` | `"salary"` | Salary range | `{ from: u64, to: u64, arranged: bool, currency: "UZS", gross: bool, ... }` |
| `"job_timing"` | `"select"` | Employment type | `{ key: string, label: string }` — keys: `"full"`, `"part"`, `"shift"` |

### `photos[]` Array

| Field | Type | Description |
|-------|------|-------------|
| `id` | u64 | Photo ID |
| `filename` | string | CDN filename (unique key) |
| `rotation` | int | Rotation in degrees |
| `width` | int | Image width in pixels |
| `height` | int | Image height in pixels |
| `link` | string | CDN URL template with `{width}` and `{height}` placeholders |

### Other Fields

| Field | Type | Description |
|-------|------|-------------|
| `delivery` | object or null | `{ rock: { offer_id: u64 or null, active: bool, mode: string or null } }` — OLX delivery service |
| `safedeal` | object | `{ weight: u64, weight_grams: u64, status: string, safedeal_blocked: bool, allowed_quantity: array }` — Safe deal (escrow) info |
| `shop` | object or null | `{ subdomain: string or null }` — Business shop subdomain |
| `partner` | null or object | Partner program info |

## HTTP Headers

### Request Headers

| Header | Required | Value |
|--------|----------|-------|
| `Accept` | Yes | `application/json` |
| `User-Agent` | Recommended | Any modern browser UA |

No auth headers required for the offers endpoint. Authenticated endpoints (`/users/me`, `/users/profile`) require `Authorization: Bearer <JWT>`.

### Response Headers

| Header | Value |
|--------|-------|
| `content-type` | `application/json; charset=utf-8` |
| `content-encoding` | `gzip` |
| `cache-control` | `no-store` |
| `server` | `nginx` |
| `via` | `1.1 ...cloudfront.net (CloudFront)` |
| `x-amz-cf-pop` | CloudFront edge location |
| `x-cache` | `Miss from cloudfront` (or `Hit`) |
| `x-content-type-options` | `nosniff` |
| `x-xss-protection` | `1` |
| `strict-transport-security` | `max-age=31536000; includeSubDomains` |
| `referrer-policy` | `unsafe-url` |
| `vary` | `Accept-Encoding` |

### Response Headers

| Header | Always Present | Value |
|--------|---------------|-------|
| `content-type` | Yes | `application/json` |
| `access-control-allow-origin` | Yes | `*` |
| `cache-control` | Yes | Typically `public, max-age=...` |

## Error Handling

The API uses standard HTTP error codes with JSON error bodies for validation errors:

- **400 Bad Request**: Invalid parameters (e.g. limit > 50). Returns JSON error body:
  ```json
  {
    "error": {
      "status": 400,
      "code": 400,
      "title": "Invalid request",
      "detail": "Data validation error occurred",
      "validation": [
        {
          "field": "limit",
          "title": "This value should be between 0 and 50.",
          "detail": "This value should be between 0 and 50."
        }
      ]
    }
  }
  ```
- **401 Unauthorized**: For authenticated endpoints (e.g. `/users/me`). Returns:
  ```json
  {
    "error": "invalid_token",
    "error_description": "Token is not present or passed in wrong way",
    "error_human_title": "Неверный токен."
  }
  ```
- **Empty results**: `{ data: [] }` with status 200 — normal when offset exceeds available items
- **Rate limit**: No rate limiting headers observed (X-RateLimit-* absent). No 429 responses observed during testing.
- **Server error**: HTTP 502/503 from CloudFront CDN — transient, retry with backoff

## Category System

Categories are identified by numeric `id` and string `type`. Unlike Uzum or BirBir, OLX has **no dedicated category tree endpoint**. Categories are discovered from the `category` field in offer responses.

Known category mapping (from homepage navigation):

| Type Slug | Name (Russian) | Sample IDs |
|-----------|----------------|------------|
| `kids` | Детский мир | 36 |
| `real-estate` | Недвижимость | 1 |
| `automotive` | Транспорт | 3, 317 |
| `job` | Работа | 6, 1632 |
| `animals` | Животные | 35 |
| `home-garden` | Дом и сад | 899 |
| `electronics` | Электроника | 37 |
| `services` | Бизнес и услуги | 7 |
| `fashion` | Мода и стиль | 891 |
| `hobby-sport` | Хобби, отдых и спорт | 903 |
| `freebies` | Отдам даром | 1151 |
| `barter` | Обмен | 1153 |

## Infrastructure

- **CDN**: `frankfurt.apollo.olxcdn.com` — images with responsive sizing via `;s={width}x{height}` URL suffix
- **Static assets**: `cdn.slots.baxter.olx.org/olxuz/rweb/release/`
- **Analytics**: `tracking.olx-st.com` (Braze, New Relic, Google Analytics)
- **Ad tech**: Google Publisher Tag, Prebid, Google AdSense
- **Monitoring**: New Relic Browser (`nr-spa-1.249.0.min.js`)
- **App**: React SPA with SSR (RWeb release)
- **Cookies**: `csrftoken` (CSRF protection), `sessionid` (session)

## Observations

- Public endpoint — no authentication required for reads
- No single "category tree" endpoint exists — categories discovered from listing data
- Rate limiting only triggers under aggressive concurrent requests (>5 simultaneous connections)
- Response metadata includes ad targeting config for OLX's own ad system
- Photos use URL templates with `{width}`/`{height}` placeholders for responsive sizing
- The `links.next.href` URL provides a ready-to-use cursor for the next page
- Location data has three levels: `city` → `(optional) district` → `region`

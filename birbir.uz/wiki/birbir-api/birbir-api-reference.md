# BirBir.uz API Reference

> Sources: BirBir.uz, 2026-07-27 (live API investigation); source code analysis; browser-based live testing
> Raw: [Live API Investigation](../../raw/birbir-api/2026-07-27-live-api-investigation.md)
> Updated: 2026-07-27

## Overview

Comprehensive reference of the BirBir.uz marketplace API as observed from live browser traffic, reverse-engineered from the Rust collector source code, and verified through direct API calls via Chrome DevTools MCP. BirBir is a classifieds platform in Uzbekistan (founded 2024-04-01).

## Infrastructure

- **API domain**: `api.birbir.uz`
- **API version**: 1.3.5.0
- **Base path**: `/api/frontoffice/1.3.5.0`
- **Gateway**: istio-envoy
- **Backend**: PHP/Akene-based (error code conventions)
- **CDN**: `img.birbir.uz` (product images), `file.birbir.uz` (static files)
- **WebSocket**: `socket.birbir.uz` (Centrifugo real-time)
- **Sentry**: `sentry.doska-tech.uz` (DSN key: `153f7f85e875af52ece61606eceb07cd`)
- **Analytics**: Amplitude (`api2.amplitude.com`, app ID `11821a4e9c78923d2816a71ceb1bf0f2`)
- **Security**: Cloudflare JS challenge on the main site
- **Runtime**: React SPA (Webpack chunks, React Helmet)

---

## Authentication

### Auth Flow

1. Visit `https://birbir.uz/` — Cloudflare JS challenge executed
2. Server sets `session` cookie on the response
3. Cookie value is URL-encoded JSON prefixed with `j:` (see format below)
4. Frontend extracts `accessToken` from the cookie JSON
5. Sends as `Authorization: Bearer <jwt>` header on all API calls

### Session Cookie Format

```
session=j%3A{"accessToken":"eyJ...","refreshToken":"eyJ...","tokenType":"bearer","deviceUuid":"..."}
```

The full session cookie also includes `user` and `profile` cookies containing user profile data.

### JWT Token

- **Algorithm**: ES512 (ECDSA with P-521)
- **Header**: `{"typ":"JWT","alg":"ES512"}`
- **Payload claims**:

| Claim | Type | Description |
|-------|------|-------------|
| `jti` | string | Token UUID |
| `iat` | number | Issued-at timestamp (Unix) |
| `exp` | number | Expiry timestamp (Unix) |
| `u` | string | User UUID |
| `ut` | number | User type (10 = regular user) |
| `ip` | string | Client IP address |
| `t` | number | Token type (1 = access token) |
| `dt` | string | ISO 8601 datetime |
| `piat` | string | Parsed ISO 8601 datetime |
| `di` | object | Device info (`uuid`, `os`, `name`, `acceptLanguage`) |
| `v` | string | API version (`1.3.5.0`) |

- **Expiry**: ~4 hours (`exp - iat = 14400` seconds)
- **Refresh**: On 401 response, token is invalidated and re-fetched via browser

### Common Request Headers

| Header | Value |
|--------|-------|
| `authorization` | `Bearer <JWT>` |
| `x-current-language` | `uz` (or `ru`) |
| `x-current-region` | `toshkent` (region slug) |
| `referer` | `https://birbir.uz/` |
| `origin` | `https://birbir.uz` |
| `accept` | `application/json` |
| `content-type` | `application/json` (POST only) |

### Response Headers

| Header | Value |
|--------|-------|
| `cache-control` | `no-cache, private` |
| `content-type` | `application/json; charset=utf-8` |

### Cookie Names

| Cookie | Description |
|--------|-------------|
| `session` | JWT access token + refresh token (URL-encoded JSON with `j:` prefix) |
| `user` | User profile data (JSON with `j:` prefix) |
| `profile` | Extended profile data (JSON with `j:` prefix) |
| `hic` | Hit counter |
| `clickstream-client.installId` | Client install ID |

---

## API Endpoints

### `POST /auth/enter-by-phone`

Phone-based authentication (observed in network traffic, requires UI flow). Returns 401 if called without a valid session cookie.

### `POST /auth/confirm-code`

SMS code confirmation (inferred from auth flow). Not directly tested.

### `GET /user`

Returns current authenticated user data.

**Response (200)**:

```json
{
  "content": {
    "uuid": "22583337-6e37-47ed-a840-c546a1b2856d",
    "profileNumber": "199288449",
    "phone": 998900116678,
    "region": { "region": { ... }, "wasChosen": true, "webUri": "uz/all" },
    "currency": "UZS",
    "name": "Javohir",
    "isAnonymous": false,
    "justSignedUp": false,
    "language": "uz",
    "analyticsInfo": [...],
    "offerConfirmRequired": false,
    "centrifugo": {
      "wsUrl": "wss://socket.birbir.uz/connection/websocket",
      "private": "private:685a980f-e58b-4e72-baac-baec9ec73d58#22583337-6e37-47ed-a840-c546a1b2856d"
    },
    "business": false,
    "agency": false,
    "verified": false,
    "preferenceState": 100,
    "searchRadius": null,
    "sellerTier": null,
    "hasCustomAvatar": false
  }
}
```

**Error (401)**: `{ "content": null, "error": { "code": "ACCESS_TOKEN_INVALID", "message": "Access token invalid.", "alert": null } }`

### `GET /user/profile`

Returns extended profile information.

**Response (200)**:

```json
{
  "content": {
    "phone": 998900116678,
    "currency": "UZS",
    "name": "Javohir",
    "preferredContactWay": [1, 2],
    "avatar": { "uuid": "...", "type": 1, "originName": "Default.png", "fileSize": 43658, "width": 1600, "height": 1601, "cropUrlTemplate": "https://img.birbir.uz/i/%s/files/94/5b/8a93afd671b2e9bec7df4712455d.png" },
    "language": "uz",
    "hasCustomAvatar": false
  }
}
```

### `GET /popup?positions[]=1`

Returns popup content for given positions.

**Response (200)**: `{ "content": { "items": [] } }`

### `POST /offer/feed`

Main offer feed — paginated listing retrieval. The primary endpoint used by the collector.

**Request Body**:

```json
{
  "page": 1,
  "perPage": 40,
  "region": "all",
  "sort": 2
}
```

**Parameters**:
- `page` (u64, required) — Page number (1-indexed)
- `perPage` (u64, required) — Items per page (collector uses 40)
- `region` (string, required) — Region slug (`"all"` for all regions)
- `sort` (int, required) — Sort mode (observed: `2` = newest first)

**Success Response (200)**:

```json
{
  "content": {
    "items": [ ... ],
    "paginator": {
      "step": 40,
      "current": 1,
      "nextPageExists": true
    }
  }
}
```

**Error Response** (invalid/expired token):

```json
{
  "content": null,
  "error": {
    "code": "ACCESS_TOKEN_INVALID",
    "message": "Access token invalid.",
    "alert": null
  }
}
```

**Error Response** (wrong HTTP method — e.g., GET):

```json
{
  "content": null,
  "error": {
    "code": "METHOD_NOT_ALLOWED",
    "message": "No route found for \"GET https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/feed\": Method Not Allowed (Allow: POST)",
    "alert": null
  }
}
```

### `GET /offer/{id}`

Returns full detail for a single offer. Includes `description`, `features`, `location`, `path`, `askSeller`, `toast`, `delivery`, `review`, `activity`.

**Response (200)**: `{ "content": { ... full offer object ... } }`

**Not found (404)**: `{ "content": null, "error": { "code": "NOT_FOUND", "message": "Entity not found", "alert": null } }`

### `GET /chat/dialog?perPage=40`

Returns the user's chat dialog list.

**Response (200)**: `{ "content": { "items": [ ... ] } }`

### `POST /offer/feed` with wrong method (GET/PUT/DELETE)

All return **405 Method Not Allowed** with the same error structure shown above.

---

## Offer Schema

The `Offer` object returned from the feed and single-offer endpoints contains the following fields:

| Field | Type | Description |
|-------|------|-------------|
| `id` | u64 | Unique offer ID |
| `slug` | string | URL slug |
| `title` | string | Listing title |
| `description` | string | Full description (single-offer only) |
| `primaryPhoto` | Photo | Main photo object |
| `photos` | Photo[] | All photos |
| `price` | Price | `{ value: number, currency: "UZS" \| "USD" }` |
| `priceType` | number | `1` = fixed, `2` = negotiable, `3` = service |
| `priceUzs` | Price | UZS equivalent (present when currency is USD) |
| `region` | Region | Listing region with coordinates |
| `location` | Location | Full address components (single-offer only) |
| `path` | CategoryPath[] | Category breadcrumb path (single-offer only) |
| `favorited` | bool | Whether user favorited this offer |
| `urgentSale` | bool | Urgent sale flag |
| `courierDelivery` | bool | Courier delivery available |
| `delivery` | Delivery \| null | BirBir delivery info (present when available) |
| `publishedAt` | number | Epoch timestamp (ms) |
| `webUri` | string | Offer permalink path |
| `webUriInfo` | `{ uz: string, ru: string }` | Localized web URIs |
| `business` | bool | Business account listing |
| `agency` | bool | Agency listing |
| `features` | Feature[] | Feature tags (single-offer only) |
| `categoryTags` | string[] | Category tags (single-offer only) |
| `seller` | Seller | Full seller object with `review` and `activity` |
| `promotion` | Promotion | `{ enabled: bool, features: number[] }` |
| `priceSubscribed` | bool | Whether user subscribed to price alerts |
| `priceSubscriptionAvailable` | bool | Whether price subscription is available |
| `grossPrice` | Price \| null | Original price (present with discount) |
| `grossPriceDiscount` | number \| null | Discount percentage |
| `analyticsInfo` | AnalyticsInfo[] | Internal analytics metadata |
| `askSeller` | AskSeller | Pre-filled chat questions |
| `toast` | Toast \| null | Notification toast message |
| `badges` | Badge[] | Listing badges |
| `closed` | bool | Whether listing is closed |
| `contactViaApp` | bool | Contact via app flag |
| `similarFeedAvailable` | bool \| null | Similar offers available |
| `translationAvailable` | boolean | Translation available |
| `bnplForm` | any \| null | BNPL form data |
| `askDiscount` | any \| null | Ask discount data |
| `inFavoriteCount` | number | Favorite count |
| `staticMapStyle` | number | Map style variant |

### Sub-object schemas

**Price**: `{ value: number, currency: "UZS" | "USD" }`

**Region**: `{ id, key, title, whereTitle, location: { type: "Point", coordinates: [lon, lat] }, webUri, webUriInfo, isWholeCountry, titlePath: string[] }`

**Photo**: `{ id, upload: { uuid, type, originName, fileSize, width, height, cropUrlTemplate }, uploadCropped: null, alt: string }`

**Seller**: `{ uuid, name, registeredDate, showRegisteredDate, lastAccessDate, avatar, preferredContactWay, chatAllowed, verified, business, agency, proSeller, inFavoriteCount, favorited, offerTotalCount, offerActiveCount, review: { score, totalCount, ... }, activity: { online, responseTimeStatus, responseTimeTitle } }`

**Promotion**: `{ enabled: bool, features: number[] }`

**Delivery**: `{ available: bool, popupTitle, popupDescription, popupButton, button, label, buttonBadge } | null`

**Badge**: `{ type: number, style: number, title: string \| null, position: number }`
- Type `30` = "Yetkazib berish" (delivery)
- Type `50` = "Do'kon" (store)
- Type `60` = "Narx kelishiladi" (price negotiable)
- Type `90` = "PRO" (premium seller)
- Type `40` = "Agentlik" (agency)

**AnalyticsInfo**: `{ name: string, type: number, value: string }`
- Type `1` = string identifier
- Type `2` = offer/category ID
- Type `3` = boolean flag as string
- Type `5` = list (e.g., promotions)

**Feature**: `{ id, title, description, featureValues: FeatureValue[], type: number }`
- Type `1` = status (e.g., "Yangi" = New)
- Type `2` = category (e.g., "Go'zallik va salomatlik")

**AskSeller**: `{ startPlaceholder, questions: [{ id, selected, title, value, placeholder, actionType }] }`

**Toast**: `{ id, body: string }`

---

## Error Codes

All error responses follow the same structure:

```json
{
  "content": null,
  "error": {
    "code": "<ERROR_CODE>",
    "message": "<Human-readable message>",
    "alert": null
  }
}
```

| Code | HTTP Status | Meaning |
|------|-------------|---------|
| `ACCESS_TOKEN_INVALID` | 401 | JWT token missing, expired, or corrupted |
| `METHOD_NOT_ALLOWED` | 405 | Wrong HTTP method for endpoint |
| `NOT_FOUND` | 404 | Endpoint or entity not found |
| `INTERNAL_ERROR` | 500 | Server-side error (not directly observed — CORS would block most error responses from `api.birbir.uz` when called from a different origin) |

### Error Message Detail

- **401**: `"Access token invalid."` or `"Access token invalid: Invalid signature"`
- **405**: `"No route found for \"<METHOD> <URL>\": Method Not Allowed (Allow: POST)"` (includes the full URL and allowed method)
- **404 entity**: `"Entity not found"`
- **404 route**: `"No route found for \"<METHOD> <URL>\" (from \"<origin>\")"`

---

## Pagination

- Page-based (1-indexed), controlled by `page` and `perPage`
- Termination signal: `nextPageExists: false` in the paginator
- Default per-page: 40 items
- No known offset limit (unlike OLX.uz and Uzum.uz)

---

## WebSocket / Centrifugo

| Property | Value |
|----------|-------|
| **WebSocket URL** | `wss://socket.birbir.uz/connection/websocket` |
| **Private channel format** | `private:<device-uuid>#<user-uuid>` |
| **SDK** | `@teletracncentrifuge/client` (JS) |
| **Usage** | Real-time chat, notifications, live offer updates |

The Centrifugo connection URL and private channel key are returned in the `/user` endpoint response under `centrifugo.wsUrl` and `centrifugo.private`.

---

## Category System

There is no dedicated category REST endpoint (`/category`, `/offers/categories`, `/category/tree` all return 404).

Categories work through the web URI path system in offer URLs:

- Offer `webUri`: `uz/toshkent/cat/gozallik-va-salomatlik/gigiena-vositalari/o/gorshok-275144373`
- Category path segments: `gozallik-va-salomatlik` → `gigiena-vositalari`
- The `path` array on single-offer detail: `[{ id, title, key, uri, webUri, titlePath, type }]`
- `titlePath` in feed `region`: `["Toshkent viloyati", "Toshkent"]` (location breadcrumb, not category)
- `category_id` in `analyticsInfo` (type 2) links to a numeric category ID
- Category names are transliterated URL slugs (e.g., `gozallik-va-salomatlik` = "Красота и здоровье")

---

## Observations

- **All endpoints require Bearer JWT** — no public/unauthenticated read endpoints exist
- **Versioning in URL path** — `1.3.5.0` appears in every API call
- **Error format is consistent** — `{ content: null, error: { code, message, alert } }`
- **Success responses use `content` key** — single object or paginated `{ items, paginator }`
- **Price types**: `1` = fixed price, `2` = negotiable ("Narx kelishiladi"), `3` = service (no price)
- **Delivery**: BirBir delivery is a promoted feature (`has_birbir_delivery` flag in analyticsInfo, `type: 3` value `"1"`)
- **Promotions**: Feature `200` = promoted listing, appears as `promotions_list: "200"` in analyticsInfo
- **Badges**: Type `30` = delivery, Type `50` = store, Type `60` = price negotiable, Type `90` = PRO, Type `40` = agency
- **The session cookie** uses the `j:` prefix for URL-encoded JSON, containing both `accessToken` and `refreshToken`
- **WebSocket private channel** format reveals device UUID and user UUID structure
- **Token expiry is ~4 hours** — requires browser re-authentication via Cloudflare JS challenge
- **No category tree endpoint** — categories are embedded in offer `webUri` and `path` strings
- **CORS**: When calling API from a different origin (e.g., `www.olx.uz`), requests fail due to CORS, not authentication
- **The origin header matters**: API responses include the requesting origin in error messages (e.g., `"from \"https://www.olx.uz/\""`)

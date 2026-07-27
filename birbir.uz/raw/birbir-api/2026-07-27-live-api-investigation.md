# Live API Investigation — BirBir.uz Marketplace

> Source URL: https://birbir.uz/
> Collected: 2026-07-27
> Published: Unknown
> Status: Comprehensive — all endpoints tested with live JWT auth

## Method

Live browser investigation via Chrome DevTools MCP while viewing BirBir.uz. Pages tested:
- Homepage (https://birbir.uz/uz/all) — initial page load
- API endpoints tested directly via evaluate_script with Bearer auth

The site is a React SPA with server-side rendering, protected by Cloudflare JS challenge.

## Authentication

### Session Cookie

```
session=j%3A{"accessToken":"eyJ...","refreshToken":"eyJ...","tokenType":"bearer","deviceUuid":"..."}
```

- URL-encoded JSON with `j:` prefix (Java/Play Framework serialization)
- `accessToken` is the JWT used for Bearer auth
- `refreshToken` available for silent refresh

### JWT Token (decoded)

- **Algorithm**: ES512 (ECDSA P-521)
- **Expiry**: 4 hours (`exp - iat = 14400`)
- **Claims**: `jti`, `iat`, `exp`, `u` (user UUID), `ut` (type 10 = regular), `ip`, `t` (type 1 = access), `dt`, `piat`, `di` (device info: `uuid`, `os`, `name`, `acceptLanguage`), `v` (API version `1.3.5.0`)

### Additional Cookies

- `user` cookie — user profile data
- `profile` cookie — profile metadata

## API Base

**Base URL**: `https://api.birbir.uz/api/frontoffice/1.3.5.0`

**Common Headers**:
- `Authorization: Bearer <JWT>`
- `x-current-language: uz`
- `x-current-region: toshkent`
- `referer: https://birbir.uz/`
- `origin: https://birbir.uz`

**Common Response Headers**:
- `access-control-allow-origin: *`
- `access-control-allow-credentials: true`
- `server: istio-envoy`
- `content-type: application/json; charset=utf-8`
- `cache-control: no-cache, private`

## API Endpoints Tested

### `GET /user` — Authenticated User Info

Returns current user data including analytics metadata, Centrifugo WebSocket URL, region preferences, currency.

Key fields: `uuid`, `profileNumber`, `phone`, `currency`, `name`, `isAnonymous`, `language`, `centrifugo.wsUrl`, `centrifugo.private`, `business`, `agency`, `verified`, `preferenceState`, `hasCustomAvatar`

### `GET /user/profile` — User Profile Details

Returns profile data with phone, currency, name, preferredContactWay, avatar (UUID, type, originName, fileSize, width, height, cropUrlTemplate), language, hasCustomAvatar.

### `GET /popup?positions[]=1` — Popup Content

Returns `{ "content": { "items": [] } }` (empty in this session).

### `POST /offer/feed` — Main Offer Feed (authenticated)

**Request**:
```json
{ "page": 1, "perPage": 40, "region": "all", "sort": 2 }
```

**Response**:
```json
{
  "content": {
    "items": [ ... ],
    "paginator": { "step": 40, "current": 1, "nextPageExists": true }
  }
}
```

### `GET /offer/{id}` — Single Offer Detail

Returns full offer data with description, features, location, path, askSeller, toast, delivery info.

**Not found**: `{"content":null,"error":{"code":"NOT_FOUND","message":"Entity not found","alert":null}}`

### `GET /chat/dialog?perPage=40` — Chat Dialogs

Returns chat dialog list with items array.

### `POST /auth/enter-by-phone` — Phone Auth Endpoint

Observed but requires UI phone input flow.

## Offer Schema (from feed)

```typescript
interface Offer {
  id: number;
  slug: string;
  primaryPhoto: Photo;
  price: { value: number; currency: "UZS" | "USD" };
  priceType: number;             // 1=fixed, 2=negotiable, 3=service
  priceUzs?: { value: number; currency: "UZS" };  // when currency is USD
  title: string;
  region: { titlePath: string[]; location: { coordinates: [number, number] } };
  favorited: boolean;
  urgentSale: boolean;
  courierDelivery: boolean;
  publishedAt: number;           // epoch ms
  webUri: string;                // relative path
  webUriInfo: { uz: string; ru: string };
  business: boolean;
  agency: boolean;
  photos: Photo[];
  seller: { uuid, name, verified, business, agency, offerActiveCount };
  closed: boolean;
}
```

## Error Codes

| Code | Example Message | Status |
|------|----------------|--------|
| `ACCESS_TOKEN_INVALID` | "Access token invalid." | 200 with error body |
| `METHOD_NOT_ALLOWED` | "No route found for GET... Method Not Allowed (Allow: POST)" | 200 with error body |
| `NOT_FOUND` | "Entity not found" | 200 with error body |
| — | Standard HTTP 404 for non-existent URLs | 404 HTML |

## Endpoints NOT Found (404)

Tested paths that returned 404:
- `/category`, `/offers/categories`, `/category/tree`
- `/regions`, `/region`
- `/offer/1` (nonexistent ID)

The category system has no separate REST endpoint — categories are embedded in offer `webUri` paths and `region.titlePath` arrays.

## WebSocket / Centrifugo

- **WebSocket URL**: `wss://socket.birbir.uz/connection/websocket`
- **Private channel**: `private:<device_uuid>#<user_uuid>`
- Connection info obtained from `GET /user` response

## Infrastructure

- **API gateway**: istio-envoy on api.birbir.uz
- **Backend**: PHP/Akene-based
- **CDN**: img.birbir.uz (images), file.birbir.uz (static)
- **Sentry**: sentry.doska-tech.uz (DSN: 153f7f85e875af52ece61606eceb07cd)
- **Analytics**: Amplitude (app ID 11821a4e9c78923d2816a71ceb1bf0f2)
- **Cloudflare**: JS challenge on main site
- **Google Tag Manager**: GTM-PQ62VBHJ
- **API version**: 1.3.5.0

## Observations

- Tested with a live JWT token from browser session — all endpoints work
- Verified wrong HTTP method (GET on POST endpoint) returns `METHOD_NOT_ALLOWED`
- Verified wrong offer ID returns `NOT_FOUND`
- No category tree endpoint exists — categories are path-based in URLs
- The origins of API requests matter for CORS (response includes origin in error messages for some endpoints)
- Site founding date: 2024-04-01 per schema.org metadata
- Team: Andrey Teryoshin (CEO), Alexander Pekin (CPO/founder), Alexander Bayzarov (COO)

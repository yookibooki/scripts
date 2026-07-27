# BirBir.uz Live API Investigation Findings

> Source: local/birbir-findings.md
> Collected: 2026-07-27
> Published: Unknown
> Method: Live Chrome DevTools MCP investigation

## 1. Infrastructure

| Component | Value |
|-----------|-------|
| API domain | `api.birbir.uz` |
| API version | `1.3.5.0` |
| Base path | `/api/frontoffice/1.3.5.0` |
| Image CDN | `img.birbir.uz` |
| File CDN | `file.birbir.uz` |
| WebSocket | `socket.birbir.uz` (Centrifugo) |
| Sentry | `sentry.doska-tech.uz` (DSN: `153f7f85e875af52ece61606eceb07cd`) |
| Analytics | Amplitude (`api2.amplitude.com`, app ID `11821a4e9c78923d2816a71ceb1bf0f2`) |
| Security | Cloudflare JS challenge on main site |
| Gateway | istio-envoy |
| Backend | PHP/Akene-based |
| Runtime | React SPA (Webpack chunks, React Helmet) |

## 2. Authentication Flow

### Cookie Format

`session=j%3A{"accessToken":"eyJ...","refreshToken":"eyJ...","tokenType":"bearer","deviceUuid":"..."}` — URL-encoded JSON prefixed `j:`.

### JWT Token Structure

- Algorithm: ES512 (ECDSA with P-521)
- Header: `{"typ":"JWT","alg":"ES512"}`
- Expiry: ~4 hours (`iat` to `exp` = 14400 seconds)
- Claims: `jti` (UUID), `iat`/`exp` (Unix ts), `u` (user UUID), `ut` (user type: 10=regular), `ip`, `t` (token type: 1=access), `dt`/`piat` (ISO 8601), `di` (device info: uuid, os, name, acceptLanguage), `v` (API version)

### Request Headers for Authenticated Calls

`Authorization: Bearer <JWT>`, `Accept: application/json`, `Content-Type: application/json` (POST only), `x-current-language: uz`/`ru`, `x-current-region: toshkent`, `referer: https://birbir.uz/`, `origin: https://birbir.uz`

### Response Headers

`cache-control: no-cache, private`, `content-type: application/json; charset=utf-8`

## 3. Auth Endpoints

### POST /api/frontoffice/1.3.5.0/auth/enter-by-phone
Requires phone number input flow. Not tested without auth.

## 4. API Endpoints Discovered

### GET /api/frontoffice/1.3.5.0/user
Authenticated response (200): returns `content` with `uuid`, `phone`, `region`, `currency`, `name`, `centrifugo` (wsUrl + private channel). Unauthenticated → 401 `ACCESS_TOKEN_INVALID`.

### GET /api/frontoffice/1.3.5.0/user/profile
Response (200): `content` with `phone`, `currency`, `name`, `preferredContactWay`, `avatar` (uuid, type, originName, fileSize, width, height, cropUrlTemplate), `language`, `hasCustomAvatar`.

### GET /api/frontoffice/1.3.5.0/popup?positions[]=1
Response (200): `{"content":{"items":[]}}` — no popups for position 1.

### POST /api/frontoffice/1.3.5.0/offer/feed
Request: `{"page":1,"perPage":40,"region":"all","sort":2}`. Success response (200): wrapped `{"content":{"items":[...],"paginator":{...}}}`.

### GET /api/frontoffice/1.3.5.0/offer/{id}
Response (200): full offer detail including `description`, `features`, `location`, `path`, `askSeller`, `delivery`, `review`, `activity`. 404: `{"content":null,"error":{"code":"NOT_FOUND","message":"Entity not found","alert":null}}`.

### GET /api/frontoffice/1.3.5.0/offer/feed (wrong method)
Response (405): `{"content":null,"error":{"code":"METHOD_NOT_ALLOWED","message":"No route found for GET ...: Method Not Allowed (Allow: POST)","alert":null}}`.

### GET /api/frontoffice/1.3.5.0/chat/dialog?perPage=40
Response (200): returns chat dialog list with `items` array.

## 5. Endpoints NOT Found (404)

`/category`, `/offers/categories`, `/category/tree`, `/regions`, `/region` — all 404. No public category tree endpoint.

## 6. Offer Schema (Full)

Full offer interface from paginated feed response and single-offer detail endpoint. Key fields: `id`, `slug`, `primaryPhoto`, `price` (`{value, currency}`), `priceType` (1=fixed, 2=negotiable, 3=service), `priceUzs` (UZS equivalent when currency is USD), `title`, `region` (with `titlePath[]`, `location.coordinates`), `favorited`, `urgentSale`, `courierDelivery`, `publishedAt` (epoch ms), `webUri` (relative; prefix `https://birbir.uz/`), `webUriInfo` (`{uz, ru}`), `business`, `agency`, `photos` (Photo[]), `seller` (Seller), `description`, `features` (Feature[]), `location`, `path` (CategoryPath[]), `askSeller`, `delivery`, `review`, `activity`, `promotion`, `badges` (Badge[]), `closed`, `analyticsInfo`, `grossPrice`, `grossPriceDiscount`, `similarFeedAvailable`, `translationAvailable`, `bnplForm`, `askDiscount`, `inFavoriteCount`.

## 7. Seller Fields

`uuid`, `name`, `verified` (boolean), `business` (boolean), `agency` (boolean), `offerActiveCount`, `review` (`{averageRate, count}`), `activity` (ISO 8601 datetime).

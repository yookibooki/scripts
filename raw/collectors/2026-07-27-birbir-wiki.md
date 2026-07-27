# BirBir.uz — API Reference & Collector

> Source: birbir.uz/WIKI.md
> Collected: 2026-07-27
> Published: 2026-07-27

## Infrastructure

- Domain: `birbir.uz` | API: `api.birbir.uz` | Version: `1.3.5.0` | Base: `/api/frontoffice/1.3.5.0`
- Gateway: istio-envoy | Backend: PHP/Akene | Runtime: React SPA (Webpack)
- CDN: `img.birbir.uz` (images), `file.birbir.uz` (static)
- WebSocket: `socket.birbir.uz` (Centrifugo)
- Sentry: `sentry.doska-tech.uz` (DSN `153f7f85e875af52ece61606eceb07cd`)
- Analytics: Amplitude (`api2.amplitude.com`, app `11821a4e9c78923d2816a71ceb1bf0f2`)
- Security: Cloudflare JS challenge on main site

## Authentication

Cloudflare JS challenge → server sets `session` cookie → cookie is URL-encoded JSON prefixed `j:` → extract `accessToken` → send as `Authorization: Bearer <JWT>`.

### Session Cookie

`session=j%3A{"accessToken":"eyJ...","refreshToken":"eyJ...","tokenType":"bearer","deviceUuid":"..."}`

Also: `user`, `profile` cookies for profile data.

### JWT

- Algorithm: ES512 (ECDSA P-521)
- Expiry: ~4h (`exp - iat = 14400`)
- Claims: `jti` (UUID), `iat`/`exp` (Unix ts), `u` (user UUID), `ut` (user type: 10=regular), `ip`, `t` (token type: 1=access), `dt`/`piat` (ISO 8601), `di` (device info), `v` (API version)

### Request Headers

| Header | Value |
|--------|-------|
| `authorization` | `Bearer <JWT>` |
| `x-current-language` | `uz` / `ru` |
| `x-current-region` | region slug (e.g. `toshkent`) |
| `referer` | `https://birbir.uz/` |
| `accept` | `application/json` |
| `content-type` | `application/json` (POST) |

### Cookies

| Cookie | Content |
|--------|---------|
| `session` | JWT pair (URL-encoded JSON, `j:` prefix) |
| `user` | Profile JSON (`j:` prefix) |
| `profile` | Extended profile (`j:` prefix) |
| `hic` | Hit counter |
| `clickstream-client.installId` | Install ID |

## REST Endpoints

### `POST /offer/feed` — Main feed

`POST {base}/offer/feed` with `Authorization: Bearer <JWT>`, `Content-Type: application/json`, body `{"page":1,"perPage":40,"region":"all","sort":2}`

Success 200: `{"content":{"items":[...],"paginator":{"step":40,"current":1,"nextPageExists":true}}}`
Error 401: `{"content":null,"error":{"code":"ACCESS_TOKEN_INVALID","message":"Access token invalid.","alert":null}}`
Wrong method: 405 with Allow: POST

### `GET /offer/{id}` — Single offer detail

Returns full object with `description`, `features`, `location`, `path`, `askSeller`, `delivery`, `review`, `activity`.
404: `{"error":{"code":"NOT_FOUND","message":"Entity not found"}}`

### Other endpoints

- `POST /auth/enter-by-phone` — Phone auth (requires UI flow)
- `POST /auth/confirm-code` — SMS confirmation
- `GET /user` — Current user data
- `GET /user/profile` — Extended profile
- `GET /popup?positions[]=1` — Popup content
- `GET /chat/dialog?perPage=40` — Chat dialogs

## Offer Schema

Full Offer object: `id`, `slug`, `title`, `description`, `primaryPhoto`, `photos`, `price` (`{value, currency}`), `priceType` (1=fixed, 2=negotiable, 3=service), `priceUzs`, `region` (with `titlePath[]`, `location.coordinates`), `location`, `path` (CategoryPath[]), `favorited`, `urgentSale`, `courierDelivery`, `delivery`, `publishedAt` (epoch ms), `webUri`, `webUriInfo` (`{uz, ru}`), `business`, `agency`, `photos`, `seller`, `promotion`, `badges`, `closed`, `analyticsInfo`, `grossPrice`, `grossPriceDiscount`, `similarFeedAvailable`, `translationAvailable`, `bnplForm`, `askDiscount`, `inFavoriteCount`.

The output stored in `birbir_export.jsonl` is raw API JSON pass-through (`serde_json::to_string(offer).unwrap()`). No transformation.

## Collector: birbir-watch

Rust CLI tool. Single `src/main.rs`. Monitors BirBir.uz for new listings.

### Architecture

**Phase 1 — Initial full collection**: Iterates pages from 1 until `nextPageExists=false`. Writes all offers. Sets `initial_complete=true`.

**Phase 2 — Incremental poll**: Fetches page 1 on each run. Writes offers where `id > max_id`. Stops when a page has only known IDs. Appends to existing output.

### Token sources (priority order)

1. Cached `token.txt`
2. Direct `curl` to `https://birbir.uz/` → parse `Set-Cookie: session=...`

JWT expiry checked locally (decode payload, check `exp`). On 401, delete cache, re-fetch, retry.

### Output

`~/.local/share/birbir/birbir_export.jsonl` — JSON Lines, one raw offer object per line.

### State

`~/.local/share/birbir/state.json`: `{"version":1,"max_id":272116974,"initial_complete":true}`

### Configuration

| Env | Default | Description |
|-----|---------|-------------|
| `POLL_INTERVAL` | `0` | Daemon poll interval (ms); 0 = oneshot |

### Operational details

- Data dir: `~/.local/share/birbir/`
- Lock: `birbir.lock` (flock exclusive)
- Token cache: `token.txt`
- Delay: 100ms between pages
- Dependencies: `curl` (system), Rust crates in `Cargo.toml`

### Error conventions

Errors to stderr with `[ERROR]`, `[WARN]`, `[INFO]` prefixes. Token auth failure → exit 1.

### Installation

```bash
cargo build --release
cp target/release/birbir-watch ~/.local/bin/
# systemd timer (every 30 min): birbir-watch.service (oneshot) + birbir-watch.timer (OnCalendar=*:0/30)
```
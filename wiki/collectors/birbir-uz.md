# BirBir.uz — API Reference & Collector

> Sources: birbir.uz WIKI, 2026-07-27; birbir.uz README, 2026-07-27; birbir.uz AGENTS, 2026-07-27; birbir-findings, 2026-07-27; birbir.uz live API investigation, 2026-07-27
> Raw: [birbir.uz_WIKI.md](../../raw/collectors/birbir.uz_WIKI.md); [birbir.uz_README.md](../../raw/collectors/birbir.uz_README.md); [birbir.uz_AGENTS.md](../../raw/collectors/birbir.uz_AGENTS.md); [birbir-findings.md](../../raw/collectors/birbir-findings.md); [birbir-live-api.md](../../birbir.uz/raw/birbir-api/2026-07-27-live-api-investigation.md)
> Updated: 2026-07-27

## Overview

BirBir.uz is an Uzbekistan-based classifieds marketplace. Its API is a REST service hosted at `api.birbir.uz` (version `1.3.5.0`, base path `/api/frontoffice/1.3.5.0`), protected by a Cloudflare JS challenge that gates JWT-based authentication. The collector binary (`birbir-watch`) performs an initial full catalog sync, then polls incrementally for new listings — writing raw API JSON pass-through to a local JSONL file with zero transformation.

## Infrastructure

| Component | Value |
|-----------|-------|
| Domain | `birbir.uz` |
| API | `api.birbir.uz` |
| API version | `1.3.5.0` |
| Base path | `/api/frontoffice/1.3.5.0` |
| Gateway | istio-envoy |
| Backend | PHP/Akene |
| Runtime | React SPA (Webpack) |
| Image CDN | `img.birbir.uz` |
| File CDN | `file.birbir.uz` |
| WebSocket | `socket.birbir.uz` (Centrifugo) |
| Sentry | `sentry.doska-tech.uz` (DSN: `153f7f85e875af52ece61606eceb07cd`) |
| Analytics | Amplitude (`api2.amplitude.com`, app `11821a4e9c78923d2816a71ceb1bf0f2`) |
| Security | Cloudflare JS challenge on main site |

## Authentication

Authentication flows through a Cloudflare JS challenge on the main site, which causes the server to set a `session` cookie. The cookie value is URL-encoded JSON prefixed with `j:`, containing `accessToken` and `refreshToken` (both ES512/ECDSA P-521 JWTs). The `accessToken` is extracted and sent as the `Authorization: Bearer <JWT>` header on API requests.

### JWT Details

- **Algorithm**: ES512 (ECDSA P-521)
- **Expiry**: ~4 hours (`exp - iat = 14400` seconds)
- **Key claims**: `jti` (UUID), `iat`/`exp` (Unix timestamps), `u` (user UUID), `ut` (user type, `10` = regular), `ip`, `t` (token type `1` = access), `dt`/`piat` (ISO 8601 datetimes), `di` (device info), `v` (API version)

### Request Headers

| Header | Value |
|--------|-------|
| `authorization` | `Bearer <JWT>` |
| `x-current-language` | `uz` / `ru` |
| `x-current-region` | region slug (e.g. `toshkent`) |
| `referer` | `https://birbir.uz/` |
| `accept` | `application/json` |
| `content-type` | `application/json` (POST) |

## API Endpoints

### `POST /offer/feed` — Main feed

Returns paginated listings. Body: `{"page":1,"perPage":40,"region":"all","sort":2}`.

- **Success 200**: `{"content":{"items":[...],"paginator":{"step":40,"current":1,"nextPageExists":true}}}`
- **401**: `{"content":null,"error":{"code":"ACCESS_TOKEN_INVALID","message":"Access token invalid."}}`
- **Wrong method**: 405 with `Allow: POST`

Pagination is page-based, controlled by `nextPageExists` in the response paginator.

### `GET /offer/{id}` — Single offer detail

Returns the full offer object including `description`, `features`, `location`, `path`, `askSeller`, `delivery`, `review`, and `activity`. 404 returned for non-existent IDs.

### Other endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/auth/enter-by-phone` | Phone-based auth (requires UI flow) |
| `POST` | `/auth/confirm-code` | SMS confirmation |
| `GET` | `/user` | Current user data (uuid, phone, region, centrifugo WS info) |
| `GET` | `/user/profile` | Extended profile (currency, avatar, language) |
| `GET` | `/popup?positions[]=1` | Popup content |
| `GET` | `/chat/dialog?perPage=40` | Chat dialogs |

## Offer Schema

The feed and detail endpoints return a rich offer object. Key fields:

| Field | Type | Notes |
|-------|------|-------|
| `id` | u64 | Unique ID, monotonically increasing |
| `slug` | string | URL slug |
| `title` | string | Listing title |
| `description` | string | Full description (detail endpoint only) |
| `primaryPhoto` / `photos` | Photo / Photo[] | Image references |
| `price` | Price | `{value, currency: "UZS"\|"USD"}` |
| `priceType` | number | `1`=fixed, `2`=negotiable, `3`=service |
| `priceUzs` | Price | UZS equivalent when currency is USD |
| `region` | Region | Nested `titlePath[]` and `location.coordinates` |
| `location` | Location | Full address (detail endpoint only) |
| `path` | CategoryPath[] | Breadcrumb (detail endpoint only) |
| `favorited` | bool | User-favorited |
| `urgentSale` / `courierDelivery` | bool | Flags |
| `delivery` | Delivery\|null | BirBir delivery info |
| `publishedAt` | number | Epoch ms |
| `webUri` | string | Relative permalink; prefix `https://birbir.uz/` |
| `webUriInfo` | `{uz, ru}` | Localized URIs |
| `business` / `agency` | bool | Account type flags |
| `features` | Feature[] | Tags (detail endpoint only) |
| `seller` | Seller | Nested: uuid, name, verified, business, agency, offerActiveCount, review, activity |
| `promotion` | Promotion | `{enabled, features[]}` |
| `badges` | Badge[] | Listing badges |
| `closed` | bool | |
| `analyticsInfo` | AnalyticsInfo[] | Internal analytics |
| `grossPrice` / `grossPriceDiscount` | Price\|null / number\|null | Discount info |
| `similarFeedAvailable` | bool\|null | |
| `translationAvailable` | boolean | |
| `bnplForm` / `askDiscount` | any\|null | BNPL / discount fields |
| `inFavoriteCount` | number | |

## Collector: birbir-watch

### Architecture

Single-file Rust CLI (`src/main.rs`, no `lib.rs`). Two phases:

**Phase 1 — Initial full collection**: Iterates pages from 1 until `nextPageExists=false`. Writes all offers to the export file. Sets `initial_complete=true` in state.

**Phase 2 — Incremental poll**: Fetches page 1 on each run. Writes offers where `id > max_id`. Stops when a page contains only known IDs. Appends to existing export.

### Token Sources (priority order)

1. Cached `token.txt` in the data directory
2. Direct `curl` to `https://birbir.uz/` → parse `Set-Cookie: session=...`

JWT expiry is checked locally (decode payload, inspect `exp`). On a 401, the cache is deleted, the token is re-fetched, and the request is retried.

### Output

`~/.local/share/birbir/birbir_export.jsonl` — JSON Lines, one raw offer object per line. Each line is the complete API response item passed through via `serde_json::to_string()` — no flattening or transformation. Nested fields (`region.titlePath`, `region.location.coordinates`, `seller.*`) are preserved verbatim.

### State

`~/.local/share/birbir/state.json`:

```json
{"version":1,"max_id":272116974,"initial_complete":true}
```

### Configuration

| Env | Default | Description |
|-----|---------|-------------|
| `POLL_INTERVAL` | `0` | Daemon poll interval in ms; `0` = one-shot |

### Operational Details

- **Data directory**: `~/.local/share/birbir/`
- **Lock file**: `birbir.lock` (exclusive `flock`)
- **State writes**: atomic via `.tmp` + `rename`
- **Token cache**: `token.txt`
- **Delay**: 100ms between pages
- **Dependencies**: `curl` (system), Rust crates from `Cargo.toml`
- **Error logging**: `[ERROR]`, `[WARN]`, `[INFO]` prefixes to stderr; token auth failure exits with code 1

### Installation

```bash
cargo build --release
cp target/release/birbir-watch ~/.local/bin/
```

Systemd user timer (every 30 min) is preferred for continuous operation:

```bash
# Service (Type=oneshot) and timer (OnCalendar=*:0/30)
systemctl --user daemon-reload
systemctl --user enable --now birbir-watch.timer
```

Ad-hoc run: `POLL_INTERVAL=60000 ./target/release/birbir-watch` (daemon mode, 60s poll).

## See Also

- [OLX.uz Collector](../collectors/olx-uz.md) — parallel classifieds collector
- [Uzum.uz Collector](../collectors/uzum-uz.md) — parallel classifieds collector
- [Project Overview](../project.md) — overarching project goals

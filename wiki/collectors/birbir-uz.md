# BirBir.uz — API Reference & Collector

> Sources: birbir.uz WIKI, 2026-07-27; birbir.uz README, 2026-07-27; birbir.uz AGENTS, 2026-07-27; BirBir findings, 2026-07-27; birbir.uz live API investigation, 2026-07-27
> Raw: [BirBir.uz WIKI](../../raw/collectors/2026-07-27-birbir-wiki.md); [BirBir.uz README](../../raw/collectors/2026-07-27-birbir-readme.md); [BirBir.uz AGENTS](../../raw/collectors/2026-07-27-birbir-agents.md); [BirBir findings](../../raw/collectors/2026-07-27-birbir-findings.md); [BirBir.uz live API](../../raw/collectors/2026-07-27-birbir-wiki.md)
> Updated: 2026-07-27

## Overview

BirBir.uz is an Uzbekistan-based classifieds marketplace with a JSON REST/GraphQL API. The API uses JWT authentication (ES512, ECDSA P-521) with Cloudflare JS challenge on the main site. The collector binary (`birbir-watch`) performs an initial full catalog sync, then polls incrementally — writing raw API JSON pass-through to a local JSONL file with zero transformation.

## Infrastructure

| Component | Value |
|-----------|-------|
| **Domain** | `birbir.uz` |
| **API** | `api.birbir.uz` |
| **Version** | `1.3.5.0` |
| **Base** | `/api/frontoffice/1.3.5.0` |
| **Gateway** | istio-envoy |
| **Backend** | PHP/Akene |
| **Runtime** | React SPA (Webpack) |
| **CDN** | `img.birbir.uz` (images), `file.birbir.uz` (static) |
| **WebSocket** | `socket.birbir.uz` (Centrifugo) |
| **Sentry** | `sentry.doska-tech.uz` |
| **Analytics** | Amplitude |
| **Security** | Cloudflare JS challenge on main site |

## Authentication

Cloudflare JS challenge → server sets `session` cookie → cookie is URL-encoded JSON prefixed `j:` → extract `accessToken` → send as `Authorization: Bearer <JWT>`.

### Session Cookie

The `session` cookie contains a URL-encoded JSON object with `accessToken` and `refreshToken` fields. Additional cookies: `user`, `profile` (profile data), `hic` (hit counter), `clickstream-client.installId`.

### JWT

- **Algorithm**: ES512 (ECDSA P-521)
- **Expiry**: ~4h (`exp - iat = 1440`)
- **Claims**: `jti` (UUID), `iat`/`exp` (Unix ts), `u` (user UUID), `ut` (user type: 10=regular), `ip`, `t` (token type: 1=access), `dt`/`piat` (ISO 8601), `di` (device info), `v` (API version)

### Request Headers

| Header | Value |
|--------|-------|
| `authorization` | `Bearer <JWT>` |
| `x-current-language` | `uz` / `ru` |
| `x-current-region` | region slug (e.g. `toshkent`) |
| `referer` | `https://birbir.uz/` |
| `accept` | `application/json` |
| `content-type` | `application/json` (POST) |

## REST Endpoints

### `POST /offer/feed` — Main feed

**Success 200**: `{"content":{"items":[...],"paginator":{"step":40,"current":1,"nextPageExists":true}}}`

**Error 401**: `{"content":null,"error":{"code":"ACCESS_TOKEN_INVALID","message":"Access token invalid."}}`

**Wrong method**: 405 with Allow: POST

### `GET /offer/{id}` — Single offer detail

Returns full object with `description`, `features`, `location`, `path`, `askSeller`, `delivery`, `review`, `activity`. **404**: entity not found.

### Other endpoints

- `POST /auth/enter-by-phone` — Phone auth (requires UI flow)
- `POST /auth/confirm-code` — SMS confirmation
- `GET /user` — Current user data (uuid, phone, region, centrifugo ws info)
- `GET /user/profile` — Extended profile (currency, avatar, language)
- `GET /popup?positions[]=1` — Popup content
- `GET /chat/dialog?perPage=40` — Chat dialogs

## Offer Schema

Full `Offer` object fields from the feed and single-offer endpoints include `id`, `slug`, `title`, `description`, `primaryPhoto`, `photos`, `price`, `priceType`, `priceUzs`, `region`, `location`, `path`, `favorited`, `urgentSale`, `courierDelivery`, `publishedAt`, `webUri`, `webUriInfo`, `business`, `agency`, `features`, `seller`, `promotion`, `badges`, `closed`, `analyticsInfo`, `delivery`, `similarFeedAvailable`, `translationAvailable`, among others.

The output stored in `birbir_export.jsonl` is **raw API JSON pass-through** (`serde_json::to_string(offer).unwrap()`). No transformation. Nested `region.titlePath`, `region.location.coordinates`, `seller.*` are preserved verbatim.

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

- **Data dir**: `~/.local/share/birbir/`
- **Lock**: `birbir.lock` (flock exclusive)
- **State writes**: atomic via `.tmp` + rename
- **Token cache**: `token.txt`
- **Delay**: 100ms between pages
- **Dependencies**: `curl` (system), Rust crates listed in `Cargo.toml`

### Error conventions

Errors to stderr with `[ERROR]`, `[WARN]`, `[INFO]` prefixes. Token auth failure → exit 1.

### Installation

```bash
cargo build --release
cp target/release/birbir-watch ~/.local/bin/

# systemd timer (every 30 min)
# Service: birbir-watch.service (Type=oneshot)
# Timer: birbir-watch.timer (OnCalendar=*:0/30)
```

## Changelog

| Date | Change |
|------|--------|
| 2026-07-27 | Initial API investigation; collector docs; live browser verification; JWT payload breakdown; error codes; full offer schema |
| 2026-07-27 | Correction: output is raw JSON pass-through (was incorrectly documented as flattened) |

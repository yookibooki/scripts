# OLX.uz — API Reference & Collector

> Sources: olx.uz WIKI, 2026-07-27; olx.uz README, 2026-07-27; olx.uz AGENTS, 2026-07-27; olx.uz live API investigation, 2026-07-27
> Raw: [2026-07-27-olx-wiki.md](../../raw/collectors/2026-07-27-olx-wiki.md); [2026-07-27-olx-readme.md](../../raw/collectors/2026-07-27-olx-readme.md); [2026-07-27-olx-agents.md](../../raw/collectors/2026-07-27-olx-agents.md); [olx-api investigation](../../olx.uz/raw/olx-api/2026-07-27-live-api-investigation.md)
> Updated: 2026-07-27

## Overview

OLX.uz is an Uzbekistan-based classifieds marketplace. Its API is a REST service hosted at `www.olx.uz/api/v1` (single endpoint `GET /offers`), requiring no authentication — public reads are unrestricted. The collector binary (`olx-watch`) performs an initial full catalog sync, then polls incrementally for new listings — writing raw API JSON pass-through to a local JSONL file with zero transformation.

## Infrastructure

| Component | Value |
|-----------|-------|
| Domain | `olx.uz` |
| API | `www.olx.uz/api/v1` |
| Endpoint | `GET /offers` |
| Auth | None (public reads) |
| CDN | `frankfurt.apollo.olxcdn.com` (images, `;s={width}x{height}`) |
| Static | `cdn.slots.baxter.olx.org/olxuz/rweb/release/` |
| Infra | CloudFront CDN, nginx origin |
| Runtime | React SPA with SSR (RWeb release) |
| Analytics | Braze, New Relic, Google Analytics, Prebid, Google AdSense |
| Cookies | `csrftoken` (CSRF), `sessionid` (session) |

## API

Single endpoint: `GET /offers`

### Parameters

| Param | Type | Default | Max | Notes |
|-------|------|---------|-----|-------|
| `offset` | u64 | 0 | — | Pagination offset. No hard cap observed; 1000 used as conservative safety margin. |
| `limit` | u64 | 50 | 50 | Advisory. API rejects values > 50. Actual page size ~65 items. |
| `category_id` | u64 | — | — | Per-category pagination resets offset budget. Each category must be paginated independently. |
| `sort_by` | string | — | — | `created_at:desc` (newest first), `created_at:asc` |

### Pagination

- No HAL links, no total count in response body
- has_more heuristic: `offers.len() >= PAGE_SIZE`
- Per-category BFS bypasses any offset limits
- Maximum observed offset is 1000 — queries beyond return empty results

### Response wrapper

```json
{"data": [...], "metadata": {...}, "links": {...}}
```

### Error format

**400**: `{"error":{"status":400,"code":400,"title":"Invalid request","detail":"...","validation":[{"field":"limit","title":"This value should be between 0 and 50."}]}}`

**401**: `{"error":"invalid_token","error_description":"...","error_human_title":"Неверный токен."}`

## Offer Schema

### Top-level

| Field | Type | Notes |
|-------|------|-------|
| `id` | u64 | Unique OLX listing ID, monotonically increasing |
| `url` | string | Direct link to listing |
| `title` | string | Listing title (Uzbek/Russian/Cyrillic) |
| `description` | string\|null | Full description (HTML), may be empty |
| `last_refresh_time` | ISO 8601\|null | UTC+5 |
| `created_time` | ISO 8601\|null | UTC+5 |
| `valid_to_time` | ISO 8601\|null | Expiry date |
| `pushup_time` | ISO 8601\|null | Re-order timestamp |
| `status` | string | `"active"` |
| `offer_type` | string | `"offer"` |
| `business` | bool | Business account listing |
| `isGpsrAvailable` | bool | GPSR compliance flag |
| `params` | array | Listing attributes (see below) |
| `key_params` | array | Key attributes |
| `promotion` | object | `highlighted`, `urgent`, `top_ad`, `options[]`, `b2c_ad_page`, `premium_ad_page` |
| `category` | object | `{id: u64, type: string}` |
| `location` | object | `city:{id,name,normalized_name}`, `district:{id,name}\|null`, `region:{id,name,normalized_name}` |
| `map` | object | `{zoom, lat, lon, radius, show_detailed}` |
| `user` | object | `id`, `uuid`, `name`, `created`, `last_seen`, `is_online`, `seller_type`, `photo`, `logo`, `banner_mobile/desktop`, `company_name`, `about`, `b2c_business_page` |
| `contact` | object | `name`, `phone`, `chat`, `negotiation`, `courier` |
| `photos` | array | `{id, filename, rotation, width, height, link}` |
| `delivery` | object\|null | `rock:{offer_id, active, mode}` |
| `safedeal` | object | `weight`, `weight_grams`, `status`, `safedeal_blocked`, `allowed_quantity[]` |
| `shop` | object\|null | `{subdomain\|null}` |

### params[] — Common parameter keys

| key | type | value shape |
|-----|------|-------------|
| `"price"` | `"price"` | `{value, type, arranged, currency, negotiable, label, converted_value}` |
| `"state"` | `"select"` | `{key:"new"\|"used", label}` |
| `"job_type"` | `"select"` | `{key:"perm"\|"temp"\|"project", label}` |
| `"salary"` | `"salary"` | `{from, to, arranged, currency, gross}` |
| `"job_timing"` | `"select"` | `{key:"full"\|"part"\|"shift", label}` |

### Price (via params array)

Price is inside the `params` array with `key: "price"`:

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

### Location example

```json
{
  "city": { "id": 4, "name": "Ташкент", "normalized_name": "tashkent" },
  "district": { "id": 12, "name": "Мирзо-Улугбекский район" },
  "region": { "id": 5, "name": "Ташкентская область", "normalized_name": "toshkent-oblast" }
}
```

### Promotion example

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

### User/Seller example

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

### Category types

Observed category types: `job`, `automotive`, `electronics`, `real-estate`, `fashion`, `services`, `animals`, `kids`, `home-garden`, `hobby-sport`, `freebies`, `barter`, `business`

## Category system

No dedicated category tree endpoint. Categories are discovered from the `category.id` field in offer responses and from the homepage feed.

| Category | URL Slug | ID |
|----------|----------|----|
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

## Collector: olx-watch

Rust CLI tool. Single `src/main.rs` (no `lib.rs`). Two-phase collection: full sync followed by incremental polling.

### Architecture

**Phase 1 — Full sync**: Seeds from the default (uncategorized) feed → discovers category IDs from `category.id` in offers → BFS over each category (paginates independently) → recursive discovery. Exports unique listings.

**Phase 2 — Incremental sync**: Polls newest-first (`created_at:desc`). Exports listings with `id > max_id`. Stops when an entire page contains only known IDs. Append-only.

### API usage

| Detail | Value |
|--------|-------|
| Endpoint | `GET https://www.olx.uz/api/v1/offers` |
| Auth | None |
| Page size | 50 (max allowed; actual ~65) |
| Max offset | 1000 (conservative safety margin) |
| Retries | 3 attempts, 500ms backoff |

### Output

`~/.local/share/olx/olx_export.jsonl` — JSON Lines, raw API offer objects. Raw pass-through — no transformation whatsoever. Each line is the complete raw offer JSON as returned by the API.

### State

`~/.local/share/olx/state.json`:

```json
{"version":1,"max_id":65506057,"initial_complete":true,"known_categories":[1,2,3,36,37,317,571,899,891,903,1151,1153,1632]}
```

### Configuration

| Env | Default | Description |
|-----|---------|-------------|
| `POLL_INTERVAL` | `0` | Daemon poll interval in ms; `0` = one-shot |

### Operational details

- **Data dir**: `~/.local/share/olx/`
- **Lock**: `olx.lock` (flock exclusive)
- **State writes**: atomic via `.tmp` + rename
- **Delay**: 100ms between pages
- **Modes**: oneshot or daemon

### Installation

```bash
cargo build --release
cp target/release/olx-watch ~/.local/bin/

# systemd timer (every 30 min)
# Service: olx-watch.service (Type=oneshot)
# Timer: olx-watch.timer (OnCalendar=*:0/30)
```

### Error logging

Errors are printed to stderr with `[ERROR]` and `[WARN]` prefixes: HTTP failures (network, timeouts), JSON parse errors, missing fields.

## Changelog

| Date | Change |
|------|--------|
| 2026-07-27 | Initial API investigation; corrected pagination (~65 items/page, no hard 1000 cap, no HAL links); added auth endpoints, CloudFront infra, complete offer interfaces, error shapes |
| 2026-07-27 | Correction: output is raw API JSON pass-through (was incorrectly documented as flattened) |

## See Also

- [BirBir.uz Collector](../collectors/birbir-uz.md) — parallel classifieds collector
- [Uzum.uz Collector](../collectors/uzum-uz.md) — parallel classifieds collector
- [Project Purpose](../../PURPOSE.md) — overarching project goals

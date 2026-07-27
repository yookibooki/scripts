# OLX.uz — API Reference & Collector

> Sources: olx.uz WIKI, 2026-07-27; olx.uz README, 2026-07-27; olx.uz AGENTS, 2026-07-27; olx.uz live API investigation, 2026-07-27
> Raw: [OLX.uz WIKI](../../raw/collectors/2026-07-27-olx-wiki.md); [OLX.uz README](../../raw/collectors/2026-07-27-olx-readme.md); [OLX.uz AGENTS](../../raw/collectors/2026-07-27-olx-agents.md); [OLX.uz live API](../../raw/collectors/2026-07-27-olx-wiki.md)
> Updated: 2026-07-27

## Overview

OLX.uz is an Uzbekistan-based classifieds marketplace. Its API is a REST service hosted at `www.olx.uz/api/v1`, requiring no authentication — public reads are unrestricted. The collector binary (`olx-watch`) performs an initial full catalog sync, then polls incrementally for new listings — writing raw API JSON pass-through to a local JSONL file with zero transformation.

## Infrastructure

| Component | Value |
|-----------|-------|
| **Domain** | `olx.uz` |
| **API** | `www.olx.uz/api/v1` |
| **Auth** | None (public reads) |
| **CDN** | `frankfurt.apollo.olxcdn.com` (images, `;s={width}x{height}`) |
| **Static** | `cdn.slots.baxter.olx.org/olxuz/rweb/release/` |
| **Infra** | CloudFront CDN, nginx origin |
| **Runtime** | React SPA with SSR (RWeb release) |
| **Analytics** | Braze, New Relic, Google Analytics, Prebid, Google AdSense |

## API

Single endpoint: `GET /offers`

### Parameters

| Param | Type | Default | Max | Notes |
|-------|------|---------|-----|-------|
| `offset` | u64 | 0 | — | Pagination offset. No hard cap observed. |
| `limit` | u64 | 50 | 50 | Advisory. Actual page size ~65 items. |
| `category_id` | u64 | — | — | Per-category pagination resets offset budget. |
| `sort_by` | string | — | — | `created_at:desc` (newest first), `created_at:asc` |

### Pagination

- No HAL links, no total count in response body
- has_more heuristic: `offers.len() >= PAGE_SIZE`
- Per-category BFS bypasses any offset limits

### Response

```json
{"data":[...]}
```

### Error format

**400**: `{"error":{"status":400,"code":400,"title":"Invalid request","detail":"...","validation":[{"field":"limit","title":"This value should be between 0 and 50."}]}}`

**401**: `{"error":"invalid_token","error_description":"...","error_human_title":"Неверный токен."}`

## Offer Schema

### Top-level

| Field | Type | Notes |
|-------|------|-------|
| `id` | u64 | Monotonically increasing |
| `url` | string | Full URL |
| `title` | string | Uzbek/Russian/Cyrillic |
| `description` | string\|null | HTML, may be empty |
| `last_refresh_time` | ISO8601\|null | UTC+5 |
| `created_time` | ISO8601\|null | UTC+5 |
| `valid_to_time` | ISO8601\|null | Expiry |
| `pushup_time` | ISO8601\|null | Re-order timestamp |
| `omnibus_pushup_time` | ISO8601\|null | Bulk re-order (sometimes absent) |
| `status` | string | `"active"` |
| `offer_type` | string | `"offer"` |
| `business` | bool | |
| `isGpsrAvailable` | bool | |
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

## Category system

No dedicated category tree endpoint. Discovered from `category` field in offers. Known type slugs: `kids`, `real-estate`, `automotive`, `job`, `animals`, `home-garden`, `electronics`, `services`, `fashion`, `hobby-sport`, `freebies`, `barter`.

## Collector: olx-watch

Rust CLI tool. Single `src/main.rs`. Archives all OLX.uz listings.

### Architecture

**Phase 1 — Full sync**: Seeds from default (uncategorized) feed → discovers category IDs → BFS over each category (paginates independently) → recursive discovery. Exports unique listings.

**Phase 2 — Incremental sync**: Polls newest-first. Exports `id > max_id`. Stops on page of known IDs. Append-only.

### API usage

| Detail | Value |
|--------|-------|
| Endpoint | `GET https://www.olx.uz/api/v1/offers` |
| Auth | None |
| Page size | 50 (max allowed; actual ~65) |
| Max offset | 1000 (conservative safety margin) |
| Retries | 3 attempts, 500ms backoff |

### Output

`~/.local/share/olx/olx_export.jsonl` — JSON Lines, raw API offer objects. Raw pass-through — no transformation.

### State

`~/.local/share/olx/state.json`:
```json
{"version":1,"max_id":65506057,"initial_complete":true,"known_categories":[1,2,3,36,37,317,571,899,891,903,1151,1153,1632]}
```

### Configuration

| Env | Default | Description |
|-----|---------|-------------|
| `POLL_INTERVAL` | `0` | Daemon poll interval (ms); 0 = oneshot |

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

## Changelog

| Date | Change |
|------|--------|
| 2026-07-27 | Initial API investigation; corrected pagination (~65 items/page, no hard 1000 cap, no HAL links); added auth endpoints, CloudFront infra, complete offer interfaces, error shapes |
| 2026-07-27 | Correction: output is raw API JSON pass-through (was incorrectly documented as flattened) |

# OLX.uz Market Data Collector (olx-watch)

> Sources: OLX.uz, 2026-07-27 (live API investigation); source code analysis
> Raw: [Live API Investigation](../../raw/olx-api/2026-07-27-live-api-investigation.md)
> Updated: 2026-07-27

## Overview

A Rust CLI tool that archives all listings from [OLX.uz](https://www.olx.uz) into a machine-readable JSON Lines file. Runs as a systemd user timer for periodic incremental updates. Outputs data to `~/.local/share/olx/olx_export.jsonl`.

## Architecture

Two-phase collection:

### Phase 1: Full Sync
Triggered when `initial_complete=false` in state. Runs exactly once:
1. Fetches the default (uncategorized) listing feed and paginates through it
2. Discovers category IDs from `category.id` field in offers
3. BFS over discovered categories — paginates each independently to bypass the 1000-offset cap
4. Each new category may reveal more categories (recursive discovery)
5. Exports every unique listing as raw API JSON
6. Saves `max_id`, sets `initial_complete=true`, records discovered `known_categories`

### Phase 2: Incremental Sync
Runs on every subsequent execution:
1. Polls the newest-first listing feed
2. Exports listings where `id > max_id`
3. Stops when an entire page contains only already-known IDs (IDs are monotonically increasing)
4. Appends to the existing output file (append-only)

## API Usage

| Detail | Value |
|--------|-------|
| Endpoint | `GET https://www.olx.uz/api/v1/offers` |
| Auth | None (public endpoint) |
| Page size | 50 (max allowed by API) |
| Max offset | 1000 (per-category) |
| Sort | `created_at:desc` |
| Method | Fetch JSON with retries (3 attempts, 500ms backoff) |

### Request Example

```
GET /api/v1/offers?offset=0&limit=50&category_id=1632&sort_by=created_at:desc
```

### Response Parsing

The tool deserializes the response into `ApiResponse { data: Option<Vec<serde_json::Value>> }`. Each offer in the `data` array is written to the output file **as-is** (raw JSON pass-through via `serde_json::to_string(offer).unwrap()`). No field filtering or transformation is performed.

### Known Limits

- **Offset cap**: 1000 per category (conservative limit set in code — actual API has no observed hard cap, but the collector enforces this as a safety margin)
- **Page size**: API returns ~65 items regardless of requested limit; `PAGE_SIZE=50` is set as the max the API allows, but actual returned count may exceed this
- **Rate limiting**: No rate limiting headers observed from API. Sequential 100ms delay is conservative/defensive.
- **Transient failures**: CDN occasionally returns 5xx — 3 retries with backoff
- **Pagination detection**: `has_more = (offers.len() >= PAGE_SIZE)` — if fewer than 50 items returned, assumes no more pages

## Output Format

`~/.local/share/olx/olx_export.jsonl` — JSON Lines, one raw API offer object per line.

Each line is the **complete raw offer JSON** as returned by `GET /api/v1/offers`. The tool performs zero transformation — it writes the `data[]` array items directly.

Example output line:

```json
{
  "id": 65506057,
  "url": "https://www.olx.uz/obyavlenie/rabota/...",
  "title": "Эщик ясашга уста керак",
  "description": "МЕБЕЛЬ ЦЕХИГА УСТА ИШГА ТАКЛИФ ЭТИЛАДИ!",
  "last_refresh_time": "2026-07-27T14:02:03+05:00",
  "created_time": "2026-07-27T13:59:21+05:00",
  "valid_to_time": "2026-08-26T13:59:59+05:00",
  "pushup_time": null,
  "status": "active",
  "offer_type": "offer",
  "business": true,
  "isGpsrAvailable": false,
  "promotion": {
    "highlighted": true,
    "urgent": false,
    "top_ad": true,
    "options": ["bundle_premium"],
    "b2c_ad_page": false,
    "premium_ad_page": false
  },
  "params": [
    { "key": "job_type", "name": "Тип работы", "type": "select", "value": { "key": "perm", "label": "Постоянная работа" } },
    { "key": "salary", "name": "Зарплата", "type": "salary", "value": { "from": 5000000, "to": 7000000, "currency": "UZS" } }
  ],
  "category": { "id": 1632, "type": "job" },
  "location": {
    "city": { "id": 161, "name": "Самарканд", "normalized_name": "samarkand" },
    "region": { "id": 33, "name": "Самаркандская область", "normalized_name": "samarkandskaya-oblast" }
  },
  "map": { "zoom": 12, "lat": 39.65483, "lon": 66.96342, "radius": 18, "show_detailed": false },
  "user": {
    "id": 539284786,
    "name": "Qand",
    "uuid": "b7cf2091-cb39-4f9e-9d44-2d7fe88ce32d",
    "created": "2026-05-25T16:10:46+05:00",
    "other_ads_enabled": true,
    "is_online": false,
    "last_seen": "2026-07-27T13:59:22+05:00",
    "seller_type": null,
    "b2c_business_page": false
  },
  "contact": { "name": "Qand", "phone": true, "chat": true, "negotiation": false, "courier": false },
  "photos": [],
  "delivery": { "rock": { "offer_id": null, "active": false, "mode": null } },
  "safedeal": { "weight": 0, "weight_grams": 0, "status": "unactive", "safedeal_blocked": false, "allowed_quantity": [] },
  "shop": { "subdomain": null }
}
```

### Important Notes

- The output is **not flattened**. Each line contains the full nested API response as-is.
- Price information is inside the `params[]` array (look for `key: "price"`).
- Category is the `category` object (`{ id, type }`), not a flat string.
- Location is a nested `location` object.
- Coordinates are in the `map` object (`lat`, `lon`).
- Some fields may be null/absent depending on the listing type.
- The schema is documented in full in [olx-api-reference.md](olx-api-reference.md).

## State Schema

`~/.local/share/olx/state.json`:

```json
{
  "version": 1,
  "max_id": 65506057,
  "initial_complete": true,
  "known_categories": [1, 2, 3, 36, 37, 317, 571, 899, 891, 903, 1151, 1153, 1632]
}
```

- `max_id`: Highest seen listing ID
- `initial_complete`: Whether Phase 1 completed
- `known_categories`: All discovered category IDs

## Operational Details

- **Data directory**: `~/.local/share/olx/`
- **Lock file**: `olx.lock` (flock-based exclusive lock)
- **State**: `state.json` (written atomically via `.tmp` + rename)
- **Output**: `olx_export.jsonl`
- **Modes**: oneshot (default) or daemon via `POLL_INTERVAL` env var (ms)
- **Delay**: 100ms between pages to avoid rate limiting

## Configuration

| Env Variable | Description | Default |
|-------------|-------------|---------|
| `POLL_INTERVAL` | Daemon poll interval in ms | 0 (oneshot) |

## Installation

```bash
cargo build --release
cp target/release/olx-watch ~/.local/bin/

# systemd timer (every 30 min)
cat > ~/.config/systemd/user/olx-watch.service << 'EOF'
[Unit]
Description=OLX.uz new posts watch

[Service]
Type=oneshot
ExecStart=%h/.local/bin/olx-watch
EOF

cat > ~/.config/systemd/user/olx-watch.timer << 'EOF'
[Unit]
Description=OLX.uz poll timer (every 30 min)

[Timer]
OnCalendar=*:0/30
Persistent=true

[Install]
WantedBy=timers.target
EOF

systemctl --user daemon-reload
systemctl --user enable --now olx-watch.timer
```

# OLX Watch
## Purpose
Maintain a complete OLX.uz listing archive:

1. Initial full collection of all active listings.
2. Incremental collection of newly created listings.

## Build
```bash
cargo build --release
```

## Structure
* `src/main.rs` — collection logic.
* `src/lib.rs` — HTTP, parsing, models, helpers.

## Collection
### Phase 1: Full Sync
* Triggered when `initial_complete=false`.
* Discover categories via BFS.
* Paginate each category separately to bypass OLX's 1000-offset limit.
* Export all unique listings.
* Save:

  * `max_id`
  * `initial_complete=true`
  * `known_categories`

### Phase 2: Incremental Sync
* Poll newest-first listing feed.
* Export listings with `id > max_id`.
* Stop when an entire page contains only known listings.
* Append only; never delete data.

## Storage
Directory: `~/.local/share/olx/`

Files:

* `state.json` → `{ max_id, initial_complete, known_categories }`
* `olx_export.jsonl` → collected listings

## Export Schema
Listing:
`id,url,title,business,created_time,last_refresh_time`

Derived:
`price_uzs,category_type`

Location:
`location_city,location_district,location_region,coordinates`

## State Schema

```json
{
  "version": 1,
  "max_id": 123,
  "initial_complete": true,
  "known_categories": [1, 2, 3]
}
```

Missing `version` field is treated as version 1.

## Operational Invariants

* Export file is append-only.
* `max_id` must never decrease.
* State writes are atomic (write to `.tmp`, then `rename`).
* Only one instance may run at a time (enforced via `flock`).
* Initial collection must complete before incremental polling begins.

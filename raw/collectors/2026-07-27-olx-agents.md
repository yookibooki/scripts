# OLX Watch

> Source: olx.uz/AGENTS.md
> Collected: 2026-07-27
> Published: 2026-07-27

## Purpose
Maintain a complete OLX.uz listing archive:
1. Initial full collection of all active listings.
2. Incremental collection of newly created listings.

## Build
```bash
cargo build --release
```

## Structure
* `src/main.rs` — collection logic (single file, no lib.rs).

## Collection

### Phase 1: Full Sync
* Triggered when `initial_complete=false`.
* Fetch default (uncategorized) listing feed, discover category IDs from `category.id`.
* BFS over discovered categories, paginating each independently.
* Export all unique listings as raw API JSON (pass-through, no transformation).

### Phase 2: Incremental Sync
* Poll newest-first listing feed (`created_at:desc`).
* Export listings with `id > max_id`.
* Stop when an entire page contains only known listings.
* Append only; never delete data.

## API
- Endpoint: `GET https://www.olx.uz/api/v1/offers`
- Auth: None (public endpoint)
- Pagination: offset/limit. API returns ~65 items per page (limit is advisory). No hard offset cap.
- Page size: 50 (set in code, actual returned count ~65)
- Sort: `created_at:desc` (newest first)

## Storage
Directory: `~/.local/share/olx/`
Files: `state.json` → `{max_id, initial_complete, known_categories}`, `olx_export.jsonl` → raw API offer JSON, one per line.

## Output
Raw API offer JSON, one per line. Each line is the complete offer object as returned by the OLX API — no flattening or transformation.

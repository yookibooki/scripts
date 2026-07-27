# BirBir Watch

> Source: birbir.uz/AGENTS.md
> Collected: 2026-07-27
> Published: 2026-07-27

## Purpose
Collect all BirBir.uz offers once, then append only newly created offers to a JSONL export.

## Build
```bash
cargo build --release
```

## Architecture
* `src/main.rs` — collector logic (single file, no lib.rs).

## Auth
* Bearer token from BirBir session cookie (Cloudflare-protected).
* Cached in `~/.local/share/birbir/token.txt`.
* Validate JWT expiry before use (ES512, 4-hour expiry).
* Refresh on 401 (delete cache, re-obtain via direct HTTP).
* Token sources: cached → agent-browser → curl → stale.

## Collection

### Initial run
* Fetch every page from `POST /api/frontoffice/1.3.5.0/offer/feed` until `nextPageExists=false`.
* Track highest seen ID.

### Subsequent runs
* Start at page 1.
* Export offers with `id > max_id`.
* Stop when a page contains only known IDs.

## API
- Endpoint: `POST https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/feed`
- Body: `{ page, perPage: 40, region: "all", sort: 2 }`
- Pagination: page-based, `nextPageExists` in response paginator.

## Persistence
Directory: `~/.local/share/birbir/`
Files: `state.json`, `birbir_export.jsonl`, `token.txt` (cached auth token)

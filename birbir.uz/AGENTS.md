# BirBir Watch
## Purpose
Collect all BirBir.uz offers once, then append only newly created offers to a JSONL export.

## Build
```bash
cargo build --release
```

## Architecture
* `src/main.rs` — collector logic (single file, no lib.rs).

## Auth
* Bearer token comes from BirBir session cookie (Cloudflare-protected).
* Cached in `~/.local/share/birbir/token.txt`.
* Validate JWT expiry before use (ES512, 4-hour expiry).
* Refresh on 401 (delete cache, re-obtain via direct HTTP).
* Token sources: cached → direct curl.

## Collection Rules
### Initial run
* Fetch every page from `POST /api/frontoffice/1.3.5.0/offer/feed` until `nextPageExists=false`.
* Export unique offers as raw API JSON (pass-through, no transformation).
* Track highest seen ID.

### Subsequent runs
* Start at page 1.
* Export offers with `id > max_id`.
* Stop when a page contains only known IDs.

## API
- **Endpoint**: `POST https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/feed`
- **Auth**: `Authorization: Bearer <JWT>` (required)
- **Body**: `{ page, perPage: 40, region: "all", sort: 2 }`
- **Pagination**: page-based, controlled by `nextPageExists` in response paginator.

## Persistence
Directory: `~/.local/share/birbir/`

Files:
* `state.json` → `{ max_id, initial_complete }`
* `birbir_export.jsonl` → raw API offer JSON, one per line (pass-through)
* `token.txt` → cached auth token

## Output
Raw API offer JSON, one per line. Each line is the complete offer object as returned by the BirBir API — no flattening or transformation. Uses `serde_json::to_string(offer).unwrap()` pass-through.

## State Schema
```json
{
  "version": 1,
  "max_id": 123,
  "initial_complete": true
}
```

## Operational Invariants
* Export file is append-only.
* `max_id` must never decrease.
* State writes are atomic (write to `.tmp`, then `rename`).
* Only one instance may run at a time (enforced via `flock`).
* Initial collection must complete before incremental polling begins.

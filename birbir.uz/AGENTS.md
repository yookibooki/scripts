# BirBir Watch
## Purpose
Collect all BirBir.uz offers once, then append only newly created offers to a JSONL export.

## Build
```bash
cargo build --release
```

## Architecture
* `src/main.rs` — collector logic.
* `src/lib.rs` — auth, HTTP, models, helpers.

## Auth
* Bearer token comes from BirBir session cookie.
* Cached in `~/.local/share/birbir/token.txt`.
* Validate JWT expiry before use.
* Refresh on 401.

## Collection Rules
### Initial run
* Fetch every page from `POST /offer/feed` until `nextPageExists=false`.
* Export unique offers.
* Track highest seen ID.

### Subsequent runs
* Start at page 1.
* Export offers with `id > max_id`.
* Stop when a page contains only known IDs.

## Persistence
Directory: `~/.local/share/birbir/`

Files:

* `state.json` → `{ max_id, initial_complete }`
* `birbir_export.jsonl` → exported offers
* `token.txt` → cached auth token

## Exported Fields
Offer:
`id,title,price,publishedAt,webUri,urgentSale,courierDelivery,business,agency,closed`

Region:
`titlePath,coordinates`

Seller:
`uuid,name,verified,business,agency,offerActiveCount`

## State Schema

```json
{
  "version": 1,
  "max_id": 123,
  "initial_complete": true
}
```

Missing `version` field is treated as version 1.

## Operational Invariants

* Export file is append-only.
* `max_id` must never decrease.
* State writes are atomic (write to `.tmp`, then `rename`).
* Only one instance may run at a time (enforced via `flock`).
* Initial collection must complete before incremental polling begins.

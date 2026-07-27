# Uzum Watch (uzum-watch)
## Purpose
Continuously archive the complete Uzum Marketplace catalog for AI agentic trading analysis.

## Build
```bash
cargo build --release
cp target/release/uzum-watch ~/.local/bin/
```

## Structure
* `src/main.rs` — collection logic (single file).

## Auth
* `UZUM_ACCESS_TOKEN` and `UZUM_INSTALL_ID` read from environment.
* JWT tokens expire ~10 hours after issue — refresh from browser session.
* Token stored in `~/.config/uzum/env`.

## Collection
### Full mode (default)
* Fetch category tree from `GET /api/main/root-categories?eco=false`.
* Scan all leaf categories sequentially.
* For each category: GraphQL `MakeSearch_ItemsAndFilters` with limit=100.
* Paginate within each category up to offset=9900 cap.
* Output: raw SearchItem JSON (pass-through), one per line + header line.
* State persisted every 50 categories.

### Refresh mode (`--refresh`)
* Compare saved API totals against current totals per category.
* Only deep-scan categories where total increased or collection didn't complete.
* Append to existing output file.

## API
- **REST**: `GET https://api.uzum.uz/api/main/root-categories?eco=false` (category tree)
- **GraphQL**: `POST https://graphql.uzum.uz/` (product search)
- **Auth**: Bearer JWT + x-iid header
- **Pagination**: offset/limit, max safe offset=9900 (with batch size 100)

## Storage
Directory: `~/.local/share/uzum/`

Files:
* `state.json` → per-category progress
* `uzum_data.jsonl` → output (raw SearchItem JSON)
* `uzum.lock` → flock-based exclusive lock

## Output
JSON Lines with a header line followed by raw SearchItem JSON objects.
Header: `{"exportedAt":"...","totalProducts":0,"version":"1.0.0","source":"uzum.uz"}`
Each data line: a SearchItem with `catalogCard` wrapper (as returned by GraphQL API, pass-through).

## State Schema
```json
{
  "version": 1,
  "categories": {
    "123": { "total": 500, "offset": 500 }
  },
  "item_count": 248668,
  "updated_at": "2026-07-26T12:00:00.000Z"
}
```

## Operational Invariants
* Output file has header line (first line), then one SearchItem per line.
* State is written atomically (`.tmp` + rename).
* Only one instance allowed (flock on `.lock` file).
* Page size: 100 (BATCH_SIZE). Max offset: 9900 (OFFSET_CAP).

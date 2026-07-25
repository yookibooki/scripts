# Uzum Marketplace Collector

## Purpose

Maintain a complete product catalog archive from Uzum.uz:

1. Initial full collection of all products across all leaf categories.
2. Refresh: re-scan categories whose totals changed or weren't fully collected.
3. Incremental on re-run: append new data on top of existing data (not a fresh start).

## Install

Install `userscript.js` in Tampermonkey, then open `https://uzum.uz/`. A control panel appears top-right with **Start**, **Stop**, and **Export JSON** buttons.

## Structure

- `userscript.js` — Tampermonkey userscript (single file, everything inline).
- `API_REFERENCE.md` — detailed API documentation (GraphQL schema, auth, endpoints, known quirks).
- `AGENTS.md` — this file.

## Collection

### Category discovery

- Fetches the full category tree from `GET /api/main/root-categories` (REST, no pagination).
- Flattens the tree to **leaf categories** (nodes with no `children`).
- Leaf count: **652** as of 2026-07-25.

### Parallel scan

- Each leaf category is scanned via GraphQL `MakeSearch_ItemsAndFilters` query.
- Default page size: **100** (API max).
- Concurrency: **10** parallel requests.
- Delay between requests: **400ms**.
- Offset limit: **9901** (batch of 100). Categories exceeding this are truncated.
- Progress saved every **50 categories** to IndexedDB (`cat_totals`).

### Resume

- On startup, reads `cat_totals` from IndexedDB state.
- For each category: resumes from the saved offset, skipping already-scanned pages.
- If a category's `total` changed since last scan, the new range is re-scanned.

### Refresh behavior

- Re-running collection checks current totals for all categories.
- Deep-scans categories where totals increased or collection didn't complete.
- Existing products are updated in-place (new `lastSeen` timestamp).

### Rate limiting

- HTTP 429: retry once after 2s delay.
- 3 consecutive failures on a category: skip it entirely (logged).
- No evidence of IP-based rate limiting at concurrency 10.

## Storage

**IndexedDB** (`uzum_product_db`, version 3):

| Object Store | Key | Purpose |
|-------------|-----|---------|
| `products` | `id` (productId) | Full product catalog |
| `state` | `key` | KV state (cat_totals, status) |

On startup, a slim pass strips `url`, `images`, and `priceHistory` fields from all stored products.

## Export Schema

Each product in IndexedDB / JSONL:

```
id,title,price,oldPrice,discountPercent,rating,reviewCount,category,categoryId,firstSeen,lastSeen
```

## State Schema

`state` store keys:

| Key | Value | Purpose |
|-----|-------|---------|
| `cat_totals` | `{ [categoryId]: { total, offset } }` | Per-category scan progress |
| `status` | `"collection_done"` or `"stopped"` | Overall collection status |

`cat_totals` shape:

```json
{
  "75": { "total": 500, "offset": 600 },
  "76": { "total": 1200, "offset": 1200 }
}
```

- `total`: API-reported total products for that category at last scan.
- `offset`: next offset to scan (equals `total` when complete, capped at 9901).

## API Reference

See [API_REFERENCE.md](./API_REFERENCE.md) for:
- GraphQL query and variables
- REST category tree endpoint
- Auth (cookies, headers)
- Full list of known restricted categories and mid-scan failures

## Operational Invariants

- IndexedDB is the single source of truth — export is a snapshot.
- Products are upserted: re-running updates existing entries (new `lastSeen`) and inserts new ones.
- The 10K offset cap means categories with >10K items are inherently truncated.
- Auth-restricted categories (food, cosmetics, pharmacy, adult) require logging into uzum.uz in the same browser session.
- Tampermonkey MCP + Chrome DevTools MCP are used for live development and debugging.

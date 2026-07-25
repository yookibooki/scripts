# Uzum Marketplace Collector

Tampermonkey userscript that collects the full product catalog from [Uzum.uz](https://uzum.uz) (Uzbekistan's largest marketplace) into IndexedDB and exports as JSONL — ~248K products across 652 leaf categories.

## Output format

Exported JSONL has a header line then one product per line:

```json
{
  "id": 123456,
  "title": "Product Name",
  "price": 80000,
  "oldPrice": 100000,
  "discountPercent": 20,
  "rating": 4.5,
  "reviewCount": 42,
  "category": "Kategoriya nomi",
  "categoryId": 12345,
  "firstSeen": "2026-07-25T12:00:00.000Z",
  "lastSeen": "2026-07-25T12:00:00.000Z"
}
```

| Field | Description |
|-------|-------------|
| `id` | Numeric product ID (primary key) |
| `title` | Product title |
| `price` | Current/sale price (UZS) |
| `oldPrice` | Original/full price (UZS) |
| `discountPercent` | Computed discount percentage |
| `rating` | Product rating (float, nullable) |
| `reviewCount` | Number of reviews |
| `category` | Leaf category name |
| `categoryId` | Leaf category ID |
| `firstSeen` | ISO timestamp of first discovery |
| `lastSeen` | ISO timestamp of last observation |

## Quick start

1. Install the [Tampermonkey](https://www.tampermonkey.net/) browser extension.
2. Open `userscript.js` and install it (Tampermonkey will detect it as a userscript).
3. Navigate to `https://uzum.uz/`.
4. Click **Start** in the panel (top-right corner).

### Via Tampermonkey Editors (Chrome DevTools MCP)

```bash
# Connect to the browser, then:
# 1. Open the Tampermonkey Editors extension
# 2. Paste or sync userscript.js
# 3. Reload uzum.uz
```

## Files

| File | Purpose |
|------|---------|
| `userscript.js` | Tampermonkey userscript — collection + export |
| `API_REFERENCE.md` | Detailed API docs for GraphQL + REST endpoints |
| `AGENTS.md` | Project overview for LLM-assisted development |
| `~/.commandcode/taste/taste.md` | Learned preferences (see AGENTS.md) |

## Known limits

- **10K offset cap**: GraphQL rejects `offset + limit > 10000`. Categories with >10K items (~3 exist) are truncated to the first ~10K.
- **Auth-restricted categories**: ~85 categories (food, cosmetics, pharmacy, adult) return `total` but 0 items without login. Log into uzum.uz to fetch them.
- **Mid-scan failures**: ~18 categories fail intermittently at specific offsets (partial data collected).
- **Output is from IndexedDB**: Click **Export JSON** in the panel to produce a JSONL download.

## Error logging

Logs appear in the panel log area and browser console with `[UzumCollector]` prefix:
- API failures (timeouts, 429 rate limits)
- GraphQL errors
- Categories that fail 3 consecutive requests (skipped)
- Progress updates every 50 categories

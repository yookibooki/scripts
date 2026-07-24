# Uzum Marketplace Userscript

> Tampermonkey userscript running on `uzum.uz` — collects the full product catalog into IndexedDB and exports as JSONL/JSON.

## Architecture

### Tech Stack
- **Runtime**: Tampermonkey browser extension (GM_* APIs)
- **Storage**: IndexedDB (`uzum_product_db`, version 3)
- **API**: Uzum's GraphQL (`graphql.uzum.uz`) + REST (`api.uzum.uz/api`)
- **Auth**: Cookie-based (`access_token`, `clickstream-client.installId`)

### Key Constants
- `BATCH_SIZE`: 48 products per page
- `REQUEST_DELAY_MS`: 400ms between requests
- `SAVE_INTERVAL`: 50 products — saves resume state every 50 collected
- `OFFSET_LIMIT`: 9936 — max safe offset (offset + limit < 10000)
- `GRAPHQL_URL`: `https://graphql.uzum.uz/`
- `REST_BASE`: `https://api.uzum.uz/api`

### API Limits
- **GraphQL offset limit**: max offset is 9951 (offset + 48 < 10000). Returns `"too big query offset"` past that.
- **Total catalog**: ~138,823 products across 1,627 leaf categories.
- **3 categories exceed limit**: Makiyaj (21K), Psixologiya (19K), Badiiy adabiyot (15K) — first ~10K only.
- **Categories API**: `/main/root-categories` (no `?eco=false`, returns 400 with that param). Returns `{payload: [...]}`.

### Files
- `userscript.js` — single-file Tampermonkey userscript (v2.3.0)

## Session Notes
- Connected via Chrome DevTools MCP + Tampermonkey MCP
- Browser open at `uzum.uz`
- Userscript requires the browser to be on uzum.uz to run

# Uzum.uz Collector

> Sources: uzum.uz/AGENTS.md, 2026-07-27; API investigation, 2026-07-27; network requests, 2026-07-27
> Raw: [agents research](../../raw/collectors/2026-07-27-uzum-agents.md); [wiki compilation](../../raw/collectors/2026-07-27-uzum-wiki.md); [network requests](../../raw/collectors/2026-07-27-uzum-network-requests.md); [live snapshot](../../raw/collectors/2026-07-27-uzum-live-snapshot.md)
> Updated: 2026-07-28

Binary: `uzum-watch`. Crate: `scripts/uzum.uz/`. Single `main.rs`.

## API

Two endpoints used:

| Endpoint | Type | Purpose |
|----------|------|---------|
| `GET /api/main/root-categories?eco=false` | REST | Fetch category tree |
| `POST https://graphql.uzum.uz/` | GraphQL | Product search (`MakeSearch_ItemsAndFilters`) |

Infrastructure: GraphQL/Apollo on `graphql.uzum.uz`, REST on `api.uzum.uz`, images on `images.uzum.uz`. Web app version 1.63.2.

## Authentication

Two environment variables required:
- `UZUM_ACCESS_TOKEN` — JWT token (~10h expiry)
- `UZUM_INSTALL_ID` — install ID from cookie

Sent as `Authorization: Bearer <JWT>` and `X-Iid: <installId>` headers on both REST and GraphQL requests.

## Collection

### Category tree
Fetched via `GET /api/main/root-categories?eco=false`. Returns recursive tree with `id`, `title`, `children`, `path`. Leaf nodes have empty `children`.

As of 2026-07-28 the tree has 1,846 total nodes: 1,627 leaves, 219 parents.

### Full mode (default)
Traverses leaf categories sequentially. For each leaf: sends GraphQL `MakeSearch_ItemsAndFilters` query with batch size 100. Paginates up to offset cap 9900. New output file with JSON header line.

### Refresh mode (`--refresh`)
Compares saved per-category totals against current API totals. Only deep-scans categories where total increased or collection didn't complete. Appends to existing output.

### Offset cap
API errors when `offset + limit > 10000`. Max safe offset is 9900 with batch 100. Categories with >10,000 products are truncated at 9,900.

### GraphQL query

```graphql
query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) {
  makeSearch(query: $queryInput) {
    items { catalogCard { productId title minFullPrice minSellPrice ... } }
    total
  }
}
```

Variables: `categoryId`, `showAdultContent: "TRUE"`, `sort: "BY_ORDERS_NUMBER_DESC"`, `pagination: {offset, limit}`.

## State

`~/.local/share/uzum/state.json`:

```json
{"version":1,"categories":{"123":{"total":500,"offset":500}},"item_count":248668,"updated_at":"2026-07-26T12:00:00.000Z"}
```

## Output

`~/.local/share/uzum/uzum_data.jsonl` — JSON Lines with header line, then one `catalogCard` JSON object per line.

## Installation

```bash
cd scripts/uzum.uz
cargo build --release
cp target/release/uzum-watch ~/.local/bin/
```

### systemd

Service: `uzum-watch.service` (Type=oneshot)
Timer: `uzum-watch.timer` (OnCalendar=*:10/30)

### Auth setup

```bash
cat > ~/.config/uzum/env << 'EOF'
UZUM_ACCESS_TOKEN=your_jwt_token
UZUM_INSTALL_ID=your_install_id
EOF
chmod 600 ~/.config/uzum/env
```

### Known limits

- Offset cap at 9900 → categories with >10K products truncated
- Token expiry ~10h — re-fetch from browser session
- Concurrent requests >1 → HTTP 429 (rate limited)

# OLX.uz Collector

> Sources: olx.uz/README.md, 2026-07-27; olx.uz/AGENTS.md, 2026-07-27; API investigation, 2026-07-27
> Raw: [readme](../../raw/collectors/2026-07-27-olx-readme.md); [agents research](../../raw/collectors/2026-07-27-olx-agents.md); [wiki compilation](../../raw/collectors/2026-07-27-olx-wiki.md)
> Updated: 2026-07-28

Binary: `olx-watch`. Crate: `scripts/olx.uz/`. Single `main.rs`.

## API

`GET https://www.olx.uz/api/v1/offers`

No authentication required (public read endpoint). Parameters: `offset`, `limit` (max 50, actual returned ~65), `category_id`, `sort_by`.

No total count in response. `has_more` heuristic: `offers.len() >= PAGE_SIZE`.

Infrastructure: CloudFront CDN, nginx origin, React SPA with SSR.

## Collection

### Phase 1 — Full sync
Seeds from default (uncategorized) feed → discovers category IDs from `category.id` field → BFS over each discovered category → paginates each independently. Recursive category discovery.

### Phase 2 — Incremental sync
Polls newest-first (`created_at:desc`). Exports offers with `id > max_id`. Stops when a page contains only known IDs. Append-only.

### Category system

No dedicated category tree endpoint. Discovered from offer `category` field. Known type slugs: `kids`, `real-estate`, `automotive`, `job`, `animals`, `home-garden`, `electronics`, `services`, `fashion`, `hobby-sport`, `freebies`, `barter`.

## State

`~/.local/share/olx/state.json`:

```json
{"version":1,"max_id":65506057,"initial_complete":true,"known_categories":[1,2,3,36,37,317,571,899,891,903,1151,1153,1632]}
```

## Output

`~/.local/share/olx/olx_export.jsonl` — one raw offer JSON object per line.

## Installation

```bash
cd scripts/olx.uz
cargo build --release
cp target/release/olx-watch ~/.local/bin/
```

### systemd

Service: `olx-watch.service` (Type=oneshot)
Timer: `olx-watch.timer` (OnCalendar=*:0/30)

## Configuration

| Env | Default | Description |
|-----|---------|-------------|
| `POLL_INTERVAL` | `0` | Daemon poll interval (ms); 0 = oneshot |

## Operational details

- Lock: `olx.lock` (flock exclusive)
- State writes: atomic via `.tmp` + rename
- Delay: 100ms between pages
- Retries: 3 attempts, 500ms backoff

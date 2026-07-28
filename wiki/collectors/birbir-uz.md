# BirBir.uz Collector

> Sources: birbir.uz/README.md, 2026-07-27; birbir.uz/AGENTS.md, 2026-07-27; API investigation, 2026-07-27
> Raw: [readme](../../raw/collectors/2026-07-27-birbir-readme.md); [agents research](../../raw/collectors/2026-07-27-birbir-agents.md); [API findings](../../raw/collectors/2026-07-27-birbir-findings.md); [wiki compilation](../../raw/collectors/2026-07-27-birbir-wiki.md)
> Updated: 2026-07-28

Binary: `birbir-watch`. Crate: `scripts/birbir.uz/`. Single `main.rs`.

## API

`POST https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/feed`

Page-based pagination. Body: `{"page":1, "perPage":40, "region":"all", "sort":2}`. Response includes `paginator.nextPageExists`.

Infrastructure: istio-envoy gateway, PHP/Akene backend, Cloudflare JS challenge on main site.

## Authentication

BirBir requires Bearer JWT extracted from the `session` cookie (Cloudflare-protected). The cookie is URL-encoded JSON with `j:` prefix containing `accessToken` and `refreshToken`.

Token sources in priority order:
1. Cached `token.txt` in data dir
2. Direct `curl` to `https://birbir.uz/` → parse `Set-Cookie: session=...`

JWT: ES512 algorithm, ~4h expiry. Checked locally before use. On 401, cache is deleted and token re-fetched.

## Collection phases

### Phase 1 — Initial full collection
Iterates pages from 1 until `nextPageExists=false`. Writes all offers. Sets `initial_complete=true` in state.

### Phase 2 — Incremental poll
Fetches page 1 on each run. Writes offers with `id > max_id`. Stops when a page contains only known IDs. Appends to existing output.

## State

`~/.local/share/birbir/state.json`:

```json
{"version":1,"max_id":272116974,"initial_complete":true}
```

## Output

`~/.local/share/birbir/birbir_export.jsonl` — one raw offer JSON object per line.

## Installation

```bash
cd scripts/birbir.uz
cargo build --release
cp target/release/birbir-watch ~/.local/bin/
```

### systemd

Service: `birbir-watch.service` (Type=oneshot)
Timer: `birbir-watch.timer` (OnCalendar=*:5/30)

## Configuration

| Env | Default | Description |
|-----|---------|-------------|
| `POLL_INTERVAL` | `0` | Daemon poll interval (ms); 0 = oneshot |

## Operational details

- Lock: `birbir.lock` (flock exclusive)
- State writes: atomic via `.tmp` + rename
- Delay: 100ms between pages
- Dependencies: `curl` (system token fetch), Rust crates

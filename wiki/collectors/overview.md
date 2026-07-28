# Overview

> Sources: PURPOSE.md, 2026-07-27; raw investigation docs, 2026-07-27
> Raw: [project purpose](../../raw/collectors/2026-07-27-purpose.md)
> Updated: 2026-07-28

## Purpose

Archive classifieds and marketplace listings from BirBir.uz, OLX.uz, and Uzum.uz into JSONL exports. The dataset feeds downstream AI-driven trading analysis.

## Architecture

Three independent Rust binaries, each in its own crate under `scripts/`. Each performs:

1. **Initial full sync** — collects all existing listings/products
2. **Incremental sync** — periodic poll for new/changed listings, append-only

State is persisted locally in `~/.local/share/<site>/state.json` for idempotent runs.

Output is one JSONL file per site:
- `~/.local/share/birbir/birbir_export.jsonl`
- `~/.local/share/olx/olx_export.jsonl`
- `~/.local/share/uzum/uzum_data.jsonl`

### Collection mechanisms

| Site | API type | Auth | Pagination |
|------|----------|------|------------|
| BirBir.uz | REST POST | Bearer JWT (Cloudflare session) | Page-based, `nextPageExists` |
| OLX.uz | REST GET | None (public) | Offset/limit, BFS category discovery |
| Uzum.uz | GraphQL | Bearer JWT + install ID | Offset/limit per leaf category, max offset 9900 |

### Scheduling

Each site runs via systemd user timer every 30 minutes, staggered to avoid thundering herd:
- OLX: `*:0/30`
- BirBir: `*:5/30`
- Uzum: `*:10/30`

### Data directory layout

```
~/.local/share/<site>/
  state.json         — collection progress, cursor position
  <export>.jsonl     — output data
  <site>.lock        — flock exclusive lock
```

## See Also

- [BirBir.uz](birbir-uz.md) — BirBir collector details
- [OLX.uz](olx-uz.md) — OLX collector details
- [Uzum.uz](uzum-uz.md) — Uzum collector details

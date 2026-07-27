# Project Overview — Classifieds Archive Collectors

> Sources: Purpose, 2026-07-27; Dev Principles, 2026-07-27; olx.uz WIKI, 2026-07-27; olx.uz README, 2026-07-27; olx.uz AGENTS, 2026-07-27; OLX.uz live API, 2026-07-27; BirBir.uz WIKI, 2026-07-27; BirBir.uz README, 2026-07-27; BirBir.uz AGENTS, 2026-07-27; BirBir findings, 2026-07-27; BirBir live API, 2026-07-27; Uzum.uz WIKI, 2026-07-27; Uzum.uz AGENTS, 2026-07-27; Uzum e2e analysis, 2026-07-25; Uzum live API, 2026-07-25; Uzum page snapshot, 2026-07-25; idlewatch script, 2026-07-27
> Raw: [Purpose](../raw/collectors/2026-07-27-purpose.md); [Dev Principles](../raw/collectors/2026-07-27-agents.md); [OLX.uz WIKI](../raw/collectors/2026-07-27-olx-wiki.md); [OLX.uz README](../raw/collectors/2026-07-27-olx-readme.md); [OLX.uz AGENTS](../raw/collectors/2026-07-27-olx-agents.md); [BirBir.uz WIKI](../raw/collectors/2026-07-27-birbir-wiki.md); [BirBir.uz README](../raw/collectors/2026-07-27-birbir-readme.md); [BirBir.uz AGENTS](../raw/collectors/2026-07-27-birbir-agents.md); [BirBir findings](../raw/collectors/2026-07-27-birbir-findings.md); [Uzum.uz WIKI](../raw/collectors/2026-07-27-uzum-wiki.md); [Uzum.uz AGENTS](../raw/collectors/2026-07-27-uzum-agents.md); [idlewatch](../raw/collectors/linux-idlewatch.sh)
> Updated: 2026-07-27

## Purpose

Continuously archive classifieds and marketplace listings from BirBir.uz, OLX.uz, and Uzum.uz into lossless JSONL exports. The resulting dataset feeds downstream AI-driven trading analysis, market intelligence, and trend modeling.

Each collector is a lean Rust binary that performs an initial full catalog sync, then polls incrementally — writing raw API JSON pass-through with zero transformation. State is persisted locally for idempotent incremental runs.

## Architecture

Three Rust collector binaries share a common design:

| Component | birbir-watch | olx-watch | uzum-watch | idlewatch |
|-----------|-------------|-----------|------------|-----------|
| Source | BirBir.uz API | OLX.uz REST API | Uzum.uz GraphQL | X11 idle |
| Auth | Bearer JWT (ES512) | None (public) | Bearer JWT | N/A |
| Sync | Full + incremental poll | BFS category traversal | Leaf-category scan | N/A |
| Output | JSONL pass-through | JSONL pass-through | JSONL pass-through | Screen off + break |
| State | `~/.local/share/birbir/state.json` | `~/.local/share/olx/state.json` | `~/.local/share/uzum/state.json` | Config file |

### OLX.uz

REST API at `www.olx.uz/api/v1`, single `GET /offers` endpoint. No authentication — fully public reads. Pagination via offset/limit parameters with a `has_more` heuristic. Complete offer schema includes 30+ fields covering listing details, location, seller info, photos, delivery options, and safedeal config.

### BirBir.uz

GraphQL API at `api.birbir.uz` (version `1.3.5.0`). Authentication via Cloudflare JS challenge → session cookie → JWT bearer token (ES512, ECDSA P-521, ~4h expiry). Full offer schema with nested `region.titlePath`, `seller.*`, `promotion`, `badges`, and price breakdowns. Output is raw API JSON pass-through with zero transformation.

### Uzum.uz

GraphQL API at `graphql.uzum.uz`. Authentication via Bearer JWT token (stored in env var `UZUM_ACCESS_TOKEN`). Two-phase collection: full scan from scratch, or incremental `--refresh` for changed categories only. Known limits include a 10,000 offset cap (max safe offset 9900 with batch size 100) and sequential-only architecture (concurrent requests ≥2 trigger 429).

## Collector Patterns

All collectors follow a common Rust binary pattern: single `src/main.rs`, Cargo workspace member per project, state persisted in `~/.local/share/<name>/state.json`. Installation via `cargo build --release` with optional systemd timer for daemon mode.

## idlewatch

A standalone bash script that monitors X11 idle time via `xprintidle` and enforces break scheduling. The screen turns off after 2 minutes of inactivity; a 30-second break is enforced after 20 minutes of active work. See [idlewatch.md](tools/idlewatch.md) for full documentation.

## See Also

- [olx-uz](collectors/olx-uz.md)
- [birbir-uz](collectors/birbir-uz.md)
- [uzum-uz](collectors/uzum-uz.md)
- [idlewatch](tools/idlewatch.md)
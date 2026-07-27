# Project Overview — Classifieds Archive Collectors

> Sources: PURPOSE.md, 2026-07-27; AGENTS.md, 2026-07-27; olx.uz WIKI, 2026-07-27; olx.uz README, 2026-07-27; olx.uz AGENTS, 2026-07-27; olx.uz live API investigation, 2026-07-27; birbir.uz WIKI, 2026-07-27; birbir.uz README, 2026-07-27; birbir.uz AGENTS, 2026-07-27; birbir-findings, 2026-07-27; birbir.uz live API investigation, 2026-07-27; uzum.uz WIKI, 2026-07-27; uzum.uz AGENTS, 2026-07-27; uzum.uz API end-to-end analysis, 2026-07-25; uzum.uz live API investigation, 2026-07-25; uzum.uz live page snapshot, 2026-07-25; idlewatch, 2026-07-27
> Raw: [PURPOSE.md](../../PURPOSE.md); [AGENTS.md](../../AGENTS.md); [olx.uz_WIKI.md](../../raw/collectors/olx.uz_WIKI.md); [olx.uz_README.md](../../raw/collectors/olx.uz_README.md); [olx.uz_AGENTS.md](../../raw/collectors/olx.uz_AGENTS.md); [olx-live-api.md](../../olx.uz/raw/olx-api/2026-07-27-live-api-investigation.md); [birbir.uz_WIKI.md](../../raw/collectors/birbir.uz_WIKI.md); [birbir.uz_README.md](../../raw/collectors/birbir.uz_README.md); [birbir.uz_AGENTS.md](../../raw/collectors/birbir.uz_AGENTS.md); [birbir-findings.md](../../raw/collectors/birbir-findings.md); [birbir-live-api.md](../../birbir.uz/raw/birbir-api/2026-07-27-live-api-investigation.md); [uzum.uz_WIKI.md](../../raw/collectors/uzum.uz_WIKI.md); [uzum.uz_AGENTS.md](../../raw/collectors/uzum.uz_AGENTS.md); [uzum-e2e-analysis.md](../../uzum.uz/raw/uzum-api/2026-07-25-api-end-to-end-analysis.md); [uzum-live-api.md](../../uzum.uz/raw/uzum-api/2026-07-25-live-api-investigation.md); [uzum-page-snapshot.txt](../../uzum.uz/raw/uzum-api/2026-07-25-live-page-snapshot.txt); [idlewatch.md](../../raw/collectors/2026-07-27-idlewatch.md)
> Updated: 2026-07-27

## Purpose

Continuously archive classifieds and marketplace listings from BirBir.uz, OLX.uz, and Uzum.uz into lossless JSONL exports. The resulting dataset feeds downstream AI-driven trading analysis, market intelligence, and trend modeling.

Each collector is a lean Rust binary that performs an initial full catalog sync, then polls incrementally — writing raw API JSON pass-through with zero transformation. State is persisted locally for idempotent incremental runs.

## Sites

### OLX.uz

Uzbekistan's largest classifieds marketplace. Public API — no authentication required for reads.

- **API**: `GET https://www.olx.uz/api/v1/offers`
- **Auth**: None (public endpoint)
- **Pagination**: offset/limit with `has_more` heuristic (`offers.len() >= PAGE_SIZE`)
- **Output**: `~/.local/share/olx/olx_export.jsonl` — raw API JSON pass-through
- **State**: `~/.local/share/olx/state.json` (`max_id`, `initial_complete`, `known_categories`)
- **Collector**: `olx-watch` — single `src/main.rs`, two-phase (full sync → incremental poll)

Sources: olx.uz WIKI, 2026-07-27; olx.uz README, 2026-07-27; olx.uz AGENTS, 2026-07-27; olx.uz live API investigation, 2026-07-27
Raw: [olx.uz_WIKI.md](../../raw/collectors/olx.uz_WIKI.md); [olx.uz_README.md](../../raw/collectors/olx.uz_README.md); [olx.uz_AGENTS.md](../../raw/collectors/olx.uz_AGENTS.md); [olx-live-api.md](../../olx.uz/raw/olx-api/2026-07-27-live-api-investigation.md)

### BirBir.uz

Uzbekistan-based classifieds marketplace with a Cloudflare JS challenge gating JWT-based authentication.

- **API**: `POST https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/feed`
- **Auth**: Bearer JWT (ES512/ECDSA P-521), extracted from `session` cookie (URL-encoded JSON with `j:` prefix)
- **Pagination**: page-based via `nextPageExists` in paginator
- **Output**: `~/.local/share/birbir/birbir_export.jsonl` — raw API JSON pass-through
- **State**: `~/.local/share/birbir/state.json` (`max_id`, `initial_complete`)
- **Collector**: `birbir-watch` — single `src/main.rs`, two-phase (initial full → incremental poll)
- **Token refresh**: cached `token.txt` → direct `curl` to site → parse `Set-Cookie`

Sources: birbir.uz WIKI, 2026-07-27; birbir.uz README, 2026-07-27; birbir.uz AGENTS, 2026-07-27; birbir-findings, 2026-07-27; birbir.uz live API investigation, 2026-07-27
Raw: [birbir.uz_WIKI.md](../../raw/collectors/birbir.uz_WIKI.md); [birbir.uz_README.md](../../raw/collectors/birbir.uz_README.md); [birbir.uz_AGENTS.md](../../raw/collectors/birbir.uz_AGENTS.md); [birbir-findings.md](../../raw/collectors/birbir-findings.md); [birbir-live-api.md](../../birbir.uz/raw/birbir-api/2026-07-27-live-api-investigation.md)

### Uzum.uz

Uzbekistan's largest online supermarket. GraphQL-based API (Apollo) with REST endpoints for categories and user data.

- **GraphQL**: `POST https://graphql.uzum.uz/`
- **REST**: `GET https://api.uzum.uz/api/main/root-categories?eco=false`
- **Auth**: Bearer JWT + `x-iid` header from cookies (`UZUM_ACCESS_TOKEN`, `UZUM_INSTALL_ID`)
- **Pagination**: offset/limit with max safe offset of 9900 (100 batch size, hard cap at 10000)
- **Output**: `~/.local/share/uzum/uzum_data.jsonl` — JSON Lines with header + raw `catalogCard` objects
- **State**: `~/.local/share/uzum/state.json` (per-category progress, `item_count`, `updated_at`)
- **Collector**: `uzum-watch` — single `src/main.rs`, two modes (full scan + `--refresh`)
- **Category tree**: 23 top-level, 1627 leaf categories

Sources: uzum.uz WIKI, 2026-07-27; uzum.uz AGENTS, 2026-07-27; uzum.uz API end-to-end analysis, 2026-07-25; uzum.uz live API investigation, 2026-07-25; uzum.uz live page snapshot, 2026-07-25
Raw: [uzum.uz_WIKI.md](../../raw/collectors/uzum.uz_WIKI.md); [uzum.uz_AGENTS.md](../../raw/collectors/uzum.uz_AGENTS.md); [uzum-e2e-analysis.md](../../uzum.uz/raw/uzum-api/2026-07-25-api-end-to-end-analysis.md); [uzum-live-api.md](../../uzum.uz/raw/uzum-api/2026-07-25-live-api-investigation.md); [uzum-page-snapshot.txt](../../uzum.uz/raw/uzum-api/2026-07-25-live-page-snapshot.txt)

## Idle Monitor

`idlewatch` — a standalone bash script that monitors X11 idle time via `xprintidle` and enforces screen-off + break scheduling:

- **Screen off**: after 2 minutes of inactivity
- **Short break**: 30 seconds after 20 minutes of active work
- **Long break**: 15 minutes after 60 minutes of active work
- **Break exit**: `SIGUSR1` kills the break early
- **Lock**: `flock` on `$LOCK_FILE` for single-instance execution

Sources: idlewatch, 2026-07-27
Raw: [idlewatch.md](../../raw/collectors/2026-07-27-idlewatch.md)

## Common Patterns

All three collectors share a consistent architecture:

1. **Single-file Rust binary** (`src/main.rs`, no `lib.rs`)
2. **Two-phase operation**: full initial sync → incremental polling
3. **Raw JSON pass-through**: zero transformation, `serde_json::to_string()` write
4. **Atomic state writes**: `.tmp` + `rename`
5. **Exclusive locking**: `flock` on `.lock` file
6. **Systemd integration**: oneshot service + timer for continuous operation
7. **Environment configuration**: `POLL_INTERVAL` for daemon mode

## Infrastructure

| Component | OLX.uz | BirBir.uz | Uzum.uz |
|-----------|--------|-----------|---------|
| API domain | `www.olx.uz` | `api.birbir.uz` | `graphql.uzum.uz` / `api.uzum.uz` |
| Auth | None | JWT (Cloudflare-gated) | JWT + install ID |
| CDN | `frankfurt.apollo.olxcdn.com` | `img.birbir.uz` / `file.birbir.uz` | `images.uzum.uz` / `static.uzum.uz` |
| Protocol | REST | REST | GraphQL + REST |
| Data format | Flat offer object | Nested offer object | `catalogCard` wrapper |
| Output | JSONL (no header) | JSONL (no header) | JSONL (header + data lines) |

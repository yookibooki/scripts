# Marketplace Archivers

Archive classifieds from 3 Uzbek marketplaces into JSONL. Feeds AI trading analysis.

## Project

Three independent Rust crates, no workspace, no shared library:

```
scripts/
├── birbir.uz/  → birbir-watch (ureq + base64 + urlencoding)
├── olx.uz/     → olx-watch    (ureq only)
├── uzum.uz/    → uzum-watch   (ureq + chrono)
├── linux/idlewatch             # systemd idle timer
├── raw/collectors/             # investigation notes (immutable)
├── wiki/                       # this kb
├── PURPOSE.md                  # goal statement
└── AGENTS.md                   # dev rules
```

Each crate: `cargo run --release` from its directory. Output JSONL at `~/.local/share/{olx,birbir,uzum}/{olx_export,birbir_export,uzum_data}.jsonl`.

### Shared patterns

| Pattern | Detail |
|---------|--------|
| Lock | `{olx,birbir,uzum}.lock` in data dir, `flock()` exclusive — fail-fast if running |
| State | `state.json` in data dir, atomic-write through `.tmp` + rename |
| Retry | 3 attempts on HTTP failure, linear backoff 500ms × attempt |
| Phases | Phase 1: initial full scan. Phase 2: incremental poll (same binary, reads state) |
| Output | One JSON object per line (JSONL), no trailing comma, flushed after each page |

### Per-site differences

| | Uzum.uz | OLX.uz | BirBir.uz |
|---|---|---|---|
| Auth | Bearer JWT + `X-Iid` header from env `UZUM_ACCESS_TOKEN`, `UZUM_INSTALL_ID` | None — public GET | Access token in POST body, also decoded from JWT for expiry check |
| API base | `api.uzum.uz/api` + `graphql.uzum.uz/` | `www.olx.uz/api/v1/offers` | `api.birbir.uz/api/frontoffice/1.3.5.0` |
| Pagination | Offset/limit per leaf category, OFFSET_CAP = **9900** | Offset/limit, MAX_OFFSET = **1000**, PAGE_SIZE = **50** | Body offset/limit, MAX_PAGE = **10000**, PAGE_SIZE = **40** |
| Category traversal | REST `GET /api/main/root-categories?eco=false` → walk nested tree → collect leaf category IDs | None — flat pagination by newest ID | None — flat pagination |
| Scale | 1627 leaf categories, ~500K items, 10M+ catalog | Small — 1000 max offset, BFS over user-defined categories | Large — up to 10000 pages × 40 = 400K items |
| Infra | `images.uzum.uz`, `static.uzum.uz` CDN; Sentry `sentry.infra.cluster.daymarket.uz`; GrowthBook feature flags; own analytics domain | `frankfurt.apollo.olxcdn.com` CDN; `*.svc.baxter.olx.org` microservices; GTM; Google Publisher Tag | Cloudflare JS challenge; `img.birbir.uz`, `file.birbir.uz` CDN; Sentry `sentry.doska-tech.uz`; Amplitude; GTM `GTM-PQ62VBHJ` |

### Sources

| Site | Raw files |
|------|-----------|
| Uzum.uz | [agents research](../../raw/collectors/2026-07-27-uzum-agents.md); [wiki compilation](../../raw/collectors/2026-07-27-uzum-wiki.md); [network requests](../../raw/collectors/2026-07-27-uzum-network-requests.md); [live snapshot](../../raw/collectors/2026-07-27-uzum-live-snapshot.md) |
| OLX.uz | [readme](../../raw/collectors/2026-07-27-olx-readme.md); [agents research](../../raw/collectors/2026-07-27-olx-agents.md); [wiki compilation](../../raw/collectors/2026-07-27-olx-wiki.md) |
| BirBir.uz | [readme](../../raw/collectors/2026-07-27-birbir-readme.md); [agents research](../../raw/collectors/2026-07-27-birbir-agents.md); [findings](../../raw/collectors/2026-07-27-birbir-findings.md); [wiki compilation](../../raw/collectors/2026-07-27-birbir-wiki.md) |

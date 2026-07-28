# Wiki Log

## 2026-07-28 | Initial wiki setup
- Created index, overview, and three collector articles
- Grounded in raw/collectors/ research notes

## 2026-07-28 | ingest | BirBir.uz collector
- Disposition: New
- Raw: ../raw/collectors/2026-07-27-birbir-readme.md; ../raw/collectors/2026-07-27-birbir-agents.md; ../raw/collectors/2026-07-27-birbir-findings.md; ../raw/collectors/2026-07-27-birbir-wiki.md

## 2026-07-28 | ingest | OLX.uz collector
- Disposition: New
- Raw: ../raw/collectors/2026-07-27-olx-readme.md; ../raw/collectors/2026-07-27-olx-agents.md; ../raw/collectors/2026-07-27-olx-wiki.md

## 2026-07-28 | ingest | Uzum.uz collector
- Disposition: New
- Raw: ../raw/collectors/2026-07-27-uzum-agents.md; ../raw/collectors/2026-07-27-uzum-wiki.md; ../raw/collectors/2026-07-27-uzum-network-requests.md; ../raw/collectors/2026-07-27-uzum-live-snapshot.md

## 2026-07-28 | lint | 4 issues found, 1 auto-fixed

## 2026-07-28 | rewrite | Compacted 5 articles → 1 (marketplaces.md)
- Removed: overview.md, birbir-uz.md, olx-uz.md, uzum-uz.md, tools/ — all redundant with the single reference article
- Verified key facts live via Chrome DevTools MCP
- Corrected: BirBir DOES have Cloudflare JS challenge (raw said none); Uzum root-categories endpoint confirmed at api.uzum.uz
- Fixed: wiki claimed shared library — project is 3 independent crates, no workspace

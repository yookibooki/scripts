# Wiki Log

## [2026-07-27] ingest | BirBir.uz API Reference
- Disposition: New
- Raw: raw/birbir-api/2026-07-27-live-api-investigation.md
- Updated: birbir-api-reference

## [2026-07-27] ingest | BirBir.uz New Posts Watch (birbir-watch)
- Disposition: New
- Raw: raw/birbir-api/2026-07-27-live-api-investigation.md
- Updated: birbir-watch-collector

## [2026-07-27] live-test | BirBir.uz Live API Verification
- Disposition: Expanded existing wiki with live browser verification results
- Raw: local://birbir-findings.md
- Updated: birbir-api-reference (expanded significantly with JWT payload breakdown, error code details, full offer schema TypeScript interfaces, WebSocket details, brute-force endpoint discovery, category system analysis)
- Verified: All API endpoints tested via Chrome DevTools MCP fetch() calls with real JWT token

## [2026-07-27] update | BirBir.uz New Posts Watch (birbir-watch)
- Disposition: Update
- Updated: birbir-watch-collector
  - Fixed output format: documents raw API JSON pass-through (was incorrectly documented as flattened with flat keys)
  - Updated auth documentation to include JWT payload claims
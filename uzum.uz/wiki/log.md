# Wiki Log

## [2026-07-25] ingest | Uzum Marketplace Collector (uzum-watch)
- Disposition: New
- Raw: raw/uzum-api/2026-07-25-live-api-investigation.md
- Updated: uzum-watch-collector

## [2026-07-25] ingest | Uzum API Reference
- Disposition: New; Update
- Raw: raw/uzum-api/2026-07-25-api-end-to-end-analysis.md
- Updated: uzum-watch-collector; uzum-api-reference

## [2026-07-25] lint | 1 issue found, 0 auto-fixed

## [2026-07-26] update | Uzum Marketplace Collector (uzum-watch)
- Disposition: Update
- Raw: raw/uzum-api/2026-07-25-live-api-investigation.md
- Updated: uzum-watch-collector; uzum-api-reference
  - Architecture corrected: now sequential (removed 5-thread concurrency)
  - Fixed serde camelCase mismatch and `deliveryOptions` type (object not array)
  - Updated GraphQL query shown to match actual Rust code
  - Updated output format example with real field names
  - Added bugs fixed section, performance measurements, token expiry notes
  - API reference: added deliveryOptions type clarification

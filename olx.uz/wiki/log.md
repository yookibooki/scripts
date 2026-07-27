# Wiki Log

## [2026-07-27] ingest | OLX.uz API Reference
- Disposition: New
- Raw: raw/olx-api/2026-07-27-live-api-investigation.md
- Updated: olx-api-reference

## [2026-07-27] ingest | OLX.uz Market Data Collector (olx-watch)
- Disposition: New
- Raw: raw/olx-api/2026-07-27-live-api-investigation.md
- Updated: olx-watch-collector

## [2026-07-27] update | OLX.uz API Reference
- Disposition: Update
- Raw: local://olx-findings.md
- Updated: olx-api-reference; olx-watch-collector
  - Corrected pagination: no hard 1000-offset cap, ~65 items per page, no HAL links
  - Added authenticated endpoints (users/me, users/profile)
  - Added CloudFront CDN infrastructure details
  - Fixed output format: raw API JSON pass-through (was incorrectly documented as flattened)
  - Added complete TypeScript offer interfaces
  - Added error response shapes (400, 401)

## [2026-07-27] update | OLX.uz Market Data Collector (olx-watch)
- Disposition: Update
- Updated: olx-watch-collector
  - Fixed output format: documents raw API JSON pass-through
  - Updated pagination documentation to match actual API behavior

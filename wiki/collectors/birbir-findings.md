# BirBir Live API Investigation Notes

> Sources: local/birbir-findings.md, 2026-07-27
> Raw: [birbir-findings](../../raw/collectors/2026-07-27-birbir-findings.md); [birbir-wiki](../../raw/collectors/2026-07-27-birbir-wiki.md)
> Updated: 2026-07-27

## Key Findings

### Infrastructure

| Component | Value |
|-----------|-------|
| API domain | `api.birbir.uz` |
| API version | `1.3.5.0` |
| Base path | `/api/frontoffice/1.3.5.0` |
| Image CDN | `img.birbir.uz` |
| File CDN | `file.birbir.uz` |
| WebSocket | `socket.birbir.uz` (Centrifugo) |
| Security | Cloudflare JS challenge on main site |
| Gateway | istio-envoy |
| Backend | PHP/Akene-based |
| Runtime | React SPA (Webpack chunks) |

### Authentication

Session cookie is URL-encoded JSON prefixed with `j:`. JWT uses ES512 (ECDSA P-521) with ~4h expiry. Refresh requires re-authentication via browser (Cloudflare JS challenge).

### API Endpoints

- `POST /offer/feed` — main feed (paginated, 40 per page, sort options)
- `GET /offer/{id}` — single offer detail
- `GET /user` — current user data
- `GET /user/profile` — extended profile
- `POST /auth/enter-by-phone` — phone auth (requires UI flow)
- `GET /chat/dialog?perPage=40` — chat dialogs

### Category System

No public REST endpoint for categories. Categories are embedded in `webUri` paths and `titlePath` arrays within offer data.

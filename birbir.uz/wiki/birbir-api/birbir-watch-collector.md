# BirBir.uz Watch Collector

> Sources: BirBir.uz, 2026-07-27 (live API investigation); source code analysis
> Raw: [Live API Investigation](../../raw/birbir-api/2026-07-27-live-api-investigation.md)
> Updated: 2026-07-27

## Overview

A Rust CLI tool that monitors [BirBir.uz](https://birbir.uz) for newly created listings via their JSON API. Runs as a systemd user timer for periodic incremental collection. Outputs data to `~/.local/share/birbir/birbir_export.jsonl`.

Unique among the three scrapers, birbir-watch requires handling a Cloudflare JS challenge to obtain a session token, using direct HTTP with a browser-like User-Agent.

## Architecture

Two-phase collection:

### Phase 1: Initial Full Collection
Triggered when `initial_complete=false` in state. Runs exactly once:
1. Obtains auth token via direct HTTP
2. Iterates pages from page 1, collecting all offers
3. Stops when `nextPageExists=false` in the API response
4. Tracks max seen ID, sets `initial_complete=true`

### Phase 2: Incremental Poll
Runs on every subsequent execution:
1. Obtains/freshens auth token
2. Fetches pages from page 1
3. Exports offers where `id > max_id`
4. Stops when a full page contains only already-known IDs (IDs are monotonically increasing)
5. Appends to existing output file

## Authentication

Due to Cloudflare JS challenge, obtaining a token requires handling the challenge. The token is obtained via direct HTTP with a browser-like User-Agent:

### Token Sources (in priority order)

1. **Cached token** (`~/.local/share/birbir/token.txt`) — fastest path, checked first
2. **Direct HTTP fetch** — uses `curl` to request `https://birbir.uz/` and parse the `Set-Cookie: session=...` header from the response

### JWT Expiry Check

The token payload is base64-decoded locally to check the `exp` claim. Tokens expiring within 60s are considered stale and trigger re-fetching.

### Token Refresh

On HTTP 401 from any API call:
1. Cached token is invalidated (deleted)
2. New token is obtained via direct HTTP
3. The failed request is retried with the new token

## API Usage

| Detail | Value |
|--------|-------|
| Endpoint | `POST /api/frontoffice/1.3.5.0/offer/feed` |
| Base URL | `https://api.birbir.uz` |
| Auth | `Bearer <JWT>` |
| Page size | 40 |
| Region | `"all"` |
| Sort | `2` |
| Method | POST with JSON body |
| Retries | 3 attempts with 500ms backoff |

### Request

```json
POST /api/frontoffice/1.3.5.0/offer/feed
Authorization: Bearer eyJ...
Content-Type: application/json

{
  "page": 1,
  "perPage": 40,
  "region": "all",
  "sort": 2
}
```

### Success Response

```json
{
  "content": {
    "items": [...],
    "paginator": {
      "step": 40,
      "current": 1,
      "nextPageExists": true
    }
  }
}
```

### Error Response (token expired)

```json
{
  "content": null,
  "error": {
    "code": "ACCESS_TOKEN_INVALID",
    "message": "Access token invalid.",
    "alert": null
  }
}
```

## Output Format

`~/.local/share/birbir/birbir_export.jsonl` — JSON Lines, one raw API offer object per line.

Each line is the **complete raw offer JSON** as returned by `POST /offer/feed`. The tool performs zero transformation — it writes the `content.items[]` array items directly via `serde_json::to_string(offer).unwrap()`.

Example output line:

```json
{
  "id": 272116974,
  "title": "Iphon 14 pro",
  "price": 500000000,
  "publishedAt": 1784694169564,
  "webUri": "uz/toshkent/cat/telefonlar/smartfonlar/o/iphon-14-pro-272116974",
  "urgentSale": false,
  "courierDelivery": false,
  "business": false,
  "agency": false,
  "closed": false,
  "region": {
    "titlePath": ["Telefonlar", "Smartfonlar"],
    "location": { "coordinates": [69.2401, 41.3328] }
  },
  "seller": {
    "uuid": "abc123...",
    "name": "Seller Name",
    "verified": false,
    "business": false,
    "agency": false,
    "offerActiveCount": null
  }
}
```

### Important Notes

- The output is **not flattened**. Each line contains the full nested API response as-is.
- Category path is in `region.titlePath` as an array of hierarchy segments.
- Seller information is in the nested `seller` object (not `seller_*` prefixed flat keys).
- Coordinates are in `region.location.coordinates` as `[lon, lat]`.
- Price is a raw number, typically in UZS (sometimes USD).
- The `webUri` is a relative path; prepend `https://birbir.uz/` for the full URL.
- Some fields may be null/absent depending on offer type.
- The schema is documented in full in [birbir-api-reference.md](birbir-api-reference.md).

## State Schema

`~/.local/share/birbir/state.json`:

```json
{
  "version": 1,
  "max_id": 272116974,
  "initial_complete": true
}
```

## Operational Details

- **Data directory**: `~/.local/share/birbir/`
- **Lock file**: `birbir.lock` (flock-based exclusive lock)
- **State**: `state.json` (atomic writes via `.tmp` + rename)
- **Token cache**: `token.txt` (also atomic write)
- **Output**: `birbir_export.jsonl` (append-only)
- **Modes**: oneshot (default) or daemon via `POLL_INTERVAL` env var
- **Delay**: 100ms between pages

## Configuration

| Env Variable | Description | Default |
|-------------|-------------|---------|
| `POLL_INTERVAL` | Daemon poll interval in ms | 0 (oneshot) |

## Known Behaviors

- Token expires approximately every 4 hours — direct HTTP fetch needed for re-auth
- The feed sorts by newest-first (sort=2) — not configurable via env
- No category tree endpoint — all offers fetched from a single feed

## Installation

```bash
cargo build --release
cp target/release/birbir-watch ~/.local/bin/

# systemd timer (every 30 min)
cat > ~/.config/systemd/user/birbir-watch.service << 'EOF'
[Unit]
Description=BirBir.uz new posts watch

[Service]
Type=oneshot
ExecStart=%h/.local/bin/birbir-watch
EOF

cat > ~/.config/systemd/user/birbir-watch.timer << 'EOF'
[Unit]
Description=BirBir.uz poll timer (every 30 min)

[Timer]
OnCalendar=*:0/30
Persistent=true

[Install]
WantedBy=timers.target
EOF

systemctl --user daemon-reload
systemctl --user enable --now birbir-watch.timer
```

## Dependencies

- **curl** — used for direct HTTP token fetch (system-provided)
- **No external browser or browser automation tool required**

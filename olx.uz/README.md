# OLX.uz Market Data Collector

Collects all listings from [OLX.uz](https://www.olx.uz) into a machine-readable archive — full history, ongoing updates, no deletions.

## Output format

`~/.local/share/olx/olx_export.jsonl` — JSON Lines, one raw API offer object per line.

Each line is the **complete raw offer JSON** as returned by `GET /api/v1/offers`. The tool performs zero transformation — it writes the API response data items directly.

Example output (abbreviated):

```json
{
  "id": 62539007,
  "url": "https://www.olx.uz/d/obyavlenie/...",
  "title": "Полировка керамика авто",
  "description": "Профессиональная полировка...",
  "last_refresh_time": "2026-07-21T00:12:04+05:00",
  "created_time": "2026-07-20T15:30:00+05:00",
  "params": [{"key": "price", "value": {...}}],
  "category": { "id": 317, "type": "automotive" },
  "location": { "city": {...}, "region": {...} },
  "map": { "lat": 41.3, "lon": 69.2 },
  "user": { "id": 123, "name": "..." },
  "photos": [],
  "business": false,
  "status": "active"
}
```

**Important**: The output is the raw API response — it is NOT flattened. Price is inside `params[]`, category is a nested object, location is nested. See `WIKI.md` for the full schema.

## Quick start

```bash
# Build
cd ~/workspace/scripts/olx.uz
cargo build --release
cp target/release/olx-watch ~/.local/bin/
```

### Run via systemd (recommended)

```bash
# Start and enable the timer
systemctl --user enable --now olx-watch.timer

# Check status
systemctl --user status olx-watch.timer

# View latest poll results
journalctl --user -u olx-watch.service -f
```

### Run ad-hoc (one poll cycle)

```bash
./target/release/olx-watch
```

### Run as a daemon (continuous loop)

```bash
POLL_INTERVAL=60000 ./target/release/olx-watch
```

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Unified binary — two-phase collection (single file, no lib.rs) |
| `Cargo.toml` | Rust package manifest |
| `~/.local/share/olx/olx_export.jsonl` | Output — all collected listings (JSON Lines) |
| `~/.local/share/olx/state.json` | State — max_id, initial_complete, known_categories |
| `~/.config/systemd/user/olx-watch.service` | systemd oneshot service unit |
| `~/.config/systemd/user/olx-watch.timer` | systemd timer (every 30 min) |

## Configuration

Via environment variables:

- `POLL_INTERVAL` — polling interval in ms for daemon mode (default: unset = oneshot mode)

## Error logging

Errors are printed to stderr with `[ERROR]` and `[WARN]` prefixes:
- HTTP failures (network, timeouts)
- JSON parse errors
- Missing fields

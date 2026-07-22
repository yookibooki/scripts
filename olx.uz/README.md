# OLX.uz Market Data Collector

Collects all listings from [OLX.uz](https://www.olx.uz) into a machine-readable archive — fully history, ongoing updates, no deletions.

## Stack

- **Rust** — single compiled binary, ~3 MB RSS at runtime
- **ureq** — lightweight HTTP client
- **serde** / **serde_json** — JSON serialization

## How it works

The system has two phases:

### Phase 1 — Initial full dump (automatic, one-time)

On first run, it discovers all categories via BFS and paginates through every single one (bypassing the API's 1000-offset-per-query cap by scoping requests to individual `category_id`). This collects every currently active listing.

### Phase 2 — Ongoing poll (every 30 minutes)

Each subsequent run polls the default listing (newest-first), identifies new posts by tracking the highest seen ID, and appends them to the same file. Nothing is ever deleted.

## Output format

`~/.local/share/olx/olx_export.jsonl` — JSON Lines, one post per line:

```json
{
  "id": 62539007,
  "url": "https://www.olx.uz/d/obyavlenie/...",
  "title": "Полировка керамика авто 800 000 сумдан бошлаб",
  "description": "Профессиональная полировка и керамическое покрытие...",
  "price": "800000",
  "category_type": "automotive",
  "city": "Ташкент",
  "district": "Юнусабадский район",
  "region": "Ташкентская область",
  "last_refresh_time": "2026-07-21T00:12:04+05:00"
}
```

| Field | Description |
|---|---|
| `id` | Unique OLX listing ID |
| `url` | Direct link to the listing |
| `title` | Listing title |
| `description` | Full description (HTML stripped) |
| `price` | Price as displayed on OLX |
| `category_type` | Category group (e.g. `electronics`, `automotive`) |
| `city` | City name |
| `district` | District name |
| `region` | Region name |
| `last_refresh_time` | ISO timestamp of last refresh/bump |

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
|---|---|
| `src/main.rs` | Unified binary — two-phase collection |
| `src/lib.rs` | Shared utilities (HTTP, parsing, helpers) |
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

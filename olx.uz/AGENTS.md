- Rust toolchain required; build with `cargo build --release`
- Library `src/lib.rs` -- shared utilities (`fetch_json`, `extract_id`, `data_dir`, `ApiResponse`)
- Single binary `olx-watch` in `src/main.rs` (deps: ureq, serde, serde_json)

## Two-phase collection

### Phase 1 -- Initial full dump (one-time)
- Runs automatically on first execution (no `state.json` found)
- BFS category discovery: starts with the default listing, then paginates every discovered category
- Bypasses the API's 1000-offset-per-query cap by scoping each pagination to a single `category_id`
- Writes every post to `~/.local/share/olx/olx_export.jsonl`
- Saves `state.json` with `max_id`, `initial_complete: true`, and `known_categories`

### Phase 2 -- Ongoing poll (every timer tick)
- Polls the default listing (newest-first)
- Processes posts with `id > max_id` as new; skips already-known posts
- Stops paginating when a full page contains only known posts (caught up)
- Appends new posts to the same `olx_export.jsonl` file -- nothing is ever deleted

### Daemon mode
- Set `POLL_INTERVAL` (ms) to run as a continuous loop instead of a one-shot
- Example: `POLL_INTERVAL=60000 ./olx-watch` polls every 60 seconds

## Output format
- File: `~/.local/share/olx/olx_export.jsonl` (JSON Lines, one object per line)
- Fields kept: `id`, `url`, `title`, `business`, `created_time`, `last_refresh_time`, `price_uzs`, `category_type`, `location_city`, `location_district`, `location_region`, `coordinates`

## State
- File: `~/.local/share/olx/state.json`
- Tracks: `max_id` (highest seen ID), `initial_complete` (whether Phase 1 is done), `known_categories` (list of discovered category IDs)

## Data dir
- `~/.local/share/olx/`

## Error logging
- `[ERROR]` and `[WARN]` prefixed messages on stderr

## Systemd
- Service: `~/.config/systemd/user/olx-watch.service` -- `Type=oneshot`, triggered by timer
- Timer: `~/.config/systemd/user/olx-watch.timer` -- `OnCalendar=*:0/30` (every 30 min)
- Manage: `systemctl --user {start,stop,restart,status} olx-watch.service` or `olx-watch.timer`
- Logs: `journalctl --user -u olx-watch.service -f`

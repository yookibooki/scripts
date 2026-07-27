# BirBir.uz New Posts Watch

Rust monitor that polls [BirBir.uz](https://birbir.uz) for newly created listings via their JSON API.

## Quick start

```bash
# Build
cd ~/workspace/scripts/birbir.uz
cargo build --release
cp target/release/birbir-watch ~/.local/bin/
```

### Run via systemd (recommended)

```bash
# Create the user service and timer
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

# Start and enable
systemctl --user daemon-reload
systemctl --user enable --now birbir-watch.timer

# Check status
systemctl --user status birbir-watch.timer

# Follow logs
journalctl --user -u birbir-watch.service -f
```

### Run ad-hoc

```bash
./target/release/birbir-watch

# Or in daemon mode (poll every 60 seconds)
POLL_INTERVAL=60000 ./target/release/birbir-watch
```

State saves automatically after each poll cycle.

## Output format

`~/.local/share/birbir/birbir_export.jsonl` — JSON Lines, one raw API offer object per line.

Each line is the **complete raw offer JSON** as returned by `POST /offer/feed`. The tool performs zero transformation — it writes the API response items directly via `serde_json::to_string()` pass-through.

Example output (abbreviated):

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
    "uuid": "...",
    "name": "..."
  }
}
```

**Important**: The output is the raw API response — it is NOT flattened. Category path is `region.titlePath[]`, seller info is in the nested `seller` object, coordinates are in `region.location.coordinates`. Prepend `https://birbir.uz/` to `webUri` for the full URL. See `WIKI.md` for the full schema.

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Rust monitor — polls listings, auth, API client (single file, no lib.rs) |
| `Cargo.toml` | Rust package manifest |
| `~/.local/share/birbir/state.json` | Persisted max seen ID |
| `~/.local/share/birbir/birbir_export.jsonl` | Output — JSON Lines |
| `~/.config/systemd/user/birbir-watch.service` | systemd user service unit |
| `~/.config/systemd/user/birbir-watch.timer` | systemd timer (every 30 min) |

## Configuration

Via environment variables:

- `POLL_INTERVAL` — polling interval in ms (default: `0` = one-shot)
  Set to run in daemon mode (loop with sleep between cycles)

### Error logging

Errors are printed to stderr with `[ERROR]` and `[WARN]` prefixes:
- Failed auth token extraction
- HTTP failures (network, timeouts)
- JSON parse errors
- Missing fields

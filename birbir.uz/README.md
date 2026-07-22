# BirBir.uz New Posts Watch

Rust monitor that polls [BirBir.uz](https://birbir.uz) for newly created listings via their JSON API.

## Stack

- **Rust** — compiled binary, ~3 MB RSS at runtime
- **ureq** — lightweight HTTP client

## How it works

1. Obtains a session token from `birbir.uz` (Bearer token extracted from the session cookie)
2. Paginates through all current listings via `POST /api/frontoffice/1.3.5.0/offer/feed`
3. Detects new posts by tracking seen IDs in `~/.local/share/birbir/state.json`
4. Appends each new post to `~/.local/share/birbir/birbir_export.jsonl`

## Output format

`~/.local/share/birbir/birbir_export.jsonl` — JSON Lines, one offer per line:

```json
{
  "id": 272116974,
  "url": "https://birbir.uz/uz/toshkent/cat/telefonlar/smartfonlar/o/iphon-14-pro-272116974",
  "title": "Iphon 14 pro",
  "price": 500000000,
  "currency": "UZS",
  "city": "Toshkent",
  "published_at": 1784694169564,
  "category_path": "telefonlar/smartfonlar"
}
```

Fields:
- **id** — unique offer ID
- **url** — full permalink to the listing
- **title** — listing title
- **price** — numeric price value
- **currency** — currency code (UZS or USD)
- **city** — location from the listing
- **published_at** — epoch timestamp (ms)
- **category_path** — category hierarchy (e.g. "telefonlar/smartfonlar")

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

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Rust monitor — polls listings |
| `src/lib.rs` | Shared library — API client, auth, helpers |
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

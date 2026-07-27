# OLX.uz Market Data Collector

> Source: olx.uz/README.md
> Collected: 2026-07-27
> Published: Unknown

Collects all listings from OLX.uz into a machine-readable archive — full history, ongoing updates, no deletions.

## Output format

`~/.local/share/olx/olx_export.jsonl` — JSON Lines, one raw API offer object per line.

Each line is the complete raw offer JSON as returned by `GET /api/v1/offers`. The tool performs zero transformation — it writes the API response data items directly.

## Quick start

```bash
cd ~/workspace/scripts/olx.uz
cargo build --release
cp target/release/olx-watch ~/.local/bin/
```

### Run via systemd (recommended)

```bash
systemctl --user enable --now olx-watch.timer
systemctl --user status olx-watch.timer
journalctl --user -u olx-watch.service -f
```

### Run ad-hoc

```bash
./target/release/olx-watch
```

### Run as daemon

```bash
POLL_INTERVAL=60000 ./target/release/olx-watch
```

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Unified binary |
| `Cargo.toml` | Rust package manifest |
| `~/.local/share/olx/olx_export.jsonl` | Output |
| `~/.local/share/olx/state.json` | State |
| `~/.config/systemd/user/olx-watch.service` | systemd unit |
| `~/.config/systemd/user/olx-watch.timer` | systemd timer |

## Configuration

Via environment variables: `POLL_INTERVAL`
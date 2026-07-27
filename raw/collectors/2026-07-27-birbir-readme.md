# BirBir.uz New Posts Watch

> Source: birbir.uz/README.md
> Collected: 2026-07-27
> Published: Unknown

Rust monitor that polls BirBir.uz for newly created listings via their JSON API.

## Quick start

```bash
cd ~/workspace/scripts/birbir.uz
cargo build --release
cp target/release/birbir-watch ~/.local/bin/
```

### Run via systemd

```bash
systemctl --user enable --now birbir-watch.timer
journalctl --user -u birbir-watch.service -f
```

### Run ad-hoc

```bash
./target/release/birbir-watch
POLL_INTERVAL=60000 ./target/release/birbir-watch
```

## Output format

`~/.local/share/birbir/birbir_export.jsonl` — JSON Lines, one raw API offer object per line.
Each line is the complete raw offer object as returned by `POST /offer/feed`. Zero transformation.

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Rust monitor |
| `Cargo.toml` | Rust package manifest |
| `~/.local/share/birbir/state.json` | Persisted max seen ID |
| `~/.local/share/birbir/birbir_export.jsonl` | Output |
| `~/.config/systemd/user/birbir-watch.service` | systemd service |
| `~/.config/systemd/user/birbir-watch.timer` | systemd timer |

# OLX.uz New Posts Watch

Rust monitor that polls [OLX.uz](https://www.olx.uz) for newly created listings via their JSON API.

## Stack

- **Rust** — compiled binary, ~3 MB RSS at runtime
- **ureq** — lightweight HTTP client

## How it works

1. Polls `/api/v1/offers/` every ~15s for the latest listings
2. Detects new posts by tracking seen IDs in `state.json`
3. Appends each post to `olx_posts.txt` in plain text

## Output format

`olx_posts.txt` — plain text, one post per block, separated by blank lines:

```
Title
Price
Description

Next Title
...
```

Fields:
- **Title** — as-is from OLX
- **Price** — numeric value
- **Description** — full description, HTML stripped

## Quick start

```bash
# Build
cd ~/workspace/scripts/olx.uz
cargo build --release

# Run (background)
./target/release/olx-watch >> monitor.log 2>&1 &

# Watch logs (stderr)
tail -f monitor.log
```

Stop with `pkill olx-watch`. State saves automatically.

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Rust monitor — polls listings |
| `Cargo.toml` | Rust package manifest |
| `state.json` | Persisted set of seen ad IDs |
| `olx_posts.txt` | Output — plain text posts |

## Configuration

Via environment variables:

- `POLL_INTERVAL` — polling interval in ms (default: `15000`)
  Can also be baked at compile time: `POLL_INTERVAL=30000 cargo build --release`

### Adaptive polling

The monitor automatically adjusts the poll interval:
- After **3 consecutive empty rounds**, the interval doubles (up to 5 min max)
- Resets to the configured interval as soon as new posts appear

### Error logging

Errors are printed to stderr with `[ERROR]` and `[WARN]` prefixes:
- HTTP failures (network, timeouts)
- JSON parse errors
- Missing fields

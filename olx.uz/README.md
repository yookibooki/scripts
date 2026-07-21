# OLX.uz New Posts Monitor

Rust monitor that polls [OLX.uz](https://www.olx.uz) for newly created listings via their JSON API. No browser required for listing data — lightweight and efficient. Outputs plain text format for token-efficient LLM consumption.

## Stack

- **Rust** — compiled binary, ~5 MB RSS at runtime
- **ureq** — lightweight HTTP client (only Rust dependency)
- **No browser needed** for listing polling — direct HTTP calls

## How it works

1. Polls `/api/v1/offers/` every ~15s for the latest listings
2. Detects new posts by tracking seen IDs in `state.json`
3. Tries to fetch the phone number for each new post via `/api/v1/offers/{id}/limited-phones/`
4. Appends each post to `olx_posts.txt` in plain text

## Output format

`olx_posts.txt` — plain text, one post per block, separated by blank lines:

```
Title
Price
Phone
Description

Next Title
Price
Phone
Description
```

Example:

```
Ёгли Кунжара (жмых) пахта чигитиники, аралашмаларсиз.
6000
+99 897 3957557
Ёгли Кунжара пахта чигитиники, хеч кандай кушимчаларсиз. Копланган, 1 коп - 35 кг.

Аэрогриль Aerogril SAF1308TPBK HOFMANN
838180
-
Nomi: 3 ta 1 ta chuqur qovurgich (Gril, Pech, chuqur qovurgich)...
```

Fields:
- **Title** — as-is from OLX
- **Price** — numeric value, or `-` if not available
- **Phone** — contact number, or `-` if unavailable (phone API may be blocked for plain HTTP)
- **Description** — full description, HTML stripped

## Quick start

```bash
# 1. Build
cd ~/workspace/scripts/olx.uz
cargo build --release

# 2. Run (background)
./target/release/olx-monitor &

# 3. Watch logs (stderr)
tail -f monitor.log
```

Stop with `pkill olx-monitor`. State saves automatically.

## Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Main source code |
| `Cargo.toml` | Rust package manifest |
| `state.json` | Persisted set of seen ad IDs |
| `olx_posts.txt` | Output — plain text posts |

## Configuration

Via environment variables:

- `POLL_INTERVAL` — polling interval in ms (default: `15000`)
  Can also be baked at compile time: `POLL_INTERVAL=30000 cargo build --release`

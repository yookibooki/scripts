# Uzum.uz Market Data Collector

Collects all product listings from [Uzum.uz](https://uzum.uz) into a machine-readable archive — full history, ongoing updates, no deletions. Designed for 24x7 unattended operation with crash safety and health monitoring.

## Stack

- **Rust** — single compiled binary, ~3 MB RSS at runtime
- **ureq** — lightweight HTTP client
- **serde** / **serde_json** — JSON serialization
- **fs2** — file locking (single-instance enforcement)
- **signal-hook** — graceful shutdown (SIGTERM/SIGINT)
- **GraphQL** — product listings via `https://graphql.uzum.uz/`
- **REST** — category tree via `https://api.uzum.uz/api`

## Authentication

Uzum's APIs require a valid JWT bearer token. Extract it from your browser session:

1. Log in to [uzum.uz](https://uzum.uz) in your browser
2. Open DevTools → Application → Cookies → `https://uzum.uz`
3. Copy the `token` cookie value (or grab the `Authorization` header from an API request)
4. Set it as the `UZUM_TOKEN` environment variable:

```bash
export UZUM_TOKEN="Bearer eyJraWQiOi..."
```

The token expires after ~10 hours; re-extract and re-export when needed.

## How it works

The system has two phases:

### Phase 1 — Initial full dump (automatic, one-time)

On first run, it fetches the full category tree from the REST API (`/api/main/root-categories`), then paginates every leaf category via GraphQL (`MakeSearch_ItemsAndFilters`). This collects every currently active product.

### Phase 2 — Ongoing poll (every 30 minutes)

Each subsequent run polls all known categories with newest-first sort, identifies new products by tracking the highest seen ID, and appends them to the same file. Nothing is ever deleted.

## Output format

`~/.local/share/uzum/uzum_export.jsonl` — JSON Lines, one product per line:

```json
{
  "id": 123456,
  "url": "https://uzum.uz/product/123456",
  "title": "Product name",
  "price": "150000",
  "full_price": "200000",
  "category_id": 10020,
  "rating": 4.5,
  "feedback_quantity": 42,
  "image_url": "https://images.uzum.uz/.../product_540x540.jpg",
  "delivery_date": "1-2 kun",
  "stock_type": "IN_STOCK"
}
```

| Field | Description |
|---|---|
| `id` | Unique Uzum product ID |
| `url` | Direct link to the product page |
| `title` | Product name |
| `price` | Current selling price in UZS |
| `full_price` | Original price (before discounts) |
| `category_id` | Numeric category ID |
| `rating` | Product rating (0–5) |
| `feedback_quantity` | Number of reviews |
| `image_url` | High-res image URL (540x540) |
| `delivery_date` | Estimated delivery time |
| `stock_type` | Stock status (e.g. `IN_STOCK`) |

## Quick start

```bash
# Build
cd ~/workspace/scripts/uzum.uz
cargo build --release
cp target/release/uzum-watch ~/.local/bin/

# Set your JWT token
export UZUM_TOKEN="Bearer eyJraWQiOi..."

# Run once (Phase 1: full collection)
./target/release/uzum-watch

# Run as a daemon (continuous polling every 60s)
POLL_INTERVAL=60000 ./target/release/uzum-watch
```

### Run via systemd (recommended)

```bash
# Copy service files
cp systemd/uzum-watch.service ~/.config/systemd/user/
cp systemd/uzum-watch.timer ~/.config/systemd/user/

# Create environment file
mkdir -p /etc/uzum-watch
cp systemd/env.example /etc/uzum-watch/env
# Edit /etc/uzum-watch/env with your token

# Start and enable the timer
systemctl --user daemon-reload
systemctl --user enable --now uzum-watch.timer

# Check status
systemctl --user status uzum-watch.timer
systemctl --user status uzum-watch.service

# View logs
journalctl --user -u uzum-watch.service -f

# Check health
cat ~/.local/share/uzum/health.json
```

### Run ad-hoc (one poll cycle)

```bash
./target/release/uzum-watch
```

## Files

| File | Purpose |
|---|---|
| `src/main.rs` | Unified binary — two-phase collection with resilience |
| `src/lib.rs` | Shared utilities (HTTP, GraphQL, parsing, lock, health, logging) |
| `Cargo.toml` | Rust package manifest |
| `systemd/` | Systemd service/timer for 24x7 operation |
| `~/.local/share/uzum/uzum_export.jsonl` | Output — all collected products (JSON Lines) |
| `~/.local/share/uzum/state.json` | State — max_id, initial_complete, known_categories |
| `~/.local/share/uzum/uzum.lock` | Lock file — prevents concurrent instances |
| `~/.local/share/uzum/health.json` | Health status — last poll timestamp, product count |
| `~/.local/share/uzum/uzum.log` | Log file — rotated at 10 MB |

## Configuration

Via environment variables:

- `UZUM_TOKEN` — JWT bearer token (required)
- `POLL_INTERVAL` — polling interval in ms for daemon mode (default: unset = oneshot mode)
- `MAX_LOG_SIZE` — max log file size before rotation in bytes (default: 10 MB)

## 24x7 Resilience

The system includes several features for unattended operation:

- **Single-instance lock** — flock-based lock file prevents concurrent scrapers
- **Graceful shutdown** — handles SIGTERM/SIGINT for clean systemd stops
- **Atomic state writes** — state.json uses write-to-tmp + rename for crash safety
- **Health reporting** — `health.json` written after each poll with timestamp, status, product count
- **Log rotation** — automatic rotation when log exceeds 10 MB
- **Token expiry detection** — logs warning on 401/403 responses

## Logging

Logs are written to both stderr and `~/.local/share/uzum/uzum.log`:

- **Rotation** — automatic when log exceeds 10 MB
- **Timestamped** — rotated logs named `uzum-{timestamp}.log`
- **Structured** — `[INFO]`, `[WARN]`, `[ERROR]` prefixes

Errors include:
- HTTP failures (network, timeouts)
- JSON parse errors
- Missing fields
- Rate limiting (429) with automatic retry and exponential backoff
- Token expiry (401/403) with re-authentication reminder

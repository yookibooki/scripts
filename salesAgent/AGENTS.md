## Data Sources — be aware when reading/processing

### OLX — `~/.local/share/olx/olx_export.jsonl`

* **Schema shift at line 255,614:**
* **Legacy (1–255,613):** Flat fields (`category_type`, `city`, `district`, `region`, `price`, `description`, etc.)
* **Current (255,614+):** Variable schema (`business`, `created_time`, `price_uzs`) — location as nested `location` + `map {lat, lon}` (first ~12 records) or split `location_*` fields + `coordinates [lon, lat]` (rest)

### Birbir — `~/.local/share/birbir/birbir_export.jsonl`

* **Schema shift at line 10,005:**
* **Legacy (1–10,004):** Compact fields (`category_path`, `city`, `currency`, `published_at`, etc.)
* **Current (10,005+):** Expanded seller metadata (`seller_*`, `publishedAt`, `urgentSale`, `courierDelivery`, `webUri`)

Over time, the legacy data will become obsolete since the system updates every 30 minutes with new, growing data in the updated format.

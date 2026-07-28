## Critical — data correctness
- [x] `birbir.uz` — API returns prices in sub-units (100× displayed value). Collector stores verbatim. All BirBir prices in JSONL are 100× the real price. Fix: normalize ÷100 at collector write time or at query read time. Existing data needs re-processing.

## Design gaps — feed API vs detail API
- [x] `uzum.uz` — JSONL missing: description, image URLs, category path, seller info, SKU variants, reviews, product specs. The collector uses the catalog feed API (card-level), not the product detail endpoint. Adding these would require N×1 extra API calls per listing.
- [x] `olx.uz` — JSONL missing: image URLs, seller info, category path (only has `category_type`). Description is present. Same feed-API limitation.
- [x] `birbir.uz` — JSONL missing: description, image URLs, seller info, product specs. Category path and location are present. Same feed-API limitation.

## Observations
- Uzum has SKU-level pricing (different sizes/colors = different prices). JSONL captures only `minSellPrice` / `minFullPrice` of the first/default SKU, not per-variant.
- OLX has `description` field populated; Uzum and BirBir don't.
- None of the collectors store image URLs, which limits vision-based analysis.

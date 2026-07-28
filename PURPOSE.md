Archive classifieds and marketplace listings from BirBir.uz, OLX.uz, and Uzum.uz into JSONL exports. The dataset feeds downstream AI-driven trading analysis.

Each collector is a Rust binary that performs an initial full catalog sync, then polls incrementally. State is persisted locally for idempotent incremental runs.

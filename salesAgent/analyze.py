#!/usr/bin/env python3
"""
Reselling Automation — Incremental Analysis Engine

Reads only fresh (current-schema) records from OLX and Birbir exports,
identifies profitable reselling opportunities, and outputs ranked results.

The exporters run every 30 min via systemd timers. This script processes
only the records that have been added since the last run.

Usage:
    python3 analyze.py                  # Incremental analysis
    python3 analyze.py --full           # Full re-analysis from scratch
    python3 analyze.py --top=N          # Show top N opportunities
"""

import json
import os
import re
import sys
import time
from collections import defaultdict
from datetime import datetime, timezone

# ── Paths ──────────────────────────────────────────────────────────────────
OLX_EXPORT = os.path.expanduser("~/.local/share/olx/olx_export.jsonl")
BIRBIR_EXPORT = os.path.expanduser("~/.local/share/birbir/birbir_export.jsonl")
OUTPUT_FILE = os.path.join(os.path.dirname(__file__), "opportunities.json")
STATE_FILE = os.path.join(os.path.dirname(__file__), "analysis_state.json")

USD_TO_UZS = 12700

RESELLABLE_CATEGORIES = {"electronics", "goods", "automotive"}

OLX_TO_BIRBIR_CAT = {
    "electronics": {"elektronika", "audio-va-video", "kompyuter-texnikasi",
                    "fotosurat-uskunalari", "telefonlar", "noutbuklar",
                    "planshetlar-va-elektron-kitoblar", "oyinlar-pristavkalar-va-dasturlar"},
    "goods": {"mahsulotlar", "kiyim-va-poyabzallar", "mebel-va-interyer",
              "soatlar-va-zargarlik-buyumlari", "sumkalar-ryukzaklar-va-chamadonlar",
              "idish-va-oshxona-buyumlari", "gozallik-va-salomatlik",
              "xobbi-va-sport", "bolalar-uchun"},
    "automotive": {"transport", "avto-aksesuarlar", "qurilish-va-tamirlash"},
}


# ── Helpers ────────────────────────────────────────────────────────────────

CYR_TO_LAT = {
    'а': 'a', 'б': 'b', 'в': 'v', 'г': 'g', 'д': 'd', 'е': 'e', 'ё': 'e',
    'ж': 'zh', 'з': 'z', 'и': 'i', 'й': 'y', 'к': 'k', 'л': 'l', 'м': 'm',
    'н': 'n', 'о': 'o', 'п': 'p', 'р': 'r', 'с': 's', 'т': 't', 'у': 'u',
    'ф': 'f', 'х': 'kh', 'ц': 'ts', 'ч': 'ch', 'ш': 'sh', 'щ': 'shch',
    'ъ': '', 'ы': 'y', 'ь': '', 'э': 'e', 'ю': 'yu', 'я': 'ya',
    'ў': 'o\'', 'қ': 'q', 'ғ': 'g\'', 'ҳ': 'h',
}

STOPWORDS = {
    'va', 'uchun', 'bilan', 'kerak', 'yangi', 'sotiladi', 'sotib', 'olinadi',
    'top', 'cрочно', 'srochno', 'arzon', 'ishlatilmagan', 'ideal', 'хороший',
    'и', 'в', 'на', 'с', 'по', 'для', 'из', 'от', 'новый', 'продам', 'купить',
    'не', 'orqali', 'agar', 'qilish', 'beriladi', 'new', 'for', 'the', 'and',
    'with', 'from', 'free', 'best', 'original', 'originalno', 'акция',
    'куплю', 'продаю', 'продается', 'ariyor', 'sotuv', 'narx', 'dona',
    'km', 'sm', 'ml', 'kg', 'gr', 'mm', '000', '0000',
}


def normalize_title(title: str) -> str:
    if not title:
        return ""
    t = title.lower().strip()
    result = []
    for ch in t:
        result.append(CYR_TO_LAT.get(ch, ch))
    t = "".join(result)
    t = re.sub(r'[^a-z0-9\s]', ' ', t)
    t = re.sub(r'\s+', ' ', t).strip()
    return t


def extract_keywords(title: str) -> set:
    """Extract keywords and bigrams from a normalized title."""
    t = normalize_title(title)
    words = t.split()
    keywords = {w for w in words if len(w) > 2 and w not in STOPWORDS}

    # Bigrams for more precise matching
    bigrams = set()
    for i in range(len(words) - 1):
        if len(words[i]) > 2 and len(words[i + 1]) > 2:
            bigrams.add(f"{words[i]} {words[i+1]}")

    return keywords | bigrams


def parse_olx_price(record: dict) -> int | None:
    p = record.get("price_uzs")
    if isinstance(p, (int, float)) and p > 0:
        return int(p)

    # Legacy fallback (shouldn't trigger on current-schema records)
    raw = record.get("price", "-")
    if raw == "-" or raw == "" or raw is None:
        return None
    try:
        val = float(raw)
    except (ValueError, TypeError):
        return None
    if val <= 0:
        return None
    if val < 500_000:
        return int(val * USD_TO_UZS)
    return int(val)


def parse_birbir_price(record: dict) -> int | None:
    price = record.get("price")
    if price is None:
        return None
    if isinstance(price, dict):
        value = price.get("value", 0)
        currency = price.get("currency", "UZS")
        if value and value > 0:
            return int(value * USD_TO_UZS) if currency == "USD" else int(value)
        return None
    if isinstance(price, (int, float)) and price > 0:
        currency = record.get("currency", "UZS")
        return int(price * USD_TO_UZS) if currency == "USD" else int(price)
    return None


# ── Incremental loader ─────────────────────────────────────────────────────

def read_current_records(path: str, schema_marker: str,
                          parse_price_fn, get_category_fn,
                          source_name: str,
                          last_line: int = 0) -> tuple[list[dict], int, int]:
    """
    Read only current-schema records from a JSONL export file.
    
    Args:
        path: Path to the JSONL file
        schema_marker: Field that identifies current schema (e.g. 'price_uzs' or 'publishedAt')
        parse_price_fn: Function to parse price from a record
        get_category_fn: Function to extract category
        source_name: 'olx' or 'birbir'
        last_line: Line number to resume from (0 = start from scratch, find schema boundary)
    
    Returns:
        (records, current_schema_start_line, total_lines_in_file)
    """
    records = []
    schema_start = None
    total_lines = 0
    started = False

    with open(path, "r", encoding="utf-8") as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            total_lines += 1

            try:
                raw = json.loads(line)
            except json.JSONDecodeError:
                continue

            # Detect current schema
            if schema_start is None and schema_marker in raw:
                schema_start = line_num

            # Skip legacy records
            if schema_marker not in raw:
                # If we already passed the boundary, this shouldn't happen
                # but handle gracefully
                if schema_start is not None:
                    continue
                # Still in legacy section
                continue

            # Only process records after our cursor
            if line_num <= last_line:
                continue

            # We're in current-schema territory
            if not started:
                started = True

            rec = {
                "_source": source_name,
                "_line": line_num,
                "id": str(raw.get("id", "")),
                "title": raw.get("title", ""),
                "url": raw.get("url", "") or (
                    f"https://birbir.uz/{raw.get('webUri', '')}"
                    if source_name == "birbir" else ""
                ),
                "category": get_category_fn(raw),
                "price_uzs": parse_price_fn(raw),
                "created": (
                    raw.get("created_time", "") or raw.get("publishedAt", "") or
                    raw.get("last_refresh_time", "") or raw.get("published_at", "")
                ),
            }

            if isinstance(rec["created"], (int, float)):
                dt = datetime.fromtimestamp(rec["created"] / 1000, tz=timezone.utc)
                rec["created"] = dt.isoformat()

            rec["keywords"] = list(extract_keywords(rec["title"]))

            records.append(rec)

    return records, schema_start, total_lines


def get_olx_category(raw: dict) -> str:
    return raw.get("category_type", "").strip().lower()


def get_birbir_top_category(raw: dict) -> str:
    path = raw.get("category_path", "")
    if path:
        return path.split("/")[0].strip().lower()
    web = raw.get("webUri", "")
    if web:
        parts = web.split("/")
        if len(parts) >= 4:
            return parts[3].strip().lower()
    return ""


# ── Cross-platform matching ────────────────────────────────────────────────

def match_items(olx_records: list[dict], birbir_records: list[dict]) -> list[dict]:
    """
    Find cross-platform matches using keyword/bigram overlap.
    Only current-schema records are passed in.
    """
    olx_r = [r for r in olx_records
             if r["category"] in RESELLABLE_CATEGORIES and r["price_uzs"] is not None]
    birbir_r = [r for r in birbir_records if r["price_uzs"] is not None]

    print(f"  Resellable with prices: {len(olx_r)} OLX, {len(birbir_r)} Birbir")

    if not olx_r or not birbir_r:
        return []

    # Index birbir by keywords
    kw_index = defaultdict(list)
    for br in birbir_r:
        for kw in br["keywords"]:
            kw_index[kw].append(br)

    matches = []
    matched_birbir_ids = set()

    for olx in olx_r:
        olx_kws = set(olx["keywords"])
        if not olx_kws:
            continue

        candidates = {}
        for kw in olx_kws:
            for br in kw_index[kw]:
                if br["id"] in matched_birbir_ids:
                    continue
                shared = olx_kws & set(br["keywords"])
                union = olx_kws | set(br["keywords"])
                score = len(shared) / len(union) if union else 0
                prev = candidates.get(br["id"])
                if prev is None or score > prev[0]:
                    candidates[br["id"]] = (score, shared)

        for br_id, (score, shared) in candidates.items():
            br = next(r for r in birbir_r if r["id"] == br_id)
            if len(shared) < 2 or score < 0.35:
                continue

            olx_p = olx["price_uzs"]
            br_p = br["price_uzs"]
            if olx_p > 0 and br_p > 0:
                ratio = max(br_p, olx_p) / min(br_p, olx_p)
                if ratio > 50:
                    continue
            if br_p <= olx_p:
                continue

            profit = br_p - olx_p
            margin = (profit / olx_p * 100) if olx_p > 0 else 0

            matches.append({
                "olx_id": olx["id"],
                "olx_title": olx["title"],
                "olx_price_uzs": olx_p,
                "olx_url": olx["url"],
                "olx_category": olx["category"],
                "birbir_id": br["id"],
                "birbir_title": br["title"],
                "birbir_price_uzs": br_p,
                "birbir_url": br["url"],
                "birbir_category": br["category"],
                "match_score": round(score, 3),
                "shared_keywords": len(shared),
                "profit_uzs": profit,
                "profit_margin_pct": round(margin, 1),
            })
            matched_birbir_ids.add(br_id)

    matches.sort(key=lambda m: m["profit_margin_pct"], reverse=True)
    return matches


# ── Category analysis ──────────────────────────────────────────────────────

def analyze_categories(olx_records: list[dict], birbir_records: list[dict]) -> dict:
    analysis = {}
    for source_name, records in [("olx", olx_records), ("birbir", birbir_records)]:
        by_cat = defaultdict(list)
        for r in records:
            cat = r["category"]
            if cat and r["price_uzs"] is not None:
                by_cat[cat].append(r["price_uzs"])

        for cat, prices in by_cat.items():
            if cat not in analysis:
                analysis[cat] = {}
            prices_sorted = sorted(prices)
            analysis[cat][source_name] = {
                "count": len(prices),
                "min": prices_sorted[0],
                "max": prices_sorted[-1],
                "median": prices_sorted[len(prices_sorted) // 2],
                "mean": int(sum(prices_sorted) / len(prices_sorted)),
            }
    return analysis


# ── Bargain detection ─────────────────────────────────────────────────────

def find_bargains(records: list[dict], source_name: str,
                  cat_analysis: dict, threshold: float = 0.5) -> list[dict]:
    """
    Find items priced significantly below the category median.
    
    threshold: items with price <= median * threshold are flagged as bargains.
    """
    bargains = []
    for r in records:
        cat = r["category"]
        if not cat or r["price_uzs"] is None:
            continue
        cat_info = cat_analysis.get(cat, {}).get(source_name)
        if not cat_info or cat_info["count"] < 3:
            continue
        median = cat_info["median"]
        if median > 0 and r["price_uzs"] <= median * threshold:
            discount_pct = round((1 - r["price_uzs"] / median) * 100, 1)
            bargains.append({
                "source": source_name,
                "id": r["id"],
                "title": r["title"],
                "price_uzs": r["price_uzs"],
                "category": cat,
                "category_median": median,
                "discount_pct": discount_pct,
                "url": r["url"],
                "created": r["created"],
            })
    bargains.sort(key=lambda b: b["discount_pct"], reverse=True)
    return bargains


# ── Scoring ────────────────────────────────────────────────────────────────

def score_opportunities(matches: list[dict], cat_analysis: dict) -> list[dict]:
    if not matches:
        return []

    margins = [m["profit_margin_pct"] for m in matches]
    abs_profits = [m["profit_uzs"] for m in matches]
    max_margin = max(margins) if margins else 1
    max_profit = max(abs_profits) if abs_profits else 1

    scored = []
    for m in matches:
        margin_score = (m["profit_margin_pct"] / max_margin) * 100 if max_margin else 0
        profit_score = (m["profit_uzs"] / max_profit) * 100 if max_profit else 0

        cat = m["olx_category"]
        cat_count = 0
        if cat in cat_analysis and "olx" in cat_analysis[cat]:
            cat_count = cat_analysis[cat]["olx"]["count"]
        liquidity_score = min(cat_count / 10000 * 100, 100)

        composite = (margin_score * 0.40 + profit_score * 0.30 + liquidity_score * 0.30)
        scored.append({**m, "score": round(composite, 1)})

    scored.sort(key=lambda x: x["score"], reverse=True)
    return scored


# ── Output ─────────────────────────────────────────────────────────────────

def save_results(opportunities: list[dict], cat_analysis: dict,
                 run_info: dict, bargains: list[dict] | None = None):
    output = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "run_info": run_info,
        "category_analysis": cat_analysis,
        "total_opportunities": len(opportunities),
        "opportunities": opportunities[:500],
        "bargains": bargains[:100] if bargains else [],
    }
    with open(OUTPUT_FILE, "w", encoding="utf-8") as f:
        json.dump(output, f, ensure_ascii=False, indent=2)
    print(f"\n  Results → {OUTPUT_FILE}")


def load_state() -> dict:
    if os.path.exists(STATE_FILE):
        try:
            with open(STATE_FILE) as f:
                return json.load(f)
        except (json.JSONDecodeError, OSError):
            pass
    return {}

def save_state(state: dict):
    with open(STATE_FILE, "w", encoding="utf-8") as f:
        json.dump(state, f, ensure_ascii=False, indent=2)


# ── Display ────────────────────────────────────────────────────────────────

def print_opportunities(opportunities: list[dict], top_n: int = 20):
    if not opportunities:
        print("  No matching opportunities found.")
        return

    print(f"\n  {'Rank':>4}  {'Score':>5}  {'Margin':>7}  {'Profit (UZS)':>13}  {'Cat':<10}  {'OLX Title':<50}")
    print(f"  {'─'*4}  {'─'*5}  {'─'*7}  {'─'*13}  {'─'*10}  {'─'*50}")
    for i, opp in enumerate(opportunities[:top_n], 1):
        print(f"  {i:>4}  {opp['score']:>5.1f}  {opp['profit_margin_pct']:>6.1f}%  {opp['profit_uzs']:>13,}  {opp['olx_category']:<10}  {opp['olx_title'][:48]}")

    print(f"\n  Total: {len(opportunities)} opportunities")
    if opportunities:
        print(f"  Top: {opportunities[0]['profit_uzs']:,} UZS profit  ({opportunities[0]['profit_margin_pct']}% margin)")


def print_bargains(bargains: list[dict], source_name: str, top_n: int = 10):
    bargains_src = [b for b in bargains if b["source"] == source_name]
    if not bargains_src:
        return
    print(f"\n  {source_name.title()} bargains (>50% below median):")
    print(f"  {'Rank':>4}  {'Discount':>8}  {'Price (UZS)':>12}  {'Median':>12}  {'Cat':<10}  {'Title':<50}")
    print(f"  {'─'*4}  {'─'*8}  {'─'*12}  {'─'*12}  {'─'*10}  {'─'*50}")
    for i, b in enumerate(bargains_src[:top_n], 1):
        print(f"  {i:>4}  {b['discount_pct']:>7.1f}%  {b['price_uzs']:>12,}  {b['category_median']:>12,}  {b['category']:<10}  {b['title'][:48]}")
    print(f"\n  Category price comparison (median UZS):")
    print(f"  {'Category':<20}  {'OLX cnt':>7}  {'OLX median':>12}  {'Birbir cnt':>10}  {'Birbir median':>14}  {'Diff':>14}")
    print(f"  {'─'*20}  {'─'*7}  {'─'*12}  {'─'*10}  {'─'*14}  {'─'*14}")
    for cat in sorted(cat_analysis.keys()):
        info = cat_analysis[cat]
        olx = info.get("olx", {})
        bir = info.get("birbir", {})
        diff = bir.get("median", 0) - olx.get("median", 0) if olx and bir else 0
        print(f"  {cat:<20}  {olx.get('count',0):>7}  {olx.get('median',0):>12,}  {bir.get('count',0):>10}  {bir.get('median',0):>14,}  {diff:>14,}")


# ── Main ───────────────────────────────────────────────────────────────────

def main():
    start = time.time()

    full_reload = "--full" in sys.argv
    top_n = 20
    for arg in sys.argv:
        if arg.startswith("--top="):
            top_n = int(arg.split("=")[1])

    print("═" * 60)
    print("  Reselling Analysis Engine (current-schema only)")
    print("═" * 60)

    # Load state (tracking last line processed per file)
    state = load_state()
    olx_cursor = 0 if full_reload else state.get("olx_last_line", 0)
    birbir_cursor = 0 if full_reload else state.get("birbir_last_line", 0)

    if full_reload:
        print("\n  Full re-analysis mode")
    else:
        print(f"\n  Incremental mode — OLX cursor: line {olx_cursor}, Birbir cursor: line {birbir_cursor}")

    # Load only current-schema records
    print("\n  Reading OLX current-schema records...")
    olx_records, olx_schema_start, olx_total = read_current_records(
        OLX_EXPORT, "price_uzs", parse_olx_price, get_olx_category, "olx", olx_cursor
    )
    print(f"    {len(olx_records)} new records (schema starts at line {olx_schema_start}, total file: {olx_total} lines)")

    print("\n  Reading Birbir current-schema records...")
    birbir_records, birbir_schema_start, birbir_total = read_current_records(
        BIRBIR_EXPORT, "publishedAt", parse_birbir_price, get_birbir_top_category, "birbir", birbir_cursor
    )
    print(f"    {len(birbir_records)} new records (schema starts at line {birbir_schema_start}, total file: {birbir_total} lines)")

    run_info = {
        "olx_new": len(olx_records),
        "birbir_new": len(birbir_records),
        "olx_schema_start": olx_schema_start,
        "birbir_schema_start": birbir_schema_start,
        "olx_total_lines": olx_total,
        "birbir_total_lines": birbir_total,
        "olx_cursor": olx_cursor,
        "birbir_cursor": birbir_cursor,
        "full_reload": full_reload,
    }

    if not olx_records and not birbir_records:
        print("\n  No new records to analyze.")
        save_state({
            "olx_last_line": olx_total,
            "birbir_last_line": birbir_total,
            "last_run": datetime.now(timezone.utc).isoformat(),
        })
        return

    if not olx_records or not birbir_records:
        print(f"\n  {'No OLX' if not olx_records else 'No Birbir'} current-schema records to cross-reference. Results partial.")

    # Category analysis
    print("\n  Analyzing categories...")
    cat_analysis = analyze_categories(olx_records, birbir_records)
    print_category_summary(cat_analysis)

    # Find bargains (items well below category median)
    bargains = []
    if cat_analysis:
        bargains.extend(find_bargains(olx_records, "olx", cat_analysis))
        bargains.extend(find_bargains(birbir_records, "birbir", cat_analysis))
        bargains.sort(key=lambda b: b["discount_pct"], reverse=True)
        print_bargains(bargains, "olx")
        print_bargains(bargains, "birbir")

    # Cross-platform matching (only if we have both)
    if olx_records and birbir_records:
        print("\n  Matching items across platforms...")
        matches = match_items(olx_records, birbir_records)

        if matches:
            print("\n  Scoring opportunities...")
            opportunities = score_opportunities(matches, cat_analysis)
            print_opportunities(opportunities, top_n=top_n)
            save_results(opportunities, cat_analysis, run_info, bargains)
        else:
            print("\n  No cross-platform matches found.")
            save_results([], cat_analysis, run_info, bargains)
    else:
        save_results([], cat_analysis, run_info, bargains)

    # Save cursor position for next incremental run
    save_state({
        "olx_last_line": olx_total,
        "birbir_last_line": birbir_total,
        "last_run": datetime.now(timezone.utc).isoformat(),
    })

    elapsed = time.time() - start
    print(f"\n  Done in {elapsed:.1f}s")


if __name__ == "__main__":
    main()

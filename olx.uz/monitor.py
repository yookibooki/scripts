#!/usr/bin/env python3
"""
OLX.uz New Posts Monitor

Lightweight HTTP-only monitor. Polls the OLX JSON API directly.
No browser, no Playwright, no Node.js.

Output: olx_posts.json  (JSONL)
State:  state.json      (sorted list of seen IDs)
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request

# ─── Config ─────────────────────────────────────────────────────────

BASE = "https://www.olx.uz"
API = f"{BASE}/api/v1/offers"
HEADERS = {
    "Accept": "application/json",
    "Accept-Language": "ru-RU,ru;q=0.9,en;q=0.8,uz;q=0.7",
    "Referer": "https://www.olx.uz/",
    "User-Agent": (
        "Mozilla/5.0 (X11; Linux x86_64) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/130.0.0.0 Safari/537.36"
    ),
}
PAGES = 2
INTERVAL_MS = int(os.environ.get("POLL_INTERVAL", "15000"))
STATE_FILE = "state.json"
OUTPUT_FILE = "olx_posts.json"


# ─── State ──────────────────────────────────────────────────────────

def load_state() -> set[int]:
    try:
        with open(STATE_FILE) as f:
            return set(json.load(f))
    except (FileNotFoundError, json.JSONDecodeError):
        return set()


def save_state(ids: set[int]) -> None:
    tmp = STATE_FILE + ".tmp"
    with open(tmp, "w") as f:
        json.dump(sorted(ids), f)
    os.replace(tmp, STATE_FILE)


def append_record(rec: dict) -> None:
    with open(OUTPUT_FILE, "a") as f:
        f.write(json.dumps(rec, ensure_ascii=False) + "\n")


# ─── HTTP helpers ───────────────────────────────────────────────────

def fetch_json(url: str) -> dict | list | None:
    req = urllib.request.Request(url, headers=HEADERS)
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            return json.loads(resp.read())
    except (urllib.error.HTTPError, urllib.error.URLError, OSError):
        return None


def get_offers(offset: int, limit: int) -> list:
    data = fetch_json(f"{API}/?offset={offset}&limit={limit}&query=")
    return (data or {}).get("data") or []


def get_phones(ad_id: int) -> list[str]:
    data = fetch_json(f"{API}/{ad_id}/limited-phones/")
    if data and isinstance(data, dict):
        return (data.get("data") or {}).get("phones") or []
    return []


# ─── Extract fields ─────────────────────────────────────────────────

def extract(offer: dict) -> dict:
    import re

    desc = (offer.get("description") or "")
    desc = re.sub(r"<[^>]+>", " ", desc)
    desc = re.sub(r"\s+", " ", desc).strip()

    loc = offer.get("location") or {}
    cat = offer.get("category") or {}

    # Price
    price = None
    currency = None
    for p in offer.get("params") or []:
        if p.get("key") == "price" and isinstance(p.get("value"), dict):
            v = p["value"]
            price = v.get("converted_value") or v.get("value")
            currency = v.get("currency")
            break

    # Location parts
    parts = []
    for k in ("region", "city"):
        s = loc.get(k)
        if s and isinstance(s, dict) and s.get("name"):
            parts.append(s["name"])

    url = offer.get("url") or ""
    if not url.startswith("http"):
        url = BASE + url

    return {
        "id": offer.get("id"),
        "title": (offer.get("title") or "").strip(),
        "url": url,
        "description": desc,
        "created_time": offer.get("created_time") or "",
        "phones": [],
        "category": (cat.get("type") or "") if isinstance(cat, dict) else "",
        "location": ", ".join(parts),
        "price": price,
        "currency": currency,
    }


# ─── Poll ───────────────────────────────────────────────────────────

def poll(seen: set[int]) -> int:
    count = 0
    for pg in range(1, PAGES + 1):
        offset = (pg - 1) * 40
        offers = get_offers(offset, 40)
        if not offers:
            break

        for offer in offers:
            oid = offer.get("id")
            if not oid or oid in seen:
                return count
            seen.add(oid)

            rec = extract(offer)
            contact = offer.get("contact") or {}
            if contact.get("phone") is True:
                time.sleep(0.4)
                rec["phones"] = get_phones(oid)
            rec["detected_at"] = time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime())
            append_record(rec)
            count += 1
    return count


# ─── Main ───────────────────────────────────────────────────────────

def main() -> None:
    seen = load_state()
    print(f"Tracking {len(seen)} IDs.", flush=True)

    while True:
        t0 = time.perf_counter()
        n = 0
        try:
            n = poll(seen)
        except Exception as e:
            print(f"Poll error: {e}", flush=True)

        save_state(seen)
        elapsed = time.perf_counter() - t0
        print(f"Round: {n} new in {elapsed:.1f}s | tracked: {len(seen)}", flush=True)

        sleep_for = max(1.0, (INTERVAL_MS / 1000) - elapsed)
        time.sleep(sleep_for)


if __name__ == "__main__":
    main()

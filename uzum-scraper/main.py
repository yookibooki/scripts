import json, os, sqlite3, time, requests

GQL, TOK = "https://graphql.uzum.uz/", "https://id.uzum.uz/api/auth/token"
CATS = "https://api.uzum.uz/api/main/root-categories?eco=false"

# One call returns 100 full cards — no per-product requests. priceBlock lives under
# discovery or buyingOptions (API A/B-flips); minSellPrice is the last resort.
Q = """query($q: MakeSearchQueryInput!) {
  makeSearch(query: $q) {
    total
    items {
      catalogCard {
        productId
        title
        category { title }
        minSellPrice
        discovery {
          priceBlock { sellPrice { amount } finalPrice { amount } fullPrice { amount } sellerPrice { amount } }
          photos { key }
        }
        buyingOptions {
          priceBlock { sellPrice { amount } finalPrice { amount } fullPrice { amount } sellerPrice { amount } }
        }
      }
    }
  }
}"""

# Light probe: total count + first page of IDs, to decide whether to deep-scrape.
P = """query($q: MakeSearchQueryInput!) {
  makeSearch(query: $q) { total items { catalogCard { productId } } }
}"""


def token(s):
    return "Bearer " + s.post(TOK, timeout=15, headers={
        "Authorization": "Bearer ", "Origin": "https://uzum.uz",
        "Referer": "https://uzum.uz/", "Accept-Language": "uz"}).cookies["access_token"]


def post(s, *a, **k):
    k.setdefault("timeout", (5, 15))  # connect ≤5s; each read ≤15s (a trickle keeps a plain timeout alive forever)
    r = s.post(*a, **k)
    r.raise_for_status()
    return r


s = requests.Session()
s.headers.update({"x-iid": "ec90b009-eb59-4897-986d-a156f6ee638d",
                  "apollographql-client-name": "web-customers"})
s.headers["Authorization"] = token(s)

# Leaves live at depth >= 2; cache their ids for a day to skip the 470KB categories call.
def leaf_ids(nodes):
    for n in nodes:
        yield from leaf_ids(n["children"]) if n.get("children") else (n["id"],)

def leaves():
    try:
        if time.time() - os.path.getmtime("leaf_ids.json") < 86400:
            return json.load(open("leaf_ids.json"))
    except OSError:
        pass
    ids = list(leaf_ids(s.get(CATS, timeout=15).json()["payload"]))
    json.dump(ids, open("leaf_ids.json", "w"))
    return ids


def price(card):
    pbs = [((card.get("discovery") or {}).get("priceBlock") or {}),
           ((card.get("buyingOptions") or {}).get("priceBlock") or {})]
    for pb in pbs:
        if (a := (pb.get("finalPrice") or {}).get("amount")) is not None:
            return a
    rest = [a for pb in pbs for f in ("sellPrice", "sellerPrice", "fullPrice")
            if (a := (pb.get(f) or {}).get("amount")) is not None]
    return min(rest) if rest else card.get("minSellPrice")


db = sqlite3.connect("sqlite.db")
db.execute("""CREATE TABLE IF NOT EXISTS items (productId INTEGER PRIMARY KEY, title TEXT,
             category TEXT, price INTEGER, photoUrls TEXT, date INTEGER)""")
db.execute("""CREATE TABLE IF NOT EXISTS scrape_state
             (categoryId INTEGER PRIMARY KEY, done_at INTEGER, total INTEGER)""")
# Older DBs may lack the total column; add it if missing (harmless no-op otherwise).
cols = {r[1] for r in db.execute("PRAGMA table_info(scrape_state)")}
if "total" not in cols:
    db.execute("ALTER TABLE scrape_state ADD COLUMN total INTEGER")
    db.commit()

# State maps: done_at + last-known total per category.
done = {r[0]: r[2] for r in db.execute("SELECT categoryId, done_at, total FROM scrape_state")}

cats = [c for c in leaves() if c not in done]
if os.environ.get("MAX_CATS"):  # test-run cap; omit for a full scrape
    cats = cats[: int(os.environ["MAX_CATS"])]

now = int(time.time())
for i, cid in enumerate(cats):
    seen = set()
    try:
        # Probe: total + first page of IDs. If total matches the last known
        # count, the category is unchanged — skip the deep scrape entirely.
        base = {"categoryId": cid, "showAdultContent": "TRUE", "filters": [],
                "sort": "BY_RELEVANCE_DESC", "pagination": {"offset": 0, "limit": 100}}
        d = post(s, GQL, json={"query": P, "variables": {"q": base}}).json()["data"]["makeSearch"]
        total = d["total"]
        if total == done.get(cid):
            print(f"[{i+1}/{len(cats)}] cat {cid}: unchanged ({total})")
            continue
        # Deep-scrape only if the count changed.
        for off in range(0, 9900, 100):  # offset >= 9900 rejected by the API
            base["pagination"] = {"offset": off, "limit": 100}
            items = post(s, GQL, json={"query": Q, "variables": {"q": base}}) \
                .json()["data"]["makeSearch"]["items"]
            fresh = [c for c in (i.get("catalogCard") for i in items)
                     if c and c.get("productId") is not None and c["productId"] not in seen]
            seen |= {c["productId"] for c in fresh}
            new_rows = [(c["productId"], c["title"], (c.get("category") or {}).get("title"), price(c),
                         json.dumps([p["key"] for p in ((c.get("discovery") or {}).get("photos") or []) if "key" in p]), now)
                        for c in fresh]
            db.executemany("INSERT OR REPLACE INTO items VALUES (?, ?, ?, ?, ?, ?)", new_rows)
            db.commit()  # partial categories survive crashes; reruns just REPLACE
            if len(items) < 100 or not fresh:
                break  # end of catalog or an all-duplicate page
    except Exception as e:
        print(f"[{i+1}/{len(cats)}] cat {cid}: FAILED ({e}); rows kept, retried next run")
        continue
    done[cid] = total
    db.execute("INSERT OR REPLACE INTO scrape_state VALUES (?, ?, ?)", (cid, now, total))
    db.commit()
    print(f"[{i+1}/{len(cats)}] cat {cid}: {len(seen)} products (total {total})")
db.close()

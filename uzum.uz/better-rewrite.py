import os, json, requests, uuid, sys

GQL = "https://graphql.uzum.uz/"
CAT = "https://api.uzum.uz/api/main/root-categories?eco=false"
TOK = "https://id.uzum.uz/api/auth/token"
Q = "query($q: MakeSearchQueryInput!) { makeSearch(query: $q) { total items { catalogCard { productId title minFullPrice minSellPrice feedbackQuantity rating buyingOptions { isSingleSku deliveryOptions { shortDate stockType } } promoFutureInfo { minFuturePrice minFuturePriceDate } badges { id text backgroundColor textColor } } } } }"

def refresh(s):
    t = s.cookies.get("access_token") or s.headers.get("Authorization", "").split()[-1]
    r = s.post(TOK, headers={"Authorization": f"Bearer {t}", "Origin": "https://uzum.uz", "Referer": "https://uzum.uz/"})
    for c in r.headers.get("set-cookie", "").split(";"):
        if "access_token=" in c:
            s.cookies.set("access_token", c.split("=")[1].strip())
            s.headers["Authorization"] = f"Bearer {s.cookies['access_token']}"
            return
    print("[WARN] Refresh failed, proceeding with current token.")

def gql(s, v):
    r = s.post(GQL, json={"query": Q, "variables": v})
    if r.status_code == 401: refresh(s); r = s.post(GQL, json={"query": Q, "variables": v})
    res = r.json()
    # 1-LINE FIX: Prevents NoneType crash and prints the actual API error if it fails
    if not res.get("data"): sys.exit(f"[ERROR] API rejected query: {res.get('errors')}")
    return res["data"]["makeSearch"]

def get_leaves(nodes):
    leaves = []
    for n in nodes:
        if not n.get("children"): leaves.append(str(n["id"]))
        else: leaves.extend(get_leaves(n["children"]))
    return leaves

def main():
    os.makedirs("data", exist_ok=True)
    s = requests.Session()
    s.headers.update({"User-Agent": "Mozilla/5.0", "Accept": "application/json", "city-id": "1", "apollographql-client-name": "web-customers"})
    s.cookies.set("clickstream-client.installId", f'"{os.environ.get("UZUM_INSTALL_ID", uuid.uuid4())}"')

    token = os.environ.get("UZUM_ACCESS_TOKEN")
    if not token: sys.exit("Set UZUM_ACCESS_TOKEN env var")
    s.cookies.set("access_token", token)
    s.headers["Authorization"] = f"Bearer {token}"
    refresh(s)

    state_path, out_path = "data/state.json", "data/uzum_data.jsonl"
    state = json.load(open(state_path)) if os.path.exists(state_path) else {}

    if not state:
        r = s.get(CAT)
        if r.status_code == 401: refresh(s); r = s.get(CAT)
        state = {lid: 0 for lid in get_leaves(r.json()["payload"])}
        print(f"[INFO] Found {len(state)} leaf categories.")

    with open(out_path, "a") as f:
        for cid, max_id in list(state.items()):
            for offset in range(0, 9900, 100):
                items = gql(s, {"q": {"categoryId": cid, "showAdultContent": "TRUE", "filters": [], "sort": "BY_DATE_ADDED_DESC", "pagination": {"offset": offset, "limit": 100}}}).get("items", [])
                if not items: break

                for item in items:
                    pid = item["catalogCard"]["productId"]
                    if pid <= max_id: break # Instant break on old ID
                    json.dump(item, f); f.write("\n")
                    max_id = pid
                else: continue
                break

            state[cid] = max_id
            json.dump(state, open(state_path, "w"))
            print(f"[INFO] Category {cid} done. Max ID: {max_id}")

if __name__ == "__main__":
    main()

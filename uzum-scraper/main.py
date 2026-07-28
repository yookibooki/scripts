import requests
GQL = "https://graphql.uzum.uz/"
TOK = "https://id.uzum.uz/api/auth/token"
Q = "query($q: MakeSearchQueryInput!) { makeSearch(query: $q) { total items { catalogCard { productId title minFullPrice minSellPrice feedbackQuantity rating buyingOptions { isSingleSku deliveryOptions { shortDate stockType } } promoFutureInfo { minFuturePrice minFuturePriceDate } badges { id text backgroundColor textColor } } } } }"
s = requests.Session()
s.headers.update({"x-iid": "ec90b009-eb59-4897-986d-a156f6ee638d", "apollographql-client-name": "web-customers"})
s.headers["Authorization"] = "Bearer " + s.post(TOK, headers={"Referer": "https://uzum.uz/", "Accept-Language": "uz"}).cookies["access_token"]
cats = [n["id"] for n in s.get("https://api.uzum.uz/api/main/root-categories?eco=false").json()["payload"] if not n.get("children")]
with open("data/uzum_data.jsonl", "a") as f:
    for cid in cats:
        for o in range(0, 9900, 100):
            items = s.post(GQL, json={"query": Q, "variables": {"q": {"categoryId": cid, "showAdultContent": "TRUE", "filters": [], "sort": "BY_DATE_ADDED_DESC", "pagination": {"offset": o, "limit": 100}}}}).json().get("data", {}).get("makeSearch", {}).get("items", [])
            if not items: break
            for i in items: print(json.dumps(i), file=f)

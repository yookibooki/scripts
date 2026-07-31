You have full authority over the codebase—you may modify it in any way, refactor it, or even rewrite it entirely from the ground up, acting as an entirely independent entity. Keep the code as concise as possible and minimize server requests to achieve maximum speed.
Your process should be:
- Streamline main.py to the smallest viable version
- Execute it
- inpect the SQLite database after run
- Repeat the cycle
- Utilize DevTools whenever you work with the API

# Uzum Scraper
## Auth
POST `id.uzum.uz/api/auth/token`
Headers: `Authorization: Bearer `,`Origin: https://uzum.uz`,`Referer: https://uzum.uz/`,`Accept-Language: uz`
Res: `204` + `access_token` cookie

## GraphQL
POST `graphql.uzum.uz/`
Headers: `x-iid: ec90b009-eb59-4897-986d-a156f6ee638d`, `apollographql-client-name: web-customers`
Payload: `{"query","variables"}` -> `data.makeSearch.items`

## Notes
- HTTP 429 = bot detection, not rate limits
- `limit` <= 100, max `offset` 9800 — paginate per leaf category so totals stay under the cap
- All root categories have children; leaf categories live at depth >= 2 (1,624 total) — iterate leaves, not roots
- 6 columns in sqlite.db: productId,title,category,price,photoUrls,timestamp

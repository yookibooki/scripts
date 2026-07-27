# BirBir.uz Live API Investigation Findings

> Date: 2026-07-27
> Agent: BirbirInvestigation
> Source: Live Chrome DevTools MCP investigation of https://birbir.uz/
> Existing wiki updated: `/home/dev/workspace/scripts/birbir.uz/wiki/birbir-api-reference.md`

---

## 1. Infrastructure

| Component | Value |
|-----------|-------|
| API domain | `api.birbir.uz` |
| API version | `1.3.5.0` |
| Base path | `/api/frontoffice/1.3.5.0` |
| Image CDN | `img.birbir.uz` |
| File CDN | `file.birbir.uz` |
| WebSocket | `socket.birbir.uz` (Centrifugo) |
| Sentry | `sentry.doska-tech.uz` (DSN: `153f7f85e875af52ece61606eceb07cd`) |
| Analytics | Amplitude (`api2.amplitude.com`, app ID `11821a4e9c78923d2816a71ceb1bf0f2`) |
| Security | Cloudflare JS challenge on main site |
| Gateway | istio-envoy |
| Backend | PHP/Akene-based (error code conventions) |
| Runtime | React SPA (Webpack chunks, React Helmet) |

---

## 2. Authentication Flow

### 2.1 Cookie Format

The `session` cookie is URL-encoded JSON prefixed with `j:`:

```
session=j%3A{"accessToken":"eyJ...","refreshToken":"eyJ...","tokenType":"bearer","deviceUuid":"..."}
```

Decoded example:

```json
{
  "accessToken": "eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzUxMiJ9...",
  "refreshToken": "eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzUxMiJ9...",
  "tokenType": "bearer",
  "deviceUuid": "685a980f-e58b-4e72-baac-baec9ec73d58"
}
```

### 2.2 JWT Token Structure

- **Algorithm**: ES512 (ECDSA with P-521)
- **Header**: `{"typ":"JWT","alg":"ES512"}`
- **Payload**:

```json
{
  "jti": "ffd5fde4-a22e-495b-8181-d1a603f5aded",
  "iat": 1785143732,
  "exp": 1785158132,
  "u": "22583337-6e37-47ed-a840-c546a1b2856d",
  "ut": 10,
  "ip": "95.214.210.230",
  "t": 1,
  "dt": "2026-07-27T09:14:14.072548Z",
  "piat": "2026-07-27T09:14:14.000000Z",
  "di": {
    "uuid": "685a980f-e58b-4e72-baac-baec9ec73d58",
    "os": "Linux",
    "name": "Chrome",
    "acceptLanguage": "uz"
  },
  "v": "1.3.5.0"
}
```

### 2.3 JWT Claims

| Claim | Description |
|-------|-------------|
| `jti` | Token UUID |
| `iat` | Issued-at timestamp (Unix) |
| `exp` | Expiry timestamp (Unix) — ~4 hours after `iat` |
| `u` | User UUID |
| `ut` | User type (10 = regular user) |
| `ip` | Client IP address |
| `t` | Token type (1 = access token) |
| `dt` | ISO 8601 datetime string |
| `piat` | ISO 8601 parsed datetime |
| `di` | Device info object (`uuid`, `os`, `name`, `acceptLanguage`) |
| `v` | API version string |

### 2.4 Token Expiry

- `iat` to `exp` = 14400 seconds (4 hours)
- On expiry, API returns `ACCESS_TOKEN_INVALID`
- Refresh requires re-authentication via browser (Cloudflare JS challenge)

### 2.5 Request Headers for Authenticated Calls

| Header | Value |
|--------|-------|
| `Authorization` | `Bearer <JWT>` |
| `Accept` | `application/json` |
| `Content-Type` | `application/json` (POST only) |
| `x-current-language` | `uz` (or `ru`) |
| `x-current-region` | `toshkent` (region slug) |
| `referer` | `https://birbir.uz/` |
| `origin` | `https://birbir.uz` |

### 2.6 Response Headers (All API Endpoints)

| Header | Value |
|--------|-------|
| `cache-control` | `no-cache, private` |
| `content-type` | `application/json; charset=utf-8` |

---

## 3. Auth Endpoints

### 3.1 POST /api/frontoffice/1.3.5.0/auth/enter-by-phone

- **Purpose**: Phone-based authentication
- **Observed**: Present in network traffic, but requires phone number input flow
- **Auth**: Not tested (requires UI flow to trigger)
- **Status on direct call without token**: 401 `ACCESS_TOKEN_INVALID`

---

## 4. API Endpoints Discovered

### 4.1 GET /api/frontoffice/1.3.5.0/user

**Authenticated response (200)**:

```json
{
  "content": {
    "uuid": "22583337-6e37-47ed-a840-c546a1b2856d",
    "profileNumber": "199288449",
    "phone": 998900116678,
    "region": { ... },
    "currency": "UZS",
    "name": "Javohir",
    "isAnonymous": false,
    "language": "uz",
    "analyticsInfo": [...],
    "offerConfirmRequired": false,
    "centrifugo": {
      "wsUrl": "wss://socket.birbir.uz/connection/websocket",
      "private": "private:685a980f-e58b-4e72-baac-baec9ec73d58#22583337-6e37-47ed-a840-c546a1b2856d"
    },
    "business": false,
    "agency": false,
    "verified": false,
    "preferenceState": 100,
    "searchRadius": null,
    "sellerTier": null,
    "hasCustomAvatar": false
  }
}
```

**Unauthenticated response (401)**:

```json
{
  "content": null,
  "error": {
    "code": "ACCESS_TOKEN_INVALID",
    "message": "Access token invalid.",
    "alert": null
  }
}
```

### 4.2 GET /api/frontoffice/1.3.5.0/user/profile

**Response (200)**:

```json
{
  "content": {
    "phone": 998900116678,
    "currency": "UZS",
    "name": "Javohir",
    "preferredContactWay": [1, 2],
    "avatar": {
      "uuid": "28fda56f-e269-49b0-b44c-8eebcffa2b18",
      "type": 1,
      "originName": "Default.png",
      "fileSize": 43658,
      "width": 1600,
      "height": 1601,
      "cropUrlTemplate": "https://img.birbir.uz/i/%s/files/94/5b/8a93afd671b2e9bec7df4712455d.png"
    },
    "language": "uz",
    "hasCustomAvatar": false
  }
}
```

### 4.3 GET /api/frontoffice/1.3.5.0/popup?positions[]=1

**Response (200)**:

```json
{ "content": { "items": [] } }
```

No popups observed for position 1 in this session.

### 4.4 POST /api/frontoffice/1.3.5.0/offer/feed

**Request body**:

```json
{
  "page": 1,
  "perPage": 40,
  "region": "all",
  "sort": 2
}
```

**Success response (200)**: Wrapped in `{"content": { "items": [...], "paginator": {...} }}`

See §6 for full offer schema.

### 4.5 GET /api/frontoffice/1.3.5.0/offer/{id}

**Response (200)**: Full offer detail including `description`, `features`, `location`, `path`, `askSeller`, `toast`, `delivery`, etc.

**Not found (404)**: `{"content":null,"error":{"code":"NOT_FOUND","message":"Entity not found","alert":null}}`

### 4.6 GET /api/frontoffice/1.3.5.0/offer/feed (wrong method)

**Response (405)**:

```json
{
  "content": null,
  "error": {
    "code": "METHOD_NOT_ALLOWED",
    "message": "No route found for \"GET https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/feed\": Method Not Allowed (Allow: POST)",
    "alert": null
  }
}
```

### 4.7 GET /api/frontoffice/1.3.5.0/chat/dialog?perPage=40

**Response (200)**: Returns chat dialog list with `items` array containing conversation metadata.

### 4.8 POST /api/frontoffice/1.3.5.0/auth/enter-by-phone

**Response (401)** when called without auth header: `ACCESS_TOKEN_INVALID`. Requires session cookie for auth.

---

## 5. Endpoints NOT Found (404)

| Endpoint | Status |
|----------|--------|
| `/category` | 404 `NOT_FOUND` |
| `/offers/categories` | 404 `NOT_FOUND` |
| `/category/tree` | 404 `NOT_FOUND` |
| `/regions` | 404 `NOT_FOUND` |
| `/region` | 404 `NOT_FOUND` |
| `/offer/1` | 404 `NOT_FOUND` (entity not found for non-existent ID) |

The category system does not have a public REST endpoint. Categories are embedded in `webUri` paths and `titlePath` arrays within offer data.

---

## 6. Offer Schema (Full)

From `POST /api/frontoffice/1.3.5.0/offer/feed` response:

```typescript
interface Offer {
  id: number;                    // e.g. 275144373
  slug: string;                  // e.g. "gorshok"
  primaryPhoto: Photo;
  price: Price;                  // { value: number, currency: "UZS" | "USD" }
  priceType: number;             // 1 = fixed, 2 = negotiable, 3 = service
  priceUzs?: Price;              // Present when currency is USD, shows UZS equivalent
  title: string;
  region: Region;
  favorited: boolean;
  urgentSale: boolean;
  courierDelivery: boolean;
  publishedAt: number;           // Epoch ms
  webUri: string;                // e.g. "uz/toshkent/cat/.../o/gorshok-275144373"
  webUriInfo: { uz: string; ru: string };
  business: boolean;
  agency: boolean;
  photos: Photo[];
  seller: Seller;
  promotion: Promotion;
  priceSubscribed: boolean;
  priceSubscriptionAvailable: boolean;
  grossPrice?: Price;            // Present when discount exists
  grossPriceDiscount?: number;   // Percentage discount
  analyticsInfo: AnalyticsInfo[];
  delivery: Delivery | null;
  distance: number | null;
  badges: Badge[];
  closed: boolean;
  contactViaApp: boolean;
  similarFeedAvailable: boolean | null;
  features?: Feature[];          // Present on single-offer GET
  description?: string;          // Present on single-offer GET
  location?: Location;           // Present on single-offer GET
  path?: CategoryPath[];         // Present on single-offer GET
  askSeller?: AskSeller;         // Present on single-offer GET
  toast?: Toast;                 // Present on single-offer GET
  translationAvailable?: boolean;
  bnplForm?: any;
  askDiscount?: any;
  inFavoriteCount?: number;
  staticMapStyle?: number;
  overlay?: any;
  topFeature?: any;
}

interface Photo {
  id: number;
  upload: Upload;
  uploadCropped: null;
  alt: string;
}

interface Upload {
  uuid: string;
  type: number;
  originName: string;
  fileSize: number;
  width: number;
  height: number;
  cropUrlTemplate: string;
}

interface Price {
  value: number;
  currency: "UZS" | "USD";
}

interface Region {
  id: number;
  key: string;
  title: string;
  whereTitle: string;
  location: { type: "Point"; coordinates: [number, number] };
  webUri: string;
  webUriInfo: { uz: string; ru: string };
  isWholeCountry: boolean;
  titlePath: string[];
}

interface Seller {
  uuid: string;
  name: string;
  registeredDate: string;
  showRegisteredDate: boolean;
  lastAccessDate: string;
  avatar: Upload;
  preferredContactWay: number[];
  chatAllowed: boolean;
  verified: boolean;
  business: boolean;
  agency: boolean;
  proSeller: boolean | null;
  inFavoriteCount: number;
  favorited: boolean;
  offerTotalCount: number;
  offerActiveCount: number;
  review?: ReviewStats;
  activity?: SellerActivity;
}

interface ReviewStats {
  createAvailable: boolean;
  score: number;
  totalCount: number;
  // ... five/four/three/two/one count breakdowns
}

interface SellerActivity {
  online: boolean;
  responseTimeStatus: number;
  responseTimeTitle: string;
}

interface Promotion {
  enabled: boolean;
  features: number[];
}

interface Delivery {
  available: boolean;
  popupTitle: string;
  popupDescription: string;
  popupButton: string;
  button: string;
  label: string;
  buttonBadge: string;
} | null;

interface Badge {
  type: number;
  style: number;
  title: string | null;
  position: number;
}

interface AnalyticsInfo {
  name: string;
  type: number;    // 1=string, 2=offer_id, 3=boolean, 5=list
  value: string;
}

interface Feature {
  id: number;
  title: string;
  description: string | null;
  featureValues: FeatureValue[];
  type: number;    // 1=status, 2=category
}

interface FeatureValue {
  webUriInfo: { uz: string; ru: string } | null;
  formattedValue: string;
  option: { id: number; title: string; description: string; color: any; image: any } | null;
}

interface Location {
  point: { type: "Point"; coordinates: [number, number] };
  identity: string;
  components: LocationComponent[];
  fullAddress: string;
  description: string | null;
  title: string;
  subtitle: string;
}

interface LocationComponent {
  kind: number;    // 100=country, 120=region, 200=city, 300=district
  title: string;
}

interface AskSeller {
  startPlaceholder: string;
  questions: AskQuestion[];
}

interface AskQuestion {
  id: number;
  selected: boolean;
  title: string;
  value: string;
  placeholder: string;
  actionType: number;
}

interface Toast {
  id: number;
  body: string;
}
```

---

## 7. Category System

There is no dedicated category REST endpoint. Categories work through the web URI path system:

- Offers contain `webUri` paths like `uz/toshkent/cat/gozallik-va-salomatlik/gigiena-vositalari/o/gorshok-275144373`
- The `path` array on single-offer detail shows the category hierarchy with `id`, `title`, `key`, `uri`, `webUri`
- `titlePath` in the `region` object of the feed shows breadcrumb categories
- `category_id` in `analyticsInfo` links to a numeric category ID
- Category names are transliterated URL slugs (e.g., `gozallik-va-salomatlik` = "Красота и здоровье")

---

## 8. Error Responses

All errors follow the same structure:

```json
{
  "content": null,
  "error": {
    "code": "<ERROR_CODE>",
    "message": "<Human-readable message>",
    "alert": null
  }
}
```

### Error Codes

| Code | HTTP Status | Meaning | Observed Message |
|------|-------------|---------|------------------|
| `ACCESS_TOKEN_INVALID` | 401 | Missing, expired, or corrupted JWT token | "Access token invalid." or "Access token invalid: Invalid signature" |
| `METHOD_NOT_ALLOWED` | 405 | Wrong HTTP method | "No route found for ... Method Not Allowed (Allow: POST)" |
| `NOT_FOUND` | 404 | Endpoint or entity not found | "No route found ..." or "Entity not found" |
| `INTERNAL_ERROR` | 500 | Server-side error | Not directly observed (CORS would block most error responses from `api.birbir.uz` when called from `www.olx.uz` origin) |

Note: When calling API from a different origin (e.g., `www.olx.uz`), CORS blocks the response body. The `origin` header must be set to `https://birbir.uz` to get proper error responses.

---

## 9. WebSocket / Centrifugo Details

- **WebSocket URL**: `wss://socket.birbir.uz/connection/websocket`
- **Private channel**: `private:685a980f-e58b-4e72-baac-baec9ec73d58#22583337-6e37-47ed-a840-c546a1b2856d`
- **Pattern**: `private:<device-uuid>#<user-uuid>`
- **Connection method**: Established via `@teletracncentrifuge/client` JS SDK (found in page source)
- **Usage**: Real-time chat, notifications, live offer updates
- **No direct WebSocket frame capture possible** from the current page context, but the URL format reveals the private channel subscription pattern

---

## 10. Additional Endpoints Discovered from Network Traffic

| Method | URL | Notes |
|--------|-----|-------|
| GET | `https://api.birbir.uz/api/frontoffice/1.3.5.0/chat/dialog?perPage=40` | Chat dialogs list |
| GET | `https://api.birbir.uz/api/frontoffice/1.3.5.0/user/profile` | User profile details |
| POST | `https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/feed` | Offer feed (main endpoint) |
| GET | `https://api.birbir.uz/api/frontoffice/1.3.5.0/user` | Current user data + Centrifugo config |
| GET | `https://api.birbir.uz/api/frontoffice/1.3.5.0/popup?positions[]=1` | Popup content |
| POST | `https://api.birbir.uz/api/frontoffice/1.3.5.0/auth/enter-by-phone` | Phone auth (requires UI flow) |
| GET | `https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/{id}` | Single offer detail |
| GET | `https://api.birbir.uz/api/frontoffice/1.3.5.0/offer/{id}/similar` | Not tested (likely 404) |
| POST | `https://api2.amplitude.com/2/httpapi` | Analytics (blocked) |
| GET | `https://file.birbir.uz/web/frontend/...` | Static files |

---

## 11. Page Assets and Third-Party Services

- **Main site**: `https://birbir.uz/` (Cloudflare-protected React SPA)
- **CSS**: `/assets/` (hashed filenames, e.g., `main.de181fc5b919e4c8f9d7.js`)
- **Fonts**: `assets/*.woff2` (606a27c..., 1ff03325b...)
- **Images**: `img.birbir.uz/i/200x200-fit/files/...` and `400x400-fit/`
- **Static**: `file.birbir.uz/web/frontend/` (phone-banner, qr-code, guard images)
- **Google Tag Manager**: `www.googletagmanager.com/gtm.js?id=GTM-PQ62VBHJ`
- **Amplitude**: `sr-client-cfg.amplitude.com/config/11821a4e9c78923d2816a71ceb1bf0f2` (blocked in DevTools)
- **Sentry**: `sentry.doska-tech.uz/api/7/envelope/` (blocked in DevTools)

---

## 12. Key Observations

1. **All endpoints require Bearer JWT** — no public/unauthenticated read endpoints exist
2. **Versioning in URL path** — `1.3.5.0` appears in every API call
3. **Error format is consistent** — `{ content: null, error: { code, message, alert } }`
4. **Success responses use `content` key** — single object or paginated `{ items, paginator }`
5. **Price types**: `1` = fixed price, `2` = negotiable ("Narx kelishiladi"), `3` = service (no price)
6. **Delivery**: BirBir delivery is a promoted feature (`has_birbir_delivery` flag in analyticsInfo)
7. **Promotions**: Feature `200` = "Promotion" (promoted listing), appears as `promotions_list: "200"`
8. **Badges**: Type `30` = "Yetkazib berish" (delivery), Type `50` = "Do'kon" (store), Type `60` = "Narx kelishiladi" (price negotiable), Type `90` = "PRO"
9. **No category tree endpoint** — categories are embedded in offer `webUri` and `titlePath` strings
10. **Token expiry is ~4 hours** — requires browser re-authentication via Cloudflare JS challenge
11. **The session cookie** uses the `j:` prefix for URL-encoded JSON, containing both `accessToken` and `refreshToken`
12. **WebSocket private channel** format reveals device UUID and user UUID structure

---

## 13. Test Methodology

- Navigation to `https://birbir.uz/uz/all` via Chrome DevTools MCP
- Waited for Cloudflare JS challenge + React SPA hydration
- Captured all network requests via `list_network_requests` (76 total requests)
- Extracted JWT from `session` cookie via `document.cookie`
- Manually decoded JWT payload using `atob()` in page console
- Tested all API endpoints via `evaluate_script` using `fetch()` with proper headers
- Tested error cases (wrong method, missing auth, non-existent endpoints)
- Extracted full offer schema from paginated feed response and single-offer detail endpoint

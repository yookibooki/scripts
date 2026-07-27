# uzum.uz Network Requests (2026-07-27)

**URL:** https://uzum.uz
**Total Requests:** 151
**Note:** uzum.uz redirected to a Yandex SmartCaptcha interstitial (olx.uz). The captured requests reflect the olx.uz page loaded by the captcha redirect.

## Notable Requests

### Redirect Chain
- Final URL landed on `www.olx.uz` (200) — olx.uz is the Uzbek marketplace sister site of uzum.uz, both under Yandex.Market/OLX Group.

### Third-Party Script Blockers
Several requests were blocked by the browser (likely by Brave's ad/tracker blocker):
- `btloader.com` (affiliate tags) — BLOCKED
- `cdn.slots.baxter.olx.org/_assets/prebid/*` — BLOCKED (prebid.js ad tech)
- `imasdk.googleapis.com/js/sdkloader/ima3.js` — BLOCKED (Google IMA video ads)
- `www.google.com/adsense/search/ads.js` — BLOCKED
- `js-agent.newrelic.com/nr-spa-1.249.0.min.js` — BLOCKED (New Relic RUM)
- `js.appboycdn.com/web-sdk/5.7/braze.min.js` — BLOCKED (Braze push/analytics)
- `tracking.olx-st.com/h/v2/it-cee/*` — BLOCKED (OLX tracking pixels)

### CDN & Static Assets
- **Font loading:** `www.olx.uz/fonts/OLXvGeomanist*.woff2` (4 font variants: Regular, Medium, Book, and additional)
- **CSS:** `cdn.slots.baxter.olx.org/olxuz/rweb/release/init.css` (304 cached)
- **JS bundles:** ~100+ chunk files under `www.olx.uz/app/static/js/` (React SPA chunks)
- **Images:** `frankfurt.apollo.olxcdn.com` CDN for category icons and banners (360x270, 360x360, 495x270, etc.)
- **Categories:** 12 category icons from `categories.olxcdn.com/assets/categories/olxuz/`

### Analytics & Ad Services (allowed)
- `securepubads.g.doubleclick.net/tag/js/gpt.js` — 200 (Google Publisher Tag)
- `www.googletagmanager.com/gtag/js` — 200
- `www.googletagmanager.com/gtm.js` — 200
- `ninja.data.olxcdn.com/beta/ninja-cee.js` — 304
- `ninja.data.olxcdn.com/config-cee-web.json` — 304
- `laquesis.data.olxcdn.com/assign` — 200

### Video/Ad SDKs
- `cdn.slots.baxter.olx.org/_assets/videojsima/` — video.js IMA player SDK (CSS + JS, 304)
# uzum.uz Live Accessibility Snapshot

**Date:** 2026-07-27
**URL:** https://uzum.uz
**Title:** Are you not a robot?
**Note:** uzum.uz redirected to a Smart Captcha challenge page (Yandex); the snapshot reflects the captcha interstitial rather than the main site content.

## Accessibility Tree

```
- main [ref=e4]:
  - generic [ref=e5]:
    - link "Yandex Cloud" [ref=e7] [cursor=pointer]:
      - /url: https://cloud.yandex.ru/
    - generic [ref=e8]:
      - generic [ref=e9]: Please confirm that you and not a robot are sending requests
      - generic [ref=e11]:
        - text: We're sorry, but it looks like requests sent from your device are automated.
        - link "Why might this happen?" [ref=e12] [cursor=pointer]:
          - /url: https://yandex.com/support/smart-captcha/problems.html?form-unique_key=9120813487753790077&form-fb-hint=2.150
    - generic [ref=e13]:
      - text: If you have any problems, please use the
      - link "feedback form" [ref=e14] [cursor=pointer]:
        - /url: https://yandex.com/support/smart-captcha/problems.html?form-unique_key=9120813487753790077&form-fb-hint=2.150
    - generic [ref=e16]: 9120813487753790077:1785178153
```

## Key Observations

- uzum.uz serves a Yandex SmartCaptcha challenge before rendering any market content.
- The page is minimal — no market listings, no navigation, no footer — just the captcha interstitial.
- The `main` landmark contains only the captcha messaging and a Yandex Cloud link.
- No interactive form elements (no CAPTCHA checkbox, no challenge widget) are exposed in the accessibility tree, which may indicate the CAPTCHA renders via an iframe or canvas not captured by the snapshot tool.
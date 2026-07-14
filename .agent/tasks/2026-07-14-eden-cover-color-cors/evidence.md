# Evidence

## AC1 — PASS

- `platform/desktop/electron/main.ts` registers only `kosmos-local-image` with `corsEnabled: true`.
- `platform/desktop/electron/main-protocols.ts` adds `Access-Control-Allow-Origin: *` only to successful validated local-image responses.
- Existing `resolveLocalImagePath` validation remains unchanged.

## AC2 — PASS

- `EverythingItemCard` adds `crossorigin="anonymous"` only for `kosmos-local-image:` sources.
- `rtk bun run visual:eden` executes an Electron assertion that loads `products/eden/icon.png` through the custom protocol, draws it to canvas, and successfully reads pixel alpha.

## AC3 — PASS

- Canvas color extraction remains active and resets on source/error changes.
- The cover overlay uses the user-provided CSS gradient with CC BY-SA 4.0 attribution.
- Component tests cover a real cover layer and centered `Без обложки` fallback.
- Visual artifact: `.tmp/visual/2026-07-14-book-cover-cors/book-cover-cors-desktop.png`.

## AC4 — PASS

- `rtk bun run --cwd products/eden test:vue` — 76 passed.
- `rtk bun run --cwd platform/desktop build:js` — passed.
- `rtk bun run visual:eden` after snapshot update — passed, including local canvas pixel read.
- `rtk bunx oxfmt --check ...` — passed.
- Independent verifier: AC1–AC4 PASS, no blocking findings.

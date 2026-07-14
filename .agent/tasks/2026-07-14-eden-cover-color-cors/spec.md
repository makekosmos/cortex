# Eden local cover color CORS

## Goal

Allow Eden to read pixels from locally served book covers so the existing canvas color extraction works instead of falling back to gray.

## Scope

- Enable CORS only for the existing `kosmos-local-image` scheme and responses.
- Request local cover images anonymously from `EverythingItemCard`.
- Keep the current local-path validation and cover fallback behavior.
- Use a CSS-only translucent cover overlay adapted from the user-provided CC BY-SA 4.0 example.

Out of scope: remote-image proxying, broader filesystem access, ARK/schema/API changes, dependencies, release work.

## Acceptance criteria

- **AC1.** `kosmos-local-image` is registered with CORS enabled and successful image responses include `Access-Control-Allow-Origin: *` without weakening path validation.
- **AC2.** Local book covers load with anonymous CORS and canvas color extraction produces a non-empty cover color; non-local covers retain their current loading behavior.
- **AC3.** The cover overlay uses the image-derived shell color and a translucent CSS spine/sheen; missing or failed covers remain a centered gray `Без обложки` fallback.
- **AC4.** Relevant unit/component/build/visual checks pass, and an independent verifier reviews the current diff.

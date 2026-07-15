# Evidence: Eden Open Library enrichment

## Result

All acceptance criteria pass. Independent read-only verification by the delegated verifier also reports PASS for AC1–AC6.

## Acceptance criteria

- **AC1 — PASS.** The renderer exposes only `lookupIsbn(isbn)` through the typed preload contract. The origin and endpoint remain main-process constants, and ISBN-13 is validated in main.
- **AC2 — PASS.** Edition and author requests stay on `https://openlibrary.org`; up to three same-origin redirects are allowed while external redirects are rejected. Requests use a 10-second timeout and 256 KiB response cap, validate JSON, map supported fields, and return `null` for 404.
- **AC3 — PASS.** The modal accepts direct or prefilled ISBN input without fetching a page. For links, `fillMissingBookMetadata` preserves source-page values and fills only missing fields from Open Library. Language codes and names normalize to the book schema's Russian select values before preview/apply.
- **AC4 — PASS.** Open Library errors are caught as best-effort failures; already extracted metadata remains available in the preview.
- **AC5 — PASS.** Main tests cover mapping, fixed URLs, same-origin redirect handling, external redirect rejection, 404 and invalid ISBN. Browser tests cover direct/prefilled ISBN lookup, source-priority merge and offline fallback.
- **AC6 — PASS.** Eden unit/browser tests, desktop build, ARK write guard, docs freshness and visual regression pass.

## Commands

- `rtk bun test platform/desktop/electron/book-metadata-fetch.test.ts platform/desktop/electron/book-metadata-open-library.test.ts` — 9 passed.
- `rtk bun run --cwd products/eden test:unit` — 97 passed, including language alias normalization.
- `rtk bun run --cwd products/eden test:vue` — 16 files / 98 tests passed, including pasted and prefilled ISBN lookup without page fetch.
- `rtk bun run --cwd platform/desktop build:js` — passed.
- `rtk bun run ark:guard:writes` — passed.
- `rtk bun run docs:sync` and `rtk bun run docs:check` — passed.
- `rtk bun run visual:eden --update-snapshots`, followed by `rtk bun run visual:eden` — passed; the flow opens the language dropdown, verifies `Английский`, and confirms imported `Русский` after apply.
- `rtk git diff --check` — passed.
- Live lookup for ISBN `9780140328721` returned title, author, cover, ISBN, page count, language, publisher and publication date.
- Live lookup for redirecting ISBN `9780374528379` returned `The Brothers Karamazov`, cover, 824 pages, language, publisher and publication date.

## Independent verification

The delegated verifier initially found that native fetch redirects could leave the fixed origin. Redirects are handled manually, capped, and accepted only when the next URL stays on `openlibrary.org`; external redirects have an adversarial regression test.

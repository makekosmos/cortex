# Evidence

## AC1

PASS. `bun run --cwd products/eden test:unit` covers tolerant reads, malformed readability, inline images, paragraph hardBreak preservation, nested lists, code fences, TipTap/Markdown conversion, invalid TipTap skip behavior, structural TipTap dirty detection, and migration skip/failure behavior. Result: 104 passed.

## AC2

PASS. `bun run --cwd products/eden test:vue` covers CM and TipTap autosave/unmount retry after `SaveEntryResult { ok:false }`, existing optimistic draft save regressions, full-entry export over summary bodies, and summary-body save rejection. Result: 86 passed.

## AC3

PASS. `bun run --cwd products/eden test:vue` covers stale save completion, journal initial-save ordering, clean refresh loading full body, dirty refresh preserving local body, and auto-migration skipping entries changed after snapshot or dirty in the open editor. Live refresh rejects older `updated_at` loads before applying to list/current entry.

## AC4

PASS. `bun run --cwd products/eden test:vue` covers API malformed body rejection, incomplete summary body rejection, and stale write rejection without `upsert_object`. `bun run ark:guard:writes` passed.

## AC5

PASS. `bun run --cwd products/eden test:unit` covers TipTap migration counts, skip reasons including unsupported invalid TipTap wrappers, failed save handling, and all-DB auto-migration tests are covered by `bun run --cwd products/eden test:vue`.

## AC6

PASS. Verification commands:

- `bun run desktop:typecheck`
- `bun run --cwd products/eden test:unit`
- `bun run --cwd products/eden test:vue`
- `bun run ark:guard:writes`
- `bunx prettier --check ...`
- `bun run docs:check`
- `git diff --check`

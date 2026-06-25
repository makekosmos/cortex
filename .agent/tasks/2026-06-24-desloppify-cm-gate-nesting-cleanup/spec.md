# Desloppify CM Gate Nesting Cleanup

## Classification

FULL_LOOP. This touches Eden editor internals, so `docs-site/apps/eden/editor.md` was loaded first.

## Goal

Remove the `DEEP_NESTING` finding in `products/eden/src/editor-cm/cmGate.ts` without changing CodeMirror-safe document coercion.

## Change

- Extracted inline paragraph/heading child coercion into `coerceInlineChildToCmSafe`.
- Kept text-node mark sanitization and plain-text fallback behavior unchanged.

## Verification

- PASS: `rtk err bun test products/eden/tests/cmGate.test.ts`
- PASS: `rtk err bun run --cwd platform/desktop build:extension eden`
- PASS: `rtk err bun run --cwd platform/desktop typecheck`
- PASS/expected failure status: `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-cm-gate-nesting-cleanup.json"`

The scan exits 1 because repository findings remain, but the targeted deep-nesting finding disappeared.

# Evidence

- Baseline scan: score 9, findings 360, high 203, medium 113, low 44.
- After scan: score 9, findings 342, high 185, medium 113, low 44.
- `products/eden/src/lib/typedNotes.ts` `DEAD_EXPORT` findings: 18 -> 0.
- `platform/desktop` typecheck passed.
- `products/eden` has no `build` script, so the requested build check could not be run as written.
- Desloppify scan produced the expected nonzero exit code while still emitting JSON.
- The edit is modifier-only and ASCII-only.

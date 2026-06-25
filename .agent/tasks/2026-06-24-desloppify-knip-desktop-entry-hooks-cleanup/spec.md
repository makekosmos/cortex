# Knip desktop entry hook cleanup

Goal: reduce false-positive `DEAD_DEPENDENCY` and `DEAD_FILE` findings for
`platform/desktop` by registering real desktop entry points in `knip.json`
without widening the project glob.

Scope:

- keep `workspaces["platform/desktop"].project` as `electron/**/*.ts`
- keep `electron/main.ts`
- add `build/afterPack.cjs`
- add `electron/preload.ts`
- add `electron/extension-preload.ts`

Success:

- the targeted Knip scan no longer reports the hook/file false positives
- the full desloppify scan shows a lower finding count than the baseline

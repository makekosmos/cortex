# Dead Files Delete Cleanup

## Baseline

- File: `.agent/tasks/2026-06-24-desloppify-local-type-deexports-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `160`
- Severity: `HIGH 49`, `MEDIUM 69`, `LOW 42`
- `DEAD_FILE`: `3`

## Change

- Deleted unused `products/delphi/src/services/sync/hlc.ts`.
- Deleted unused `products/delphi/src/services/sync/peer-protocol.ts`.
- Deleted unused `site/src/components/DesktopMockup.vue`.
- Removed the stale commented `DesktopMockup` import from `site/src/App.vue`.

## Result

- File: `.agent/tasks/2026-06-24-desloppify-dead-files-delete-cleanup/desloppify-after.json`
- Score: `11`
- Findings: `157`
- Severity: `HIGH 46`, `MEDIUM 69`, `LOW 42`
- `DEAD_FILE`: `0`

## Checks

- `rtk grep "services/sync/hlc|services/sync/peer-protocol|from .*hlc|from .*peer-protocol|DesktopMockup|\bHLC\b|computeMeshId|createPeerHello|PeerHello" products/delphi site/src -n`
- `rtk test bun run --cwd products/delphi test:vue`
- `rtk test bun run ark:guard:writes`
- mojibake scan on `site/src/App.vue`
- `rtk test bun run --cwd site build` (blocked before compile: `bun: command not found: vue-tsc`)
- `rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-dead-files-delete.json"`

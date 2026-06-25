# Raycast view-host dead export cleanup

Scope: `platform/desktop/electron/raycast/view-host.ts`

Goal: remove safe `DEAD_EXPORT` findings by de-exporting internal Raycast view-host helpers without changing runtime behavior.

Targets:

- `openRaycastElementView`
- `getRaycastSnapshot`

Verification:

- `bun run --cwd platform/desktop typecheck`
- `knip` export scan
- `desloppify scan --json`

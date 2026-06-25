# Evidence

- Baseline copied to `baseline/desloppify-before.json`.
- `platform/desktop/electron/raycast/manifest.ts` now keeps only the public parser export and `RaycastPackageManifest` export.
- `rtk err bun run --cwd platform/desktop typecheck` passed.
- No targeted Raycast manifest test file was present in the repo.
- `rtk bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress` no longer reported the five target type exports from `manifest.ts`; `parseRaycastPackageManifest` remains exported by design.
- Desloppify scan delta:
  - total findings: 315 -> 310
  - high findings: 158 -> 153
  - target DEAD_EXPORT findings in `manifest.ts`: 5 -> 0
  - remaining manifest DEAD_EXPORT: `parseRaycastPackageManifest`

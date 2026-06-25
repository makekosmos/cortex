# Evidence

Knip was updated with a minimal `platform/desktop` workspace entry/project
override in `knip.json`.

Result:

- score: 9 -> 9
- findings: 332 -> 336
- high: 175 -> 179
- medium: 113 -> 113
- low: 44 -> 44
- DEAD_FILE: 124 -> 69

Target Electron files no longer reported as `DEAD_FILE`:

- `platform/desktop/electron/main-commands.ts`
- `platform/desktop/electron/extension-host.ts`
- `platform/desktop/electron/extension-installer.ts`
- `platform/desktop/electron/extension-manifest.ts`

Remaining Electron `DEAD_FILE` findings are test and preload files under
`platform/desktop/electron/`, including `preload.ts`, `extension-preload.ts`,
and the Electron test files.

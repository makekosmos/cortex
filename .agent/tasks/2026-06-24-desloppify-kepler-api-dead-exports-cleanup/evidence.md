# Evidence

`SemVer` and `parseSemver` were de-exported from `platform/desktop/electron/kepler-api.ts`.

Scan comparison:

- DEAD_EXPORT findings for `kepler-api.ts`: 2 -> 0
- repo high findings: 162 -> 160
- repo findings: 319 -> 317
- repo score: 9 -> 9

Verification:

- `bun run --cwd platform/desktop typecheck`
- `bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress`
- `bunx desloppify scan --json .`

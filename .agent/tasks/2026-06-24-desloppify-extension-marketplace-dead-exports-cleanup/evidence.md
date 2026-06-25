# Evidence

Baseline copy was saved before the post-change scan at `.agent/tasks/2026-06-24-desloppify-extension-marketplace-dead-exports-cleanup/baseline/desloppify-before.json`.

Checks:

- `rtk err bun run --cwd platform/desktop typecheck` passed.
- `rtk test bun test tests/unit/extension-update-plan.test.ts` passed.
- `rtk bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress` no longer reported `fetchCatalog`, `installFromUrl`, or `autoUpdateExtensionsOnce`.
- `rtk proxy cmd /c "set PATH=%CD%\\.tmp\\bin;%PATH%&& bunx desloppify scan --json . > .tmp\\desloppify-after-extension-marketplace-dead-exports-cleanup.json"` exited 1, then the JSON was copied to `.agent/tasks/2026-06-24-desloppify-extension-marketplace-dead-exports-cleanup/desloppify-after.json`.

Scan delta:

- total findings: 310 -> 307
- high findings: 153 -> 150
- medium findings: 113 -> 113
- low findings: 44 -> 44
- target runtime dead exports: 3 -> 0
- shared type exports kept: `CatalogExtension`, `Catalog`

Mojibake check:

- no new non-ASCII text was introduced by the edit; the file still contains pre-existing non-ASCII comments unchanged.

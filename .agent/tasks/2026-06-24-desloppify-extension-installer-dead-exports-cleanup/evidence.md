# Evidence

Changed only `platform/desktop/electron/extension-installer.ts`.

Removed `export` from six internal helpers:

- `userExtensionsRoot`
- `extensionsBackupsRoot`
- `previewKext`
- `previewDir`
- `backupExtension`
- `repoDevExtensionsRoot`

Results:

- `bun run --cwd platform/desktop typecheck` passed.
- `bun test platform/desktop/electron/extension-installer.test.ts` failed on an unrelated assertion in `extension-host.ts` that expects `if (!app.isPackaged) {`.
- `bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress` no longer reported exports from `extension-installer.ts`.
- `desloppify` scan for `extension-installer.ts` dropped from 7 findings with 6 DEAD_EXPORTs to 1 remaining finding and 0 DEAD_EXPORTs.

Kept exported:

- none in this file.

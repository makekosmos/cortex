# Evidence

- Baseline copied from `.agent/tasks/2026-06-24-desloppify-extension-marketplace-dead-exports-cleanup/desloppify-after.json` to `baseline/desloppify-before.json`.
- File changed: `platform/desktop/electron/extension-manifest.ts`.
- Export cleanup: removed `export` from `ExtensionRootEntry`, `resolveExtensionRootEntries`, and `resolveEntryHtml`.
- Kept public exports untouched: `readDevModeSetting`, `resolveExtensionRoots`, `userExtensionsRoot`, `extensionIconDataUri`, `ExtensionKind`, `KextManifestCommand`.

Scan delta:

- Total findings: 307 -> 304
- High findings: 150 -> 147
- Medium findings: 113 -> 113
- Low findings: 44 -> 44
- `DEAD_EXPORT` findings in this file: 9 -> 6
- Removed `DEAD_EXPORT` messages:
  - `Unused export: ExtensionRootEntry`
  - `Unused export: resolveExtensionRootEntries`
  - `Unused export: resolveEntryHtml`

Verification:

- `rtk err bun run --cwd platform/desktop typecheck` passed.
- `rtk bun test platform/desktop/electron/extension-installer.test.ts` failed on an unrelated stale assertion in `extension-host.ts` that expects `if (!app.isPackaged) {`.
- `rtk bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress` confirmed the three target exports no longer appear in the export report; documented exports remain listed.
- `rtk proxy cmd /c "set PATH=%CD%\\.tmp\\bin;%PATH%&& bunx desloppify scan --json . > .tmp\\desloppify-after-extension-manifest-internal-dead-exports-cleanup.json"` returned exit code 1, as expected with remaining findings.

Mojibake check: the edit was modifier-only and ASCII-only; no non-ASCII text was introduced.

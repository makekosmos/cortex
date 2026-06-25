# Evidence

Edit stayed inside `platform/desktop/electron/extension-host.ts`.

Removed the three requested non-contract re-exports and made `assertExtensionSenderHostPermission` internal again.

Verification:

- `rtk err bun run --cwd platform/desktop typecheck` passed.
- `rtk bun test platform/desktop/electron/extension-installer.test.ts` failed on the known stale source-text assertion that still expects the old `extension-host.ts` contents.
- `rtk bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress` still reports only the documented/public exports in `extension-host.ts`, not the removed dead exports.
- `rtk proxy cmd /c "set PATH=%CD%\\.tmp\\bin;%PATH%&& bunx desloppify scan --json . > .tmp\\desloppify-after-extension-host-noncontract-dead-exports-cleanup.json"` produced the after scan used for comparison.

Scan delta for `extension-host.ts`:

- before: 8 findings
- after: 4 findings
- removed: `userExtensionsRoot`, `assertExtensionSenderHostPermission`, `ExtensionKind`, `KextManifestCommand`
- kept: `LARGE_FILE`, `extensionIconDataUri`, `listExtensions`, `loadExtensionManifest`

# extension-host non-contract export cleanup

Scope is limited to `platform/desktop/electron/extension-host.ts`.

Remove only these dead non-contract exports:

- `userExtensionsRoot` from the `export { ... } from "./extension-manifest"` block
- `ExtensionKind` and `KextManifestCommand` from the `export type { ... } from "./extension-manifest"` block
- `export` from `assertExtensionSenderHostPermission`

Keep documented/public host exports in place:

- `extensionIconDataUri`
- `listExtensions`
- `loadExtensionManifest`
- other existing host API exports

Do not address `LARGE_FILE` in this slice.

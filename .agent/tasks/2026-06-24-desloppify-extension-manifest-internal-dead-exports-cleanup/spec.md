# desloppify extension-manifest internal export cleanup

Scope: `platform/desktop/electron/extension-manifest.ts`

Goal: remove safe internal `DEAD_EXPORT` surface without changing runtime behavior or the public extension-host contract.

Kept exported: `readDevModeSetting`, `resolveExtensionRoots`, `userExtensionsRoot`, `extensionIconDataUri`, `ExtensionKind`, `KextManifestCommand`, and other manifest/schema public exports.

Targets de-exported in this slice: `ExtensionRootEntry`, `resolveExtensionRootEntries`, `resolveEntryHtml`.

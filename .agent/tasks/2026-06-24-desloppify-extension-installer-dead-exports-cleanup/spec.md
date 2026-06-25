# Desloppify extension-installer dead exports

Scope: `platform/desktop/electron/extension-installer.ts`

Goal:

- Remove `export` from internal helpers that have no external callers.
- Keep runtime behavior unchanged.

Targets reviewed:

- `userExtensionsRoot`
- `extensionsBackupsRoot`
- `previewKext`
- `previewDir`
- `backupExtension`
- `repoDevExtensionsRoot`

Constraints:

- Do not edit other files.
- Do not remove declarations.
- Do not change tests.

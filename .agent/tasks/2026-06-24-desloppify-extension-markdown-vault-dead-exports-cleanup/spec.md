# Extension Markdown Vault Dead Export Cleanup

Scope: `platform/desktop/electron/extension-markdown-vault.ts` only.

Goal: remove safe dead exports without changing runtime behavior.

Safe names removed from the public surface:

- `MarkdownVaultTextFile`
- `MarkdownVaultImageFile`
- `isIgnoredVaultDir`
- `readImageDimensions`

Kept exports:

- `MarkdownVaultOpenResult`
- `MarkdownVaultExportFile`
- `MARKDOWN_FILE_MAX_BYTES`
- `safeMarkdownDefaultName`
- `safeVaultOutputPath`
- `resolveMarkdownVaultSourcePath`
- `scanMarkdownVault`

Validation:

- typecheck passed
- targeted test passed
- knip no longer reports this file for `DEAD_EXPORT`
- desloppify scan reduced target-file `DEAD_EXPORT` findings from 4 to 0

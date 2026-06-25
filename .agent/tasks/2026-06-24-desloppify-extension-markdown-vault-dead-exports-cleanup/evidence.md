# Evidence

Baseline scan: score 9, 323 findings, 166 high, 113 medium, 44 low.

After scan: score 9, 319 findings, 162 high, 113 medium, 44 low.

Target file `DEAD_EXPORT` findings moved from 4 to 0.

Removed names:

- `MarkdownVaultTextFile`
- `MarkdownVaultImageFile`
- `isIgnoredVaultDir`
- `readImageDimensions`

Kept names:

- `MarkdownVaultOpenResult`
- `MarkdownVaultExportFile`
- `MARKDOWN_FILE_MAX_BYTES`
- `safeMarkdownDefaultName`
- `safeVaultOutputPath`
- `resolveMarkdownVaultSourcePath`
- `scanMarkdownVault`

Checks:

- `bun run --cwd platform/desktop typecheck`
- `bun test platform/desktop/electron/extension-markdown-vault.test.ts`
- `bunx knip -c knip.json --workspace platform/desktop --include exports --reporter compact --no-progress`
- `bunx desloppify scan --json .`

# Evidence

## Scan Delta

Full scan artifacts:

- Before: `baseline/desloppify-before.json`
- After: `desloppify-after.json`

Summary:

| Metric         | Before | After | Delta |
| -------------- | -----: | ----: | ----: |
| Score          |      0 |     0 |     0 |
| Total findings |    451 |   444 |    -7 |
| Critical       |      0 |     0 |     0 |
| High           |    250 |   245 |    -5 |
| Medium         |    130 |   128 |    -2 |
| Low            |     71 |    71 |     0 |

Scoped to `products/eden/src/lib/markdownFrontmatter.ts`:

- Before: `RETURN_UNDEFINED`, 3 `CATCH_WRAP_NO_CAUSE`, `LARGE_FILE`, and 4 `DEAD_EXPORT` findings.
- After: only `RETURN_UNDEFINED` and 1 `CATCH_WRAP_NO_CAUSE` remain.

The file changed by 1 insertion and 539 deletions, and now has 252 lines.

## Change

Removed the unused strict Markdown-frontmatter import parser:

- `parseEntryMarkdownDocument`
- `ParseEntryMarkdownDocumentOptions`
- `ParsedEntryMarkdownDocument`
- `YamlSubsetParser` and its parser-only helpers

Kept the active export API:

- `FrontmatterValue`
- `buildEntryMarkdownDocument`

`EntryMarkdownFrontmatter` is now internal because it is only used inside the serializer implementation.

## Reference Check

Command:

```powershell
rtk rg "parseEntryMarkdownDocument|ParsedEntryMarkdownDocument|ParseEntryMarkdownDocumentOptions|EntryMarkdownFrontmatter|YamlSubsetParser|buildEntryMarkdownDocument|FrontmatterValue" products/eden/src products/eden/tests tests
```

Result:

- No references to the removed parser API outside `markdownFrontmatter.ts`.
- Active references remain in `products/eden/src/lib/obsidianVault.ts`, `products/eden/src/components/settings/ExportSettings.vue`, and `products/eden/tests/obsidianVault.test.ts`.

## Verification

```powershell
rtk err bun run --cwd platform/desktop typecheck
rtk err bun run --cwd platform/desktop build:extension eden
rtk test bun test products/eden/tests/obsidianVault.test.ts
rtk proxy cmd /c "set PATH=%CD%\.tmp\bin;%PATH%&& bunx desloppify scan --json . > .tmp\desloppify-after-markdown-frontmatter-parser-cleanup.json"
```

Results:

- Typecheck: PASS
- Eden extension build: PASS
- `obsidianVault.test.ts`: PASS, 12 tests / 51 expects
- Desloppify scan: JSON produced; command exits 1 because findings remain in the repo.

## Notes

This intentionally does not modify the active Obsidian loose parser or Markdown export serialization semantics. The remaining two scoped findings are in active serializer code and should be handled separately.

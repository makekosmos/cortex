# 2026-06-24 Desloppify Markdown Frontmatter Active Cleanup

## Classification

FULL_LOOP.

## Goal

Remove the remaining local `desloppify` findings from Eden's active Markdown
frontmatter serializer without changing Markdown export behavior.

## Context

After the strict parser removal, `products/eden/src/lib/markdownFrontmatter.ts`
still had two findings:

- `RETURN_UNDEFINED` in `normalizeFrontmatterValue`
- `CATCH_WRAP_NO_CAUSE` in `parseEntryHeaderProps`

Both are in the active `buildEntryMarkdownDocument` path used by Eden export and
Obsidian integration.

## Scope

In scope:

- `products/eden/src/lib/markdownFrontmatter.ts`
- This task's evidence files

Out of scope:

- Markdown export format changes
- Obsidian import parsing behavior
- ARK data/write paths

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after full-scan JSON outputs and a summary of the
desloppify delta.

**AC2. Local findings are removed.**
`markdownFrontmatter.ts` has no scoped findings in the after scan.

**AC3. Behavior is preserved.**
`parseEntryHeaderProps` still reports the same invalid JSON message while
preserving the original error as `cause`; `normalizeFrontmatterValue` still
returns `undefined` for `undefined` input.

**AC4. Relevant checks pass.**
Typecheck, Eden build, focused Obsidian/export tests, and the full scan run are
recorded.

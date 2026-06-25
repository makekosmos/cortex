# 2026-06-24 Desloppify Markdown Frontmatter Parser Cleanup

## Classification

FULL_LOOP.

## Goal

Remove the unused strict Markdown-frontmatter import parser from Eden while
preserving the active Markdown export path.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 451 total
- Severity: critical 0, high 250, medium 130, low 71

`buildEntryMarkdownDocument` and `FrontmatterValue` are the active public API
used by Eden settings and Obsidian import/export. `parseEntryMarkdownDocument`
and its strict YAML parser helpers have no callers and no focused tests.

## Scope

In scope:

- `products/eden/src/lib/markdownFrontmatter.ts`
- This task's evidence files

Out of scope:

- Obsidian loose parser behavior
- Markdown export serialization behavior
- Eden import/export UI flow
- ARK data/write paths

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Unused strict parser API is removed.**
Reference checks prove `parseEntryMarkdownDocument` and related parser result
types have no callers, and scoped `DEAD_EXPORT` findings disappear.

**AC3. Active export API remains available.**
`buildEntryMarkdownDocument` and `FrontmatterValue` remain exported and current
Obsidian/export tests continue to pass.

**AC4. Relevant checks pass.**
Eden build, typecheck, and focused Obsidian/export tests pass, or any unrelated
blocker is documented with exact command output.

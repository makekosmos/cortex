# 2026-06-24 Desloppify Akasha Obsidian Dead Exports

## Classification

FULL_LOOP.

## Goal

Remove a small set of confirmed unused Akasha/Obsidian exports while preserving
current EPUB parsing and Eden Obsidian import/export behavior.

## Context

Current scan after prior cleanup:

- Score: 0
- Findings: 460 total
- Severity: critical 0, high 259, medium 130, low 71

The active Akasha reader/library code uses `readEpubBytes`; no current code
imports `readEpubFile`. Eden Obsidian import/export uses runtime functions and
public draft/result types, while two helper types are local implementation
details.

## Scope

In scope:

- `incubator/akasha/src/lib/epub.ts`
- `products/eden/src/lib/obsidianVault.ts`
- This task's evidence files

Out of scope:

- Markdown frontmatter parser cleanup
- EPUB parser behavior changes
- Eden import/export UI flow changes
- ARK data/write paths

## Acceptance Criteria

**AC1. Metrics are reproducible.**
The task contains before/after `desloppify scan --json .` outputs and a summary
of total findings and severity counts.

**AC2. Scoped dead exports are removed.**
Reference checks prove `readEpubFile` has no callers and scoped helper types
have no external imports, and the scoped `DEAD_EXPORT` findings disappear.

**AC3. Active parser APIs remain available.**
`readEpubBytes` and Eden Obsidian import/export functions remain exported.

**AC4. Relevant checks pass.**
Akasha/Eden builds and focused parser/import-export tests pass, or any unrelated
blocker is documented with exact command output.

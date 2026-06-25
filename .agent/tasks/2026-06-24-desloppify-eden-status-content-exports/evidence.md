# Evidence

## Commands

- `rg TASK_STATUSES|TASK_STATUS_DEFAULT|TASK_STATUS_LABELS|getStatusCategory|TaskStatusCategory|MARKDOWN_CONTENT_VERSION|GENERATED_UNTITLED_ENTRY_PREFIX|UNTITLED_ENTRY_TITLE_FLAG|isGeneratedUntitledEntryTitle products/eden platform tests packages`
- `bun run --cwd platform/desktop typecheck`
- `bun run --cwd platform/desktop build:extension eden`
- `bun test products/eden/tests/content.test.ts`
- `bunx vitest run --browser=chromium tests/components/EntryTitle.spec.ts` from `products/eden`
- `bunx desloppify scan --json . > .agent/tasks/2026-06-24-desloppify-eden-status-content-exports/desloppify-after.json`

## Results

- References check: PASS.
  - `TASK_STATUSES` and `TASK_STATUS_DEFAULT` are only needed locally by
    `TaskStatus`/`normalizeStatus`.
  - `TASK_STATUS_LABELS`, `TaskStatusCategory`, and `getStatusCategory` had no
    callers and were removed.
  - `MARKDOWN_CONTENT_VERSION` is only used locally by `MarkdownContent` and
    `writeEntryMarkdown`.
  - `UNTITLED_ENTRY_TITLE_FLAG` and `isGeneratedUntitledEntryTitle` are only
    used inside `entryTitles.ts`.
  - `GENERATED_UNTITLED_ENTRY_PREFIX` had no callers and was removed.
- `typecheck`: PASS.
- `build:extension eden`: PASS.
- `content.test.ts`: PASS, 4 tests.
- `EntryTitle.spec.ts`: PASS, 5 tests.
  - Vite printed an existing optimizeDeps warning for `@tiptap/pm/state`, but
    the command exited 0.
- Baseline scan: score 0, 488 findings; severity critical 0, high 287,
  medium 130, low 71.
- Final scan: score 0, 479 findings; severity critical 0, high 278,
  medium 130, low 71.
- Scoped `DEAD_EXPORT`: 9 -> 0.

## AC Verdicts

- AC1: PASS. Baseline and final JSON are saved.
- AC2: PASS. Scoped Eden-local exports are removed.
- AC3: PASS. Runtime behavior is covered by build and focused tests.
- AC4: PASS. Relevant checks passed.

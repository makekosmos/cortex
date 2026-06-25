# desloppify small export de-export cleanup

Baseline:

- `.agent/tasks/2026-06-24-desloppify-eden-delphi-dead-files-cleanup/desloppify-after.json`
- score 9, total 222, HIGH 73, MEDIUM 106, LOW 43

After:

- `.agent/tasks/2026-06-24-desloppify-small-export-deexport-cleanup/desloppify-after.json`
- score 9, total 220, HIGH 71, MEDIUM 106, LOW 43

Rule delta:

- `DEAD_EXPORT`: 38 -> 36

Changes:

- `CatalogExtension` is now file-private in `extension-marketplace.ts`.
- `cancelDictation` is now file-private in `dictation-pill.ts`.

Checks:

- `bun run --cwd platform/desktop typecheck` passed.
- targeted grep confirmed no exported `CatalogExtension` / `cancelDictation` remains.
- mojibake grep over touched files passed.
- `desloppify scan --json` produced the after JSON; exit 1 is expected while findings remain.

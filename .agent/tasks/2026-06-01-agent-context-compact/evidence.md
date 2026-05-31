# Evidence — Agent context compact generation

## Generated Context Audit

Command:

```powershell
rg --files -g "AGENTS.md" -g "CLAUDE.md"
```

Result:

| File                        | Classification                  | Lines |  Bytes | Verdict |
| --------------------------- | ------------------------------- | ----: | -----: | ------- |
| `AGENTS.md`                 | root generated boot context     |   190 | 16,213 | PASS    |
| `CLAUDE.md`                 | root generated boot context     |   186 | 15,755 | PASS    |
| `mobile/delphi/AGENTS.md`   | per-area generated boot context |    53 |  3,278 | PASS    |
| `crates/ark-core/AGENTS.md` | per-area generated boot context |    59 |  3,648 | PASS    |

## AC Status

- AC1: PASS — every returned `AGENTS.md` / `CLAUDE.md` is listed above with count and classification.
- AC2: PASS — root `AGENTS.md` and `CLAUDE.md` are under 220 lines / 32 KB.
- AC3: PASS — per-area `AGENTS.md` files are under 180 lines / 24 KB.
- AC4: PASS — `scripts/sync-agents-docs.mjs` enforces budgets in `GENERATED_CONTEXT_BUDGETS` during `write(...)`.
- AC5: PASS — per-area files now contain scope, source-doc pointers, local invariants, commands, and compact repo TL;DR instead of full app/package docs and full `forbidden.md`.
- AC6: PASS — `docs-site/agents/docs-maintenance.md` documents budgets, compact-context rules, and what not to inline.
- AC7: PASS — verification commands below passed.

## Verification Commands

```powershell
bun run docs:sync
bun run docs:check
bun run lint -- scripts/sync-agents-docs.mjs
```

All passed.

## Size Impact

`git diff --stat` for touched generated/docs files:

```text
8 files changed, 386 insertions(+), 1909 deletions(-)
```

The main reductions are:

- Root `AGENTS.md`: from about 1,033 lines / 107 KB to 190 lines / 16 KB.
- `mobile/delphi/AGENTS.md`: from 463 lines / 53 KB to 53 lines / 3 KB.
- `crates/ark-core/AGENTS.md`: from 452 lines / 50 KB to 59 lines / 4 KB.

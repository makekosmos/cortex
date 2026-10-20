# ARK exocortex completion run

- [x] Read the Principles section of pstack:poteto-mode.
- [x] Phase A — Frame: completion predicate is every requirement in `/tmp/cortex-thermo-ark-exocortex/05_remaining_plan.md`, with successful final exact-SHA CI, independent review, and Linear reconciliation. Scope is the remaining Stage 1 evidence, Stage 2 ownership cutovers, Stage 3 structural findings, Stage 4 recovery criteria, and Stage 5 acceptance. Known gaps include the typed-operation inventory still exposing SQLite to Core, remaining sync and transport ownership, recovery platform evidence, and final exact-SHA checks.
- [x] Phase B — Design the workflow: capture current ownership and verification baseline, order by risk and semantic dependencies, and assign independent read-only audits.
- [ ] Phase C — Run the loop: verify one ownership cutover or recovery criterion at a time, delete superseded paths, run focused checks, and commit each green unit.
- [ ] Phase D — Keep the audit trail: append evidence-backed decisions and audit each row against committed artifacts.
- [ ] Phase E — Verify and hand back: run the complete local suite and platform matrix at the final SHA, obtain independent review, reconcile KOS-424/KOS-425/KOS-428/KOS-412, and prove every plan predicate.

## Completion units

- [x] Establish the exact current baseline and determine the prior full-gate failure. The full check passed and was cached for disk tree `6f6c295e4e988234be7f7aa638a72b4b270fd2e4`; the apparent shell failure came from assigning the result to zsh's read-only `status` variable. The current tree includes a new audit file, so the final gate must rerun after source work.
- [x] Cut local usage writes, Clear All, and Delete Trashed over to Store-owned transaction methods and actor tasks; delete the superseded Core command types in the same change. Focused behavior tests, formatting, Clippy, writer census, and affected checks passed; committed as `76053d6e`.
- [x] Cut object-link upsert, delete, and replace over to Store writer operations and actor tasks; delete the superseded Core commands in the same change. Seven affected service tests, Clippy, writer census, source-size, and affected checks passed. The debug-only test adapter adds one explicitly allowlisted blocking bridge; production actor writes remain Store-owned. Committed as `843bf36b`.
- [x] Cut external-reference upserts over to Store writer operations and actor tasks; delete the superseded Core command. Preserve its transaction and wire behavior; external-ref and object-link tests passed 7/7, Clippy passed, writer census is 81 counts, formatting and source-size passed, and the affected pre-commit check passed. Shared debug writer test support now has a neutral module owner. Committed as `8a3ef4b5`.
- [ ] Complete Stage 1 root-cause and exact-SHA Windows evidence.
- [ ] Move every remaining SQLite operation, schema, query, maintenance path, and transaction owner out of `ark-core`; retain the single database and transaction boundary.
- [ ] Inventory note: the storage-boundary checker currently reports 1,140 findings, including tests and dependency configuration, so this is a signal of remaining scope rather than a count of independent production migrations.
- [ ] Finish sync-session and transport ownership in `ark-sync` and `ark-net`.
- [ ] Close `partN`, optional-port, and string-error findings without broad size-limit exceptions.
- [ ] Map each KOS-428 recovery criterion to a passing test and supported-platform evidence.
- [ ] Update architecture/build/recovery documentation from verified behavior.
- [ ] Run exact-SHA desktop/mobile/Windows CI and independent final review.
- [ ] Reconcile Linear only after the acceptance evidence is complete.

# KOS-424 task-merge application port extraction

## Refactoring playbook

- [x] Pin the behavior contract first. Run the **how** skill over the affected subsystem to learn the contract, then write a characterization test, snapshot, or equivalence harness that captures current behavior before any structure moves. If the area has no coverage, write the pin before touching structure. Type check and lint are not a pin.
- [x] Name the structure the code is missing per **principle-model-the-domain**. Boring code stays when the shape is already clear and local. The reshape must delete branches or invalid states, not add indirection.
- [x] Name the target shape. State what the module layout, types, and call graph should be if built today (**principle-foundational-thinking**, **principle-redesign-from-first-principles**). If the target crosses a function boundary, run the **architect** skill for parallel design exploration of the shape before the move.
- [x] Subtract before you add. Delete dead code, collapse one-caller wrappers, drop redundant validators, and remove orphan references before introducing the new shape (**principle-subtract-before-you-add**). The smallest change that reaches the target shape ships (**principle-laziness-protocol**). A speculative cleanup that "might help" gets reverted.
- [ ] Move in small behavior-preserving steps, each keeping the pin green. For API reshapes, migrate every caller and delete the old API in the same wave (**principle-migrate-callers-then-delete-legacy-apis**). No compatibility shims, no parallel old-and-new paths. Spot-check every rename against the actual files. Renames silently miss usages in strings, prose, and back-references. Delegate the mechanical edits to a subagent using your configured refactoring model (default in poteto-mode's Models section) with a specific scope (file paths, the names being moved, the behavior to hold).

`skip: Keep the existing public sync_storage paths as a reviewed facade in this unit. Migrate the many internal and test callers after the larger sync contract extraction, then remove the facade in that same unit.`
- [x] Prove behavior is unchanged on the real artifact, not "it compiles" (**principle-prove-it-works**). For larger reshapes, run an equivalence check: a script that diffs old-vs-new outputs, a recorded baseline replayed against the new code, or a smoke run on the matching surface via the relevant driver skill.
- [x] Confirm the change is worth keeping. The success measure is reduced reader load (**principle-minimize-reader-load**). If the diff does not lower reader load somewhere, revert it.
- [ ] Rebase into small ordered commits. A subtraction commit, then the reshape, then any follow-on cleanup. Shape them with the **sequence-verifiable-units** principle skill, so each behavior-preserving slice stays green before the next. Run **Opening a PR**.

`skip: This is one verified commit in the still-active KOS-424 stack. Defer publication until the crate boundaries are complete.`

## Extraction unit

- [x] Ground the task-merge port in the direct authenticated sync path and existing SQLite adapter.
- [x] Compare a focused application-port crate against extracting all `application/` and the shared type DTOs first.
- [x] Move the policy identity and task-merge storage port without changing field names, serde behavior, results, or protocol messages.
- [x] Keep existing `ark-core::application::sync_storage` paths as a public facade while current callers use them.
- [x] Run `cargo test -p ark-application`, the direct authenticated sync behavior pin, and the full serial ark-core suite (521 passed).
- [x] Run strict Clippy, format, dependency-tree, diff, and source-size gates.
- [ ] Commit this unit and keep KOS-424 open for the remaining application, store, sync, and net boundaries.

## Throughput checkpoint

Use at most two read-only GPT-6-Luna reviewers. Run heavyweight Cargo checks serially. Commit each verified boundary separately.

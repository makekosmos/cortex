# KOS-412 full epic delivery

## Phase A: Frame

- [x] Read the Principles section of the `poteto-mode` skill.
- [x] Define completion as: every KOS-413…KOS-429 acceptance criterion is implemented and verified against current source/runtime; all 17 child issues are Done in Linear; KOS-412 is Done; the root build, test, lint, format, source-size, and dependency gates pass for the final tree.
- [x] Count the remaining work from live Linear: KOS-413…421 Done (9); KOS-422 and KOS-429 In Progress (2); KOS-423…428 Backlog (6). Eight child issues remain open.
- [x] Set rigor high because the remaining work affects replicated personal data, access control, backups, SQLite durability, and Cargo dependency boundaries.
- [x] Record current baseline: `f620906aee3d974db2c5922a93f2ace7a2809676`; current source-size gate passes.
- [x] Confirm clean baseline: `cargo nextest run -p ark-core --no-fail-fast` passed (655 passed, 1 skipped), and `node scripts/check-source-size.mjs` passed.

## Phase B: Design the workflow

- [x] Use current Linear dependency edges, not the stale sequencing in the pasted plan.
- [x] Reconcile the pasted plan: census gate, `DbHandle`, bounded `submit`, and the `core.task` caller are already implemented; do not repeat them.
- [x] Sequence remaining work to establish device admission first, then selective policy/eviction, merge and app permissions, blob contract and recovery, and finally crate extraction plus CI graph gates.
- [x] Confirm whether the custom WebSocket relay can satisfy authenticated peer-addressed registry delivery; current room transport is broadcast-only, so it remains fail-closed pending relay-server protocol support.
- [x] Define an acceptance test or executable gate for each remaining issue before implementing it.
- [x] Reuse the accepted KOS-429 signed-registry and `ReadyPeer` design; inspect relay state and transport ownership before editing. No second arena is needed for that settled contract.
- [x] Compare two source-grounded KOS-429 fanout designs; select a per-peer admission reducer plus a service-owned committed-delta callback to avoid a second readiness authority. Record decision and transport acceptance evidence in `.audit/kos429-design.md`.

## Phase C: Run the loop

- [ ] Finish KOS-429: verify direct, Iroh, and custom relay behavior; complete revocation fanout, reconnect control-only catch-up, and queued-frame rejection; run production SQLite and transport tests; commit; update Linear.
  - [ ] Iroh live registry update sends only to registry-ready sessions bound to the active transport endpoint and admission session.
  - [ ] Iroh applies signed live registry updates, fans verified events out, and revokes every currently connected peer named in newly applied disable events.
  - [ ] Iroh queued data is rejected after revoke; reconnect still exchanges registry history before any user-data frame.
  - [ ] Custom WebSocket relay authenticates registry admission with the pinned Iroh identity challenge or remains explicitly unusable before any user data; never trust a room-broadcast device ID alone.
  - [ ] Multi-peer transport tests prove rename propagation, offline disable catch-up, replay idempotency, and no data after disable.
- [ ] Finish KOS-422: authenticated receiver-owned per-device policy exchange; bind snapshot/feed delivery, terminal pages, receipts, and applied ACKs to revision plus digest; durable residency, safe local-only eviction, pending-edit retention, and same-revision rehydration; verify multi-peer SQLite scenarios; commit; update Linear.
- [ ] Finish KOS-423: test merge alternatives on real domain fixtures; retain concurrent scalar/text variants and common ancestry; define optimistic AI/human resolution contract; commit evidence and update Linear.
- [ ] Finish KOS-426: define app grants independently of device sync policy; enforce on Engine reads/writes; implement approximate scoped volume preview and test revoke/stale-grant behavior; commit and update Linear.
- [ ] Finish KOS-427: specify metadata/content identity, hash, ownership, independent selection, and missing-content semantics without adding a blob service; verify contract examples; commit and update Linear.
- [ ] Finish KOS-428: decide and document SQLite durability from measured platform evidence; add manifest-based backup/restore, recovery identity/epoch, and failure-injection coverage; commit and update Linear.
- [ ] Finish KOS-424: extract only proven domain, application, store, sync, and net contracts into crates bottom-up; enforce the target dependency graph; preserve API/schema/wire behavior; commit each verified extraction unit and update Linear.
- [ ] Finish KOS-425: add dependency whitelist and desktop/mobile feature/build matrix CI; prove rejection on an injected forbidden edge/feature; commit and update Linear.
- [ ] Re-run each issue's acceptance checks after any later change that could affect it; do not infer completion from issue status or compilation alone.

## Phase D: Keep the audit trail

- [x] Append decisions and verified checkpoints to `.audit/ark-exocortex.tsv`.
- [ ] Review every new trail row against the transcript and evidence pointers.
- [ ] Obtain an independent review of the completed trail before handoff.

## Phase E: Verify and hand back

- [ ] Audit all 17 issue descriptions and their acceptance criteria against current files, tests, build graphs, and live Linear state.
- [ ] Run the full required root gates on the final tree and inspect the exact commit diff.
- [ ] Mark all child issues and KOS-412 Done only after the final audit proves their acceptance criteria.

## Throughput checkpoint

Use at most two read-only GPT-6-Luna reviewers at a time, keep heavyweight Cargo builds serial, and commit one logical verified unit at a time. Current readers are auditing KOS-429's remaining transport gaps and reconciling the supplied plan against current source and Linear state.

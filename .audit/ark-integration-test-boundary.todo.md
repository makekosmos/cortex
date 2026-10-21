# Integration replication test boundary

- [x] Frame: define the test-support boundary artifact and gradeable criteria.
- [x] Fan out: compare two structurally distinct designs for removing Core's test-only SQLite dependency.
- [x] Cross-judge: independently evaluate both designs against the criteria.
- [x] Pick: select the smallest typed test-support API that retains Core behavior coverage.
- [x] Graft: implement the accepted shape and remove all Core SQLite test access.
- [x] Verify: pass Core/Store tests, release check, lint, format, source-size and unchanged storage-boundary gate.

## Final verification

Core no longer depends on rusqlite, holds concrete SQLite readers, or contains SQL. Store owns test database setup and inspection through named test-support operations. `pnpm run check:affected` passed: 2,079 Rust tests, 5 skipped; Clippy with `-D warnings`, formatting, source-size, and the storage-boundary checks passed. The sync-round-trip regression uses the production writer actor and passed in the full suite.

## Artifact and criteria


Deliver an implementation sketch and rationale for the test-only integration-replication fixtures and signed-frame tests. The design must (1) remove `rusqlite`, concrete SQLite readers, and connection handles from `ark-core`, (2) keep Core-owned authorization, verification, and plaintext-free response assertions in Core, (3) keep SQL setup/inspection and transaction mechanics in Store test support, (4) preserve signed-frame atomicity and failure behavior, and (5) expose no new production API or generic raw-SQL escape hatch. The broader source audit also found test-only `SqliteReader` in outbound validation and `TestDatabase::open_connection` in the sync-round-trip fixture; these are in scope even though the boundary scanner did not report them.

## Synthesis decision

Candidate A is the base (9/10 versus Candidate B at 5/10), with judge-required reductions: the fixture must own `TestDatabase`; use only named typed reads rather than a query-result enum; keep actor lifecycle only in dedicated Core actor tests; and expose test helpers only behind test-support. The broader source audit adds the Core outbound `SqliteReader` test and sync-round-trip connection fixture to the remaining scope. Candidates and independent verdict are retained at `/tmp/ark-integration-test-boundary-candidate-a.md`, `/tmp/ark-integration-test-boundary-candidate-b.md`, and `/tmp/ark-integration-test-boundary-judge.md`.

# ARK Stage 2 execution

## Definition of done

The dependency graph matches the documented ownership, `ark-store` can be exercised without importing `ark-core`, `ark-net` has no storage dependency, `ark-core` keeps the stable Engine request/response contract, the single-writer and transaction invariants hold, legacy internal facades are deleted with their callers, and the required source, build, test, and GitHub CI checks pass on the final SHA.

## Work units

- [x] Revalidate stage branch and capture independent ownership reviews.
- [x] Remove broad storage access from the transport authorization path and enforce that boundary in the graph gate.
- [ ] Move SQLite query/mutation adapters and schema lifecycle into `ark-store`.
  - [x] Move usage journal SQL and `sync_kv` SQL functions; migrate production and test callers and remove the core exports.
  - [x] Move committed-feed watermark SQL; update feed progress and snapshot restore callers.
  - [x] Move SQLite integrity validation and unpublished chunked backup primitives into `ark-store::open`.
  - [x] Move SQLite compatibility DDL and legacy backfills into `ark-store::schema::migrations`; keep canonical migration orchestration in Core.
  - [x] Move the authorized-node SQLite lookup into `ark-store::integration_replication`; migrate callers and remove the Core facade.
  - [x] Move refresh-lease row loading and decoding into `ark-store::integration_replication`; migrate callers and remove the Core facade.
  - [x] Move integration-configuration loading and opaque-cursor scrub into `ark-store::integration_replication`; update the writer census and remove the Core facade.
  - [x] Move credential-envelope row loading and decoding into `ark-store::integration_replication`; keep recipient/grant authorization in Core.
  - [x] Move integration-grant row loading and upsert SQL into `ark-store::integration_replication`; keep epoch, revocation, node-key, and HLC policy in Core.
  - [x] Move integration-configuration upsert SQL into `ark-store::integration_replication`; keep identity and HLC policy in Core.
  - [x] Move authorized-node upsert SQL into `ark-store::integration_replication`; keep epoch, revocation, key-binding, and HLC policy in Core.
  - [x] Move refresh-lease upsert SQL into `ark-store::integration_replication`; keep grant authorization and fencing policy in Core.
  - [x] Move credential-envelope upsert and generation queries into `ark-store::integration_replication`; keep recipient, issuer, lease/fence, generation, and HLC policy in Core.
  - [x] Move authorized-recipient envelope generation selection into `ark-store`; Core supplies the already-authorized node epoch and still validates each exported envelope.
  - [x] Remove the final integration-configuration identity SQL from Core by using the Store query inside the existing savepoint.
- [ ] Move the single writer executor into `ark-store`.
  - [x] Move ownership of the production SQLite connection, immediate transaction, commit, and maintenance execution into `ark-store::writer`; keep source fairness and Engine/sync admission in Core.
  - [x] Move the full snapshot and restore lifecycle into `ark-store` and keep `ark-core::db` request-facing re-exports stable.
- [ ] Move sync session orchestration into `ark-sync` and transport-specific implementations into `ark-net`.
  - [x] Move direct WebSocket address racing and its connection timeout into `ark-net`; keep reconnect and protocol policy in Core.
  - [x] Move TCP listener binding and WebSocket accept handshake into `ark-net`; keep peer lifecycle and admission in Core.
  - [x] Move client Hello offer construction and authentication proof generation into `ark-sync`; keep platform metadata and socket exchange at the boundary.
- [ ] Remove obsolete core facades and dependencies while preserving Engine API and wire shapes.
- [ ] Run focused and full checks; commit, push, and verify exact-SHA CI.
- [ ] Update architecture documentation and reconcile KOS-424/KOS-425 evidence in Linear.

## Latest verification

- Direct WebSocket dialing move passed its loopback test, five `ark-core` sync-client tests, targeted clippy, and desktop/mobile graph checks. The final Stage 2 full gate remains pending.
- Authorized-node lookup move passed all `ark-core` tests (606 across unit, compatibility, and integration targets), all `ark-store` tests (22), `cargo check -p ark-core`, `clippy -p ark-core -p ark-store --all-targets -- -D warnings`, and `cargo fmt --all --check`.
- Refresh-lease lookup move passed its store decoding test, all 140 `ark-core` DB tests, targeted clippy, and format.
- Integration-configuration query move passed the Store cursor-scrub test, the Core legacy-cursor regression test, all 140 Core DB tests, targeted clippy, format, and writer census (78 path/category counts).
- Credential-envelope row loading and decoding now lives in `ark-store`; Core retains recipient/grant authorization. The Store envelope query test, all 140 Core DB tests, targeted clippy, format, and writer census (78 path/category counts) passed.
- Integration-grant read/write SQL now lives in Store; Core retains node authorization, epoch monotonicity, revocation, key binding, HLC, and version-vector policy. Store round-trip/overflow and Core policy tests, targeted clippy, format, and the 78-count writer census passed.
- Integration-configuration upsert SQL now lives in Store; Core retains provider/account identity and HLC policy. Store write round-trip, Core legacy-cursor regression and focused DB tests, targeted clippy, format, and writer census (78 path/category counts) passed.
- Authorized-node upsert SQL now lives beside its Store query. Core retains grant-epoch monotonicity, same-epoch revocation/key binding, HLC, and version-vector policy. Store round-trip/overflow, six issuer lookup tests, local refresh identity test, targeted clippy, format, and writer census (78 path/category counts) passed.
- Refresh-lease upsert SQL now lives in Store. Core retains node/grant authorization, generation monotonicity, and equal-fence policy in the existing transaction. Store round-trip/overflow, concurrent lease winner and publication-fence Core tests, targeted clippy, format, and writer census (78 path/category counts) passed.
- Credential-envelope upsert, generation lookup, and persisted row decoding now live in Store; Core retains recipient/issuer authorization, lease/fence checks, and generation/HLC policy. Store envelope round-trip/max-generation, issuer lookup, peer-export, and publication-fence tests passed with targeted clippy, format, and writer census (78 path/category counts).
- Peer export now asks Store for ordered envelope generations at the current authorized node epoch; Core still decides whether the recipient is current and validates each envelope before export. Store generation-list coverage, both peer-export Core tests, targeted clippy, and format passed.
- Core's integration-replication adapters contain no direct SQL statements: configuration identity validation uses the Store query within its current savepoint, and export selection is Store-owned. The 78-count writer census, integration-config/peer-export regressions, targeted clippy, and format passed.
- Client Hello ownership move passed its serialized-handshake test, all 5 `ark-core` sync-client tests, targeted clippy, format, and desktop/mobile graph checks. The final Stage 2 full gate remains pending.
- Server Hello negotiation now lives in `ark-sync`; its focused tests, all 72 `ark-sync` tests, all 5 sync-client tests, direct sync round-trip, targeted clippy, formatting, and source-size checks passed. One server test intermittently reset the WebSocket connection during `connect_async` in two filtered runs; both isolated reruns passed. The full Stage 2 gate remains pending.
- Added bounded typed `SessionIo` in `ark-sync` and a supervised WebSocket adapter in `ark-net`; all 74 `ark-sync` tests, all 38 `ark-net` tests, targeted clippy, formatting, and source-size checks passed. The adapter's loopback test covers typed frame exchange, malformed frames, remote close, and worker join. Core loops still use the prior transport path until the next migration unit.
- The direct WebSocket client now consumes typed `SessionIo` events and sends typed messages through bounded queues; removed the Core client writer task. A pre-commit review caught a seed-ID policy check and a possible full-duplex backpressure cycle. The sender now uses the Hello-authenticated peer ID, bulk snapshots run outside the receive loop, and transport keeps a separate bounded control queue ahead of bulk frames. Added coverage that admission sends the client policy when the discovered peer ID is a `seed-*` placeholder and that control messages still enqueue when bulk is full. Server Core loop and session-state extraction remain.
- Client session unit closed: 113 `ark-sync`/`ark-net` tests, all 5 `ark-core` sync-client tests, four committed-feed regressions, targeted clippy with `-D warnings`, root include-fragment formatting, repository format and source-size gates, and `git diff --check` passed. An independent code-quality re-review found no remaining findings. This does not close the full Stage 2 gate.
- Server session transport now uses typed bounded `SessionIo` and the `ark-net` WebSocket adapter. The write-boundary guard still checks current peer admission and signed integration authorization. Independent review found and prompted two ordering fixes: send local `SyncPolicy` only after activating the admitted peer, and snapshot registry fanout targets before awaiting bounded sends. The reviewer rechecked both fixes and found no remaining findings. Extracted WebSocket tests to keep `websocket.rs` under the 500-line gate. `ark-net` passed 39 tests, `ark-core sync_server` passed 21 tests, `ark-sync session_io` passed 4 tests, targeted clippy passed, and root formatting, source-size, and diff checks passed. The server connected-message loop remains in Core and is the next extraction unit.
- The server's first-message gate now lives in a per-connection `ark-sync` state machine: it owns one-time Hello acceptance, authentication/protocol validation, self-connect rejection, and the pending block-list transition. Core performs the existing block-list read, drops stale completions after checking the connection id, creates registry admission, and sends/closes the transport. Five `ark-sync server_session` tests and targeted clippy passed; the authenticated Core Hello test passed on isolated retry. The grouped Core filter intermittently reset the test's ephemeral TCP connection before WebSocket setup, as seen in earlier server filters. The full server message loop remains in Core.
- The same `ark-sync` session now owns the exactly-once policy-bound sync-start phase. After registry admission, Core must claim the policy-persistence phase before validating and writing the incoming policy; the session permits one sync start only after that write succeeds. Core continues to load and send the initial vector. Six `ark-sync server_session` tests, two-server sync round-trip, selective-profile reconnect/narrowing integration, admission-focused Core tests, Clippy, formatting, source-size, and diff checks passed. The selective-sync test's initial poll interval was raised from 25 ms to 100 ms after two intermittent five-second timeouts observed only type definitions; it then passed three times, including once after the final enum API change. Keep an eye on the full Stage 2 gate.
- WebSocket listener move passed its loopback handshake/frame test, all 21 Core sync-server tests in a serial run, targeted clippy, and format. One parallel run saw a connection reset in the existing ephemeral-port test; its isolated rerun and serial suite passed. Final Stage 2 full gate remains pending.
- SQLite compatibility migration move passed the full affected gate: workspace clippy, 1,912 Rust tests (5 skipped), and Manager checks. Nextest run `c2acda2e-df38-4fc4-bf6c-b7e97d4c2e5a`; full output: `/tmp/arkrp/ark-stage2-schema-compat-final.log`. Writer census passed with 77 path/category counts.
- `pnpm run check:affected` selected the full gate and passed after introducing `ark_store::writer::SqliteWriter`: workspace clippy, 1,912 Rust tests (5 skipped), and Manager checks included. Nextest run `9d9c8fe1-b860-4ad1-9a85-f314aa1b5b72`; full output: `/tmp/arkrp/ark-stage2-writer-extraction-final.log`.
- The writer-focused test proves successful commit and rollback of partial writes on command failure; all 21 production actor tests passed. The SQLite writer census passed with 75 path/category counts.
- `pnpm run check:affected` selected the full gate and passed after moving snapshot/restore ownership: workspace clippy, 1,911 Rust tests (5 skipped), and Manager checks included. Nextest run `c39f131b-7fe7-4a1e-83fe-c399a83f21d0`; full output: `/tmp/arkrp/ark-stage2-snapshot-move-final-3.log`.
- `ark-store` remains independently buildable; `cargo metadata` shows no `ark-core` dependency, and all 73 SQLite writer path/category counts pass after moving snapshot SQL ownership.
- `pnpm run check:affected` selected the full gate and passed after the integrity and backup primitive move: workspace clippy, 1,910 Rust tests (5 skipped), and Manager checks included. Nextest run `4154cc8d-1a65-4c09-b723-c6ba2729ea6c`; full output: `/tmp/arkrp/ark-stage2-open-primitives-final.log`.
- The SQLite writer census passed with 73 path/category counts after moving one lifecycle path from `ark-core::db::open` to `ark-store::open`.
- `pnpm run check:affected` selected the full gate and passed after the committed-feed watermark move: workspace clippy, 1,909 Rust tests (5 skipped), and Manager checks included. Full output: `/tmp/arkrp/ark-stage2-feed-watermark-check.log`.
- `ark-store` nextest run `6ec62cd4-0149-45aa-9379-7cf5c2a0ef6b` passed 15 tests. The core committed-feed watermark test passed, and snapshot-related nextest run `7a9f9151-a967-45ae-8396-3947a20bf1c8` passed 30 tests.
- The SQLite writer census passed with 73 path/category counts.
- The source boundary rejects `SyncRuntimeStorage`, `IntegrationReplicationBackend`, `StorageBackedOutboundIntegrationFrameAuthorizer`, `ark_store`, and `rusqlite`; its 5 tests and desktop/mobile graph checks pass.
- GitHub CI for the published parent `42f93c2c` passed macOS, Windows, Android, and lint. Current local commits have no exact-SHA CI because they have not been pushed.
- The pre-commit formatter now resolves the pinned `oxfmt` package directly; its regression test passes when `node_modules/.bin` is absent from `PATH`.

## Decision trail

The detailed interface rationale is in `ark-stage2-boundaries.md`. Run decisions and verification evidence append to `ark-stage2.tsv`.

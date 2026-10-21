# Ark API and wire baseline

This baseline pins the current public request, host response, LAN wire, and signed canonical-byte behavior. Fixtures are literal JSON or hex; tests exercise the production `Request` deserializer, `ArkHost::request`, LAN serializer/deserializer, and `SignedSyncEnvelope::canonical_signing_bytes`.

The gate was run against base `ba8a9dfa977e8b0af80046bb4124bc8decb3070a` and the current tree by copying only the test modules and fixtures into a temporary detached base worktree. All five filters passed on both trees:

| Surface | Command | Result on base and current |
|---|---|---|
| Ark Core request JSON | `cargo nextest run -p ark-core ark_api_v1_requests_remain_deserializable` | 1/1 passed |
| LAN exact bytes, including signed frame and ack | `cargo nextest run -p ark-core --test it lan_wire_v1_fixtures_decode_and_preserve_exact_current_bytes` | 1/1 passed |
| Signed canonical bytes | `cargo nextest run -p ark-core --test it signed_sync_v1_canonical_signing_bytes_match_fixture` | 1/1 passed |
| Six-device import/edit interaction | `cargo nextest run -p ark-core --test it six_devices_converge_during_initial_import_and_local_edit` | 1/1 passed |
| Real ArkHost response envelope bytes | `cargo nextest run -p engine-packages request_response_bytes_match_ark_api_v1_fixtures` | 1/1 passed |

The request corpus covers `load_all`, legacy todo upsert, canonical object upsert/delete with expected snapshots, the pairing-code alias, and signed integration send. The response corpus covers one success and one malformed-request error. The LAN corpus checks old incoming bytes against the current serialized form and semantic decode/re-encode equality for hello, sync changes, live change, acknowledgements, and signed integration frame/ack. Signed canonical bytes are pinned separately as hex.

The six-device scenario uses six real in-memory SQLite backends and the production sync server/client. It seeds 256 selected `object` records, waits for a production storage-page request and an authenticated client callback, applies a local edit while the initial page is held, then releases the page and checks convergence across all five importing peers. It then narrows one device's profile and checks that already-imported local rows remain stored. This is a functional baseline only; it makes no performance or remote-visible timing claim.

The existing `phase3_migration_system::clear_all_removes_ephemeral_rows_but_keeps_locked_registry_and_archive` test was extended with live rows in `usage_sync_heads`, `integration_configurations`, and `authorized_nodes`. Its assertions pass on base and current, alongside the existing checks for retained system-locked types and archive rows.

Cargo feature trees were captured on base and current using:

```sh
cargo tree -p ark-core -e features --locked
cargo tree -p engine -e features --locked
cargo tree -p engine-mobile -e features --locked --target aarch64-linux-android
```

The resulting trees had 2,594, 3,739, and 2,272 lines. After normalizing each worktree root path, base and current outputs were identical. Raw base/current outputs are retained under `.audit/kos413/` for this task's review. The comparison replaced each absolute worktree path with `<WORKTREE>` and then ran `diff -q` on each package's base/current pair.

The complete scoped writer inventory and known boundaries are in [ark-writes.md](ark-writes.md). In particular, outermost SQLite savepoint `RELEASE` commits when there is no enclosing transaction; a nested release remains inside its caller's transaction. Pull cursor persistence follows entity apply and is a separate write boundary. `clear_all` resets a subset of content/progress rows without per-entity deletion metadata; it preserves system-locked type definitions, integration/auth records, and `usage_sync_heads`.

Current compile checks passed for ark-core, engine-packages, engine, and host-target engine-mobile (`cargo check -p <package> --all-targets --locked`). Android engine-mobile cross-check remains unavailable because the installed environment has no `aarch64-linux-android-clang`; see `.audit/kos413/receipts.md` for the exact attempt and result.

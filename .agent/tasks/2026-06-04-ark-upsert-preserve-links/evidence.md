# Evidence — 2026-06-04 ARK upsert preserves links

## Summary

The generic object model upsert helpers in `crates/ark-core/rust/src/db.rs` now use `INSERT ... ON CONFLICT(id) DO UPDATE` instead of SQLite `INSERT OR REPLACE`. Regression tests cover object, object type, and object link upsert behavior.

## Results

### AC1 — ON CONFLICT semantics for generic object model helpers

Verdict: PASS.

Command:

```powershell
rg -n "INSERT OR REPLACE INTO (object_types|objects|object_links)|INSERT INTO (object_types|objects|object_links)|ON CONFLICT\(id\) DO UPDATE|upsert_object_preserves_existing_links|upsert_object_type_preserves_existing_objects_and_links|upsert_object_link_updates_existing_row_in_place" crates/ark-core/rust/src/db.rs
```

Relevant output:

```text
613:        "INSERT INTO object_types
616:         ON CONFLICT(id) DO UPDATE SET
881:            "INSERT INTO objects
884:             ON CONFLICT(id) DO UPDATE SET
1395:        "INSERT INTO object_links
1398:         ON CONFLICT(id) DO UPDATE SET
3793:    fn upsert_object_preserves_existing_links() {
3829:    fn upsert_object_type_preserves_existing_objects_and_links() {
3853:    fn upsert_object_link_updates_existing_row_in_place() {
```

### AC2 — Object upsert preserves links

Verdict: PASS.

Command:

```powershell
cargo test --manifest-path crates\ark-core\rust\Cargo.toml --lib upsert_object
```

Relevant output:

```text
running 3 tests
test db::tests::upsert_object_preserves_existing_links ... ok
test db::tests::upsert_object_type_preserves_existing_objects_and_links ... ok
test db::tests::upsert_object_link_updates_existing_row_in_place ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 163 filtered out
```

### AC3 — Object type upsert preserves objects and links

Verdict: PASS.

Covered by:

```text
test db::tests::upsert_object_type_preserves_existing_objects_and_links ... ok
```

### AC4 — Object link upsert updates in place

Verdict: PASS.

Covered by:

```text
test db::tests::upsert_object_link_updates_existing_row_in_place ... ok
```

### AC5 — Required guards

Verdict: PASS.

Commands:

```powershell
bun run ark:guard:writes
$env:CARGO_TARGET_DIR='.tmp\cargo-ark-smoke'; bun run ark:smoke
bun run docs:sync
bun run docs:check
```

Relevant output:

```text
ARK write boundary guard passed.
ARK smoke matrix passed.
docs:sync: done.
docs:check: everything fresh, stale references not found.
```

Note: `bun run ark:smoke` first failed because the default `target\debug\ark-core-rpc.exe` was locked by a running process. The verifier reran smoke with `CARGO_TARGET_DIR=.tmp\cargo-ark-smoke`, which avoids touching the locked exe and passed.

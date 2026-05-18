# Evidence: Bug Audit — Wave 2

**Date:** 2026-05-17  
**Branch:** claude/audit-project-bugs-RhvQl

## AC-1: mesh.rs HLC dedup — PASS

`dedup_key_from_message` now uses `HLC::from_string(&entity.hlc).device_id` instead of the
broken `entity.hlc.splitn(3, ':').nth(2)` which returned wrong results for ISO8601 timestamps
containing colons.

Test added: `dedup_key_extracts_device_id_correctly` — verifies HLC
`"2026-03-28T14:30:00.123Z:000042:my-device-abc"` → `device_id == "my-device-abc"`.

```
test mesh::tests::dedup_key_extracts_device_id_correctly ... ok
test mesh::tests::dedup_key_returns_none_for_sync_changes ... ok
```

## AC-2: rawg.rs HTTP timeout — PASS

`build_client()` now includes `.timeout(Duration::from_secs(30))`.

```rust
fn build_client() -> Result<reqwest::Client, RawgError> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}
```

Verified via `rawg_timeout_configured` test in rawg.rs.

## AC-3: CSV formula injection — PASS

`csv_safe_cell()` added to `export/mod.rs`. Prefixes `=`, `+`, `-`, `@` with TAB.
Applied to `title` and `tags` in `task_csv.rs`, and to `title` and `source` in `time_entry_csv.rs`.

```
test export::tests::csv_safe_cell_sanitizes_formula_prefix ... ok
test export::task_csv::tests::writes_csv_header_and_two_rows ... ok
test export::task_csv::tests::quotes_titles_with_commas ... ok
test export::time_entry_csv::tests::writes_entries ... ok
test export::time_entry_csv::tests::handles_empty_input ... ok
```

## AC-4: extension-host.ts icon path traversal — PASS

Added bounds check before `readFileSync`:

```ts
const iconPath = path.resolve(path.join(dir, manifest.icon));
if (!iconPath.startsWith(path.resolve(dir) + path.sep) && iconPath !== path.resolve(dir)) return undefined;
```

Any `manifest.icon` with `..` traversal is rejected before reading.

## AC-5: db.rs replay_pending_for_type atomicity — PASS

Loop wrapped in `BEGIN IMMEDIATE` / `COMMIT`. On any error, `ROLLBACK` is executed.
Events are emitted only after successful `COMMIT`.

```rust
conn.execute_batch("BEGIN IMMEDIATE").map_err(|e| e.to_string())?;
// ... upsert + DELETE loop ...
conn.execute_batch("COMMIT")?;
// emit events
```

All ark-core tests: `test result: ok. 147 passed`.

## AC-6: extension-marketplace.ts redirect limit — PASS

`fetchUrl` now takes `depth` parameter, rejects at depth > 5 and blocks non-HTTPS redirects:

```ts
const fetchUrl = (u: string, depth = 0) => {
  if (depth > 5) { reject(new Error(`too many redirects...`)); return; }
  if (!u.startsWith("https://")) { reject(new Error(`redirect to non-HTTPS URL blocked...`)); return; }
  // ...
  fetchUrl(res.headers.location, depth + 1);
```

## AC-7: relay_transport.rs outbox bound — PASS

Outbox capped at 500 messages:

```rust
const MAX_OUTBOX_SIZE: usize = 500;
let mut outbox = self.outbox.lock().unwrap();
if outbox.len() < MAX_OUTBOX_SIZE {
    outbox.push_back(msg);
}
```

## AC-8: note_md.rs YAML newline escape — PASS

`yaml_scalar` now escapes `\n`:

```rust
let escaped = input
    .replace('\\', "\\\\")
    .replace('"', "\\\"")
    .replace('\n', "\\n");
```

Test:
```
test export::note_md::tests::yaml_scalar_escapes_newlines ... ok
```

## AC-9: note_md.rs unsafe href — PASS

Non-http(s) hrefs replaced with `about:blank`:

```rust
if href.starts_with("http://") || href.starts_with("https://") {
    out = format!("[{out}]({href})");
} else {
    out = format!("[{out}](about:blank)");
}
```

Test:
```
test export::note_md::tests::unsafe_link_href_replaced_with_blank ... ok
```

## AC-10: scanner.rs VDF depth limit — PASS

`parse_object_body` now takes `depth: usize` and returns `VdfNode::Null` at depth > 64.

Test:
```
test arrancador::scanner::tests::vdf_depth_limit_prevents_stack_overflow ... ok
```

## AC-11: protocol.rs serialization log — PASS

`serialize_message` now logs on failure:

```rust
Err(e) => {
    eprintln!("[ark-core] serialize_message failed: {e}");
    String::new()
}
```

## AC-12: settings-window.ts atomic write — PASS

Write via temp file + `renameSync`:

```ts
const tmp = target + ".tmp";
writeFileSync(tmp, JSON.stringify(next, null, 2), "utf8");
renameSync(tmp, target);
```

## Final test run

```
ark-core:        test result: ok. 149 passed; 0 failed (lib + integration)
kepler-backend:  test result: ok. 112 passed; 0 failed
```

All 12 ACs: PASS.

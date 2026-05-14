# Raw command results: ARK RPC query endpoints

## cargo test object query helper

Command:

```powershell
cargo test --manifest-path packages\ark-core\rust\Cargo.toml object_query_helpers_filter_by_type_and_ids
```

Result: PASS

## cargo test usage process query helper

Command:

```powershell
cargo test --manifest-path packages\ark-core\rust\Cargo.toml usage_process_queries_return_recent_and_search_candidates
```

Result: PASS

## cargo test full ark-core

Command:

```powershell
cargo test --manifest-path packages\ark-core\rust\Cargo.toml
```

Result: PASS, 115 tests passed.

## SDK typecheck

Command:

```powershell
bun run --cwd packages/kosmos-ark typecheck
```

Result: PASS

## Arrancador tests

Command:

```powershell
bun run --cwd apps/arrancador test
```

Result: PASS, 47 files and 161 tests passed.

## Arrancador typecheck

Command:

```powershell
bun run --cwd apps/arrancador typecheck
```

Result: PASS

## ARK smoke matrix

Command:

```powershell
bun run ark:smoke
```

Result: PASS, ARK smoke matrix passed. This smoke matrix uses isolated test/smoke databases.

## Diff check

Command:

```powershell
git diff --check
```

Result: PASS. Only LF/CRLF warnings were printed by Git for existing modified files.

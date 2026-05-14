# Raw command results: ARK usage playtime summary endpoint

## cargo fmt

Command:

```powershell
cargo fmt --manifest-path packages\ark-core\rust\Cargo.toml
```

Result: PASS

## focused Rust test

Command:

```powershell
cargo test --manifest-path packages\ark-core\rust\Cargo.toml usage_game_playtime_summary_matches_bindings_and_range
```

Result: PASS

## full ARK core Rust tests

Command:

```powershell
cargo test --manifest-path packages\ark-core\rust\Cargo.toml
```

Result: PASS

## SDK typecheck

Command:

```powershell
bun run --cwd packages/kosmos-ark typecheck
```

Result: PASS

## Arrancador focused tests

Command:

```powershell
bun run --cwd apps/arrancador test ark-usage
```

Result: PASS, 5 files and 19 tests passed.

## Arrancador full tests

Command:

```powershell
bun run --cwd apps/arrancador test
```

Result: PASS, 47 files and 163 tests passed.

## Arrancador typecheck

Command:

```powershell
bun run --cwd apps/arrancador typecheck
```

Result: PASS

## diff check

Command:

```powershell
git diff --check
```

Result: PASS. Git printed only LF/CRLF warnings for modified files.

## ARK smoke matrix

Command:

```powershell
bun run ark:smoke
```

Result: PASS after escalated rerun. The first sandboxed run failed with `spawn EPERM` while starting the Playwright worker. The rerun passed and used isolated smoke/test databases.

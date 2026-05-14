# Raw command results

Run date: 2026-04-25

## SDK packages

```text
$ bun run --cwd packages/kosmos-ark typecheck
$ tsc --noEmit
PASS

$ bun run --cwd packages/kosmos-ark build
$ tsc
PASS

$ bun run --cwd packages/arksync-node typecheck
$ tsc --noEmit
PASS

$ bun run --cwd packages/arksync-node build
$ tsc
PASS
```

## App/package resolution

```text
$ bun run --cwd apps/arrancador build:main
Bundled 73 modules in 26ms
PASS

$ bun run --cwd apps/arrancador test
Test Files 47 passed (47)
Tests 161 passed (161)
PASS

$ bun run --cwd apps/dashboard typecheck
$ tsc --noEmit
PASS

$ bun run --cwd apps/dashboard build
electron-rebuild better-sqlite3: Rebuild Complete
vite renderer/main/preload builds: PASS
PASS

$ bun run --cwd apps/dashboard smoke:seed
Smoke dashboard DB seeded at apps/dashboard/.tmp/smoke-dashboard.db
PASS

$ bun run --cwd apps/dashboard smoke:analytics
sessions: 33
topApp: Odyssey Browser
recentSessions: 10
PASS

$ bun run --cwd apps/dashboard test:e2e:smoke
status: ok
statusText: База подключена
route: sessions
PASS
```

## Rust core

```text
$ cargo test --manifest-path packages\ark-core\rust\Cargo.toml
lib tests: 113 passed
main tests: 7 passed
relay_round_trip: 1 passed
sync_round_trip: 5 passed
doc tests: 0 passed
PASS
```

## Diff checks

```text
$ git diff --check
PASS
Only CRLF normalization warnings were emitted; no whitespace errors.
```

# Raw command results

Run date: 2026-04-25

## Delphi

```text
$ bun run --cwd apps/delphi/ts test
Test Files 8 passed (8)
Tests 97 passed (97)
PASS

$ bun run --cwd apps/delphi/ts build:web
BUILD_TARGET=web tsc + Vite web build
PASS
```

## Eden

```text
$ bun run --cwd apps/eden/ts test:ark-migration
{"status":"ok","objectTypes":["note_obj","research_note"],"objects":["note-a","note-b"],"links":["note-a:related:note-b"]}
PASS

$ bun run --cwd apps/eden/ts build
cargo build heart: PASS
cargo build ark-core-rpc: PASS
tsc: PASS
vite renderer/main/preload builds: PASS
PASS
```

## Documentation and diff checks

```text
$ rg -n "delphi-db|@arksync/node|packages/arksync-node/src/ark-client.ts" apps\delphi\AGENTS.md
No matches
PASS

$ git diff --check
PASS
Only CRLF normalization warnings were emitted; no whitespace errors.
```

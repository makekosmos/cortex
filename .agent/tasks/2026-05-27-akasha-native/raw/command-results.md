# Command Results

## cargo check -p akasha

PASS. First run timed out during cold dependency build; rerun completed:

```text
Finished `dev` profile [unoptimized + debuginfo]
```

## cargo test -p akasha

PASS after cold build retry:

```text
running 2 tests
test state::tests::persists_reader_state ... ok
test epub::tests::extracts_spine_text_in_order ... ok
test result: ok. 2 passed
```

## bun run --cwd shell typecheck

PASS:

```text
$ tsc --noEmit
```

## bun run --cwd shell build:extensions

PASS. Built existing Vue extensions and native Akasha:

```text
[build:extensions] arrancador
[build:extensions] delphi
[build:extensions] eden
[build:extensions] horologion
[build:extensions:native] akasha
Finished `dev` profile
```

## bun run --cwd shell build:js

PASS. Shell renderer/main/preload and extensions built. Vite emitted existing
CSS/plugin warnings, but command exited 0.

## bun run ark:guard:writes

PASS:

```text
ARK write boundary guard passed.
```

## bun run test:e2e -- --grep "extension contract: akasha"

Initial run failed because the native branch checked dynamic commands instead
of manifest-declared commands. After the fix:

```text
1 passed
```

## bun run test:e2e -- tests/e2e/extensions-contract.spec.ts

PASS:

```text
5 passed
```

## node --check shell/scripts/build-extensions.mjs

PASS.

## node --check shell/scripts/publish-extension.mjs

PASS.

## cargo build --release -p akasha

PASS after cold build retry:

```text
Finished `release` profile [optimized]
```

## bun run docs:build

PASS. Also ran `docs:sync`.

## bun run docs:check

PASS:

```text
всё свежо, stale references не найдено
```

## bun run ark:smoke

Initial run timed out. Rerun passed:

```text
ARK smoke matrix passed.
```

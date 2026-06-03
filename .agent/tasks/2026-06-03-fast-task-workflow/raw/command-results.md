# Command Results

## `node shell/scripts/build-extensions.mjs --help`

Exit code: 0.

Printed usage for:

- default full build;
- `--only <id[,id...]>`;
- `--only=<id[,id...]>`;
- `--changed`;
- `--vue-only`;
- `--skip-native`.

## `node shell/scripts/build-extensions.mjs --only=`

Exit code: 1, expected.

Error:

```text
[build:extensions] --only requires at least one extension id
```

## `node shell/scripts/build-extensions.mjs --only eden --changed`

Exit code: 1, expected.

Error:

```text
[build:extensions] --only and --changed are mutually exclusive
```

## `node shell/scripts/dev-extensions.mjs --only=__missing__`

Exit code: 1, expected.

Output:

```text
[dev:extensions] unknown or non-dev Vue extension id(s): __missing__
```

## `bun run --cwd shell build:extensions:only eden --skip-native`

Exit code: 0.

Built Eden with Vite and ended with:

```text
[build:extensions:native] no native extensions to build
```

## `bun run --cwd shell build:extensions:changed`

Exit code: 0.

Built affected Vue extensions and ended with:

```text
[build:extensions:native] no native extensions to build
```

## `bun run docs:sync`

Exit code: 0.

Regenerated:

- `AGENTS.md`
- `CLAUDE.md`
- `mobile/delphi/AGENTS.md`
- `crates/ark-core/AGENTS.md`
- `docs-site/public/llms.txt`
- `docs-site/public/full-llms.txt`

## `bun run docs:check`

Exit code: 0.

Output:

```text
всё свежо, stale references не найдено
```

# Evidence: Phase 2.5 — tooling stack sync

## Что найдено

Документация (`docs-site/guide/tooling.md`) описывала несуществующий
state. Reality check:

| Утверждение docs            | До Phase 2.5                     | После               |
| --------------------------- | -------------------------------- | ------------------- |
| `bun run lint` существует   | ❌ нет скрипта                   | ✅ `oxlint`         |
| `bun run format` существует | ❌ нет                           | ✅ `oxfmt`          |
| `bun run format:check`      | ❌ нет                           | ✅ `oxfmt --check`  |
| `.oxlintrc.json` в корне    | ❌                               | ✅ (Phase 2)        |
| `.oxfmtrc.json`             | ❌                               | ✅                  |
| oxfmt в lefthook            | ❌                               | ✅                  |
| oxfmt версии унифицированы  | ❌ (root 0.43, visuals 0.7)      | ✅ только root 0.43 |
| Massive baseline format     | 1106/1430 файлов неформатированы | ✅ 1405/1405 clean  |

`crates/ark-core/benches` (typo) исправлен на `crates/ark-core/rust/benches`.

## AC verification

### AC1 — корневые scripts

```
$ bun run lint
$ oxlint
... 0 errors, 16 warnings
$ bun run format:check
$ oxfmt --check
All matched files use the correct format.
```

PASS.

### AC2 — oxfmt --check clean

```
$ bunx oxfmt --check .
All matched files use the correct format.
Finished in 5847ms on 1405 files using 12 threads.
```

PASS.

### AC3 — massive format apply не сломал ничего

```
typecheck shell:   green
oxlint:            0 errors, 16 warnings (тот же baseline что до format)
ark:guard:writes:  passed
playwright eden:   9/9 passed (53.4s)
```

PASS.

### AC4 — lefthook

```yaml
oxfmt:
  glob: "*.{ts,tsx,vue,mjs,cjs,js,jsx,json}"
  run: bunx oxfmt --check {staged_files}
```

`bunx lefthook validate` → All good. PASS.

### AC5 — tooling.md обновлён

После правки секции «Линт и формат»:

- описаны 4 активных guards: oxlint config, oxfmt config, workspace.lints, ark:guard:writes.
- описаны pre-commit / pre-push фазы lefthook.

`bun run docs:sync` → done. `bun run docs:check` → fail на pre-existing
`/memory` link в `docs-site/apps/kepler-roadmap.md`, **не** в tooling.md
(этот файл проходит чисто). Отдельная задача.

PASS (по tooling.md).

### AC6 — packages/visuals без локальной oxfmt

```diff
-    "oxfmt": "^0.7.0",
```

PASS.

## Файлы и commits

```
fc1bae1b chore(fmt): oxfmt setup — .oxfmtrc.json + корневые scripts + унификация версий
0089983d chore(fmt): mass oxfmt apply (1405 files)
fda4c1d4 chore(lefthook): oxfmt --check на pre-commit
ae23ccde docs(tooling): обновить guide/tooling.md под реальное состояние стека
```

Изменено вне format-only:

- `.oxfmtrc.json` (новый)
- `package.json` (+lint/format/format:check scripts)
- `bun.lock` (visuals oxfmt removed)
- `packages/visuals/package.json` (oxfmt dep removed)
- `lefthook.yml` (+oxfmt hook)
- `docs-site/guide/tooling.md` (rewrite section + benches typo fix)
- regenerated `AGENTS.md` / `CLAUDE.md` / `crates/ark-core/AGENTS.md` /
  `mobile/delphi/AGENTS.md` / `docs-site/public/full-llms.txt` (через
  `bun run docs:sync`).

## Known issues TODO

- `/memory` link в `docs-site/apps/kepler-roadmap.md` — pre-existing, чинить
  отдельно.
- 16 oxlint warnings в `scripts/` и пары других мест (unused vars / useless
  fallback) — отдельный hygiene proof loop.

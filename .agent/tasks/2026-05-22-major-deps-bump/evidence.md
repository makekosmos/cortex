# Evidence: Phase 2.6 — major deps bump (5 packages)

## Сводная таблица

| Пакет | Before | After | Workspaces | Risk | Reality |
|---|---|---|---|---|---|
| vue-router | 4.6.4 | **5.0.7** | delphi, horologion, visuals, root | low | no-op в коде |
| uuid | 13.0.2 | **14.0.0** | extensions/eden | low | named import совместим |
| lucide-vue-next | 0.548.0 | **1.0.0** | shell, horologion, visuals, site | low-medium | no brand icons → safe |
| electron | 41.6.0 | **42.2.0** | shell, root | medium | нет native deps → safe |
| typescript | 5.8.3 | **6.0.3** | shell, packages/ark | medium-high | 1 deprecation (baseUrl) + 1 type discovery fix |

**Не сделано: `@types/node` 24 → 25** — потенциально вредно. `@types/node` должен отражать **runtime** который реально доступен. У нас:
- bun 1.3 (реализует Node 22-23 API set).
- Electron 42 встроенная Node = 22.21.x.

Поднимать `@types/node` впереди Node 22 → autocomplete покажет API'ы Node 25 (новые crypto-методы, новые Web Streams) → код напишется → runtime упадёт. Поднимать когда Electron подтянет Node 24/25 внутри (Electron 47-48, конец 2026).

## Commits chain

```
a6dce781 chore(deps): bump typescript 5.8 → 6.0.3
c1b4222c chore(deps): bump electron 41 → 42
0750dd39 chore(deps): bump lucide-vue-next 0.548 → 1.0.0
cfa66f04 chore(deps): bump uuid 13.0.2 → 14.0.0 в extensions/eden
7dce6a40 chore(deps): bump vue-router 4.x → 5.0.7
```

## Что сломалось по дороге (и как починили)

### TypeScript 6.0
- **`baseUrl` deprecated** (functioning до TS 7) — добавлен
  `"ignoreDeprecations": "6.0"` в shell, delphi, site tsconfig.
- **`packages/ark` потерял auto `@types/node`** на `module=NodeNext` —
  добавлен explicit `"types": ["node"]` в `packages/ark/tsconfig.json`.

Других ожидаемых breaking changes (`noUncheckedSideEffectImports default`,
`esModuleInterop`, `target=es2025`) — не сработали, потому что наши tsconfig'и
явно задают эти опции (не полагаются на defaults).

### Electron 42
- `clearStorageData({ quotas })` — grep clean.
- Native node deps — у нас нет (ARK это spawn'ный Rust child process).
  `electron-rebuild` не нужен.
- postinstall lazy binary download — bun install прошёл без сюрпризов.

### lucide-vue-next 1.0
- Brand icons (GitHub, Twitter, Figma, etc) удалены — у нас не используются
  (grep clean во всём workspace).
- NB: lucide теперь также под scope `@lucide/vue@1.16.0`. На переход —
  отдельная задача (rename 40+ импортов), сейчас остались на старом scope.

### uuid 14.0
- ESM-only, named imports не тронуты. Единственный callsite —
  `extensions/eden/src/store/eden.ts:5` — совместим.

### vue-router 5.0
- Composition-API API (createRouter, useRoute, useRouter, router-link,
  router-view) — не изменился.
- unplugin-vue-router (где было бы breaking change на import path) — не
  используем.

## AC verification

### AC1 — 5 commits

```
$ git log --oneline -5
a6dce781 chore(deps): bump typescript 5.8 → 6.0.3
c1b4222c chore(deps): bump electron 41 → 42
0750dd39 chore(deps): bump lucide-vue-next 0.548 → 1.0.0
cfa66f04 chore(deps): bump uuid 13.0.2 → 14.0.0 в extensions/eden
7dce6a40 chore(deps): bump vue-router 4.x → 5.0.7
```
PASS.

### AC2 — Final verify

```
$ bun run --cwd shell typecheck       → green
$ bun run --cwd packages/ark typecheck → green
$ bun run --cwd shell build:js         → green (812ms)
$ bunx oxlint .                        → 0 errors, 16 warnings (same baseline)
$ bunx oxfmt --check .                 → All matched files use the correct format
$ bun run ark:guard:writes             → passed
$ cargo clippy --workspace --all-targets → 0 errors (lib test warnings — known)
$ bunx playwright test tests/e2e/eden.spec.ts → 9 passed (49.5s)
```
PASS.

### AC3 — TS 6 deprecations handled

`"ignoreDeprecations": "6.0"` добавлен в 3 tsconfig:
- `shell/tsconfig.json`
- `extensions/delphi/tsconfig.json`
- `site/tsconfig.json`

`packages/ark/tsconfig.json` не требует — там нет `baseUrl`. Но добавлен
explicit `"types": ["node"]` для регрессии node module discovery.

PASS.

### AC4 — @types/node обоснование

См. сводную таблицу выше + лекцию в spec'е. Решение зафиксировано:
оставить `^24.9.2`, не поднимать до 25.x.

PASS.

## Smoke timings (для контекста по latency)

| Spec | Phase 1 baseline | Phase 2.6 | Δ |
|---|---|---|---|
| eden.spec.ts (9 tests) | 51.5s | 49.5s | −2s (within noise) |

Не регрессия по latency после всех bump'ов.

## Known TODO

- `lucide-vue-next` → `@lucide/vue` rename (отдельная задача).
- TS `baseUrl` → relative paths в `paths` (до TS 7 hard removal).
- Unlisted `lucide-vue-next` / `vue-router` в arrancador/delphi/eden
  package.json (отдельная hi.fix).

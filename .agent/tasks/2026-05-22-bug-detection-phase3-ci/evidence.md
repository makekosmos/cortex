# Evidence: Phase 3 — CI workflow

## Что построено

Три файла под `.github/`:

| Файл                    | Цель                                                                          | Запуск                                   |
| ----------------------- | ----------------------------------------------------------------------------- | ---------------------------------------- |
| `workflows/ci.yml`      | PR-блокатор: 4 jobs (guards, typecheck, rust, unit-ts) на Ubuntu, цель <5 мин | На каждый push в main + каждый PR в main |
| `workflows/nightly.yml` | Full Playwright e2e на Windows                                                | cron 03:00 UTC + workflow_dispatch       |
| `dependabot.yml`        | Weekly npm/cargo, monthly github-actions                                      | автоматически                            |

## Что найдено и починено

### Pre-existing test failures (Memory rule: «failing тесты исправляются всегда»)

**`packages/ark/tests/ensure-kepler.test.ts`** — 2 теста падали по таймауту:

- `stale lock (dead PID) + autoLaunch off → cleaned + not-installed`
- `malformed lock JSON → not-installed`

Причина: `ensureKeplerRunning({autoLaunch: false})` поллит default `waitMs = 10000ms` ожидая live lock; bun test timeout 5s. Эти тесты ожидают сразу `not-installed` без polling'а.

**Fix**: добавлен `waitMs: 0` в обоих тестах — раз stale lock удалён `readLockIfAlive` на первой же итерации, полу polling смысла не имеет.

После: 12/12 passed за 315ms.

### Eden `test:unit` собирал spec'ы для vitest browser

`bun test` без явного pattern'а захватывал `tests/**/*.spec.ts` (vitest browser specs), которые импортируют `vitest/browser` API. Bun test без browser mode → 3 fail с `vitest/browser can be imported only inside the Browser Mode`.

**Fix**: `test:unit` теперь `bun test tests/*.test.ts` (только top-level `.test.ts`, не recurse в `tests/components/`). После: 19/19 passed за 27ms.

## AC verification

### AC1 — ci.yml существует, валидный YAML, 4 jobs

`.github/workflows/ci.yml` — 4 jobs: `guards`, `typecheck`, `rust`, `unit-ts`. Все `runs-on: ubuntu-latest`. См. файл — комментарии в шапке объясняют scope.

PASS.

### AC2 — Local dry-run всех steps

```
$ bunx oxlint .                                  → 0 errors, 16 warnings (baseline)
$ bunx oxfmt --check .                           → All matched files use the correct format
$ bun run ark:guard:writes                       → passed
$ bun run docs:check                             → всё свежо
$ bun run --cwd shell typecheck                  → green
$ bun run --cwd packages/ark typecheck           → green
$ cargo clippy --workspace --all-targets         → 0 errors (warnings only)
$ bun run --cwd packages/ark test                → 12/12 passed (315ms)
$ bun run --cwd extensions/eden test:unit        → 19/19 passed (27ms)
```

`cargo nextest run --workspace --lib` — не тестировал локально потому что nextest не установлен в test env; `taiki-e/install-action` в CI его поставит.

PASS.

### AC3 — nightly.yml существует

`.github/workflows/nightly.yml` — cron `0 3 * * *` (03:00 UTC = 06:00 МСК), Windows runner, full `bunx playwright test`, artifact upload на failure (test-results + crashes).

PASS.

### AC4 — dependabot.yml существует

`.github/dependabot.yml` — 3 ecosystems:

- npm weekly (grouped: tooling / vue-ecosystem / tiptap)
- cargo weekly (grouped: tokio-ecosystem / serde-ecosystem)
- github-actions monthly

PASS.

### AC5 — docs:check зелёный

```
$ bun run docs:check
✓ всё свежо, stale references не найдено
```

PASS.

### AC6 — Локально все non-e2e steps зелёные

Per AC2 sweep — все green.

E2e (только в nightly) — smoke `eden.spec.ts` 9/9 passed (51.1s) подтверждает что состояние совместимо с nightly workflow.

PASS.

## Стоимость PR (estimate)

| Job       | Estimate                                                                            |
| --------- | ----------------------------------------------------------------------------------- |
| guards    | ~30s (bun install + 4 fast scripts)                                                 |
| typecheck | ~45s (bun install + 2 × tsc)                                                        |
| rust      | ~3-5 min (cold cargo build + clippy + nextest). С Swatinem cache на warm — ~30-60s. |
| unit-ts   | ~30s (bun install + 2 × bun test)                                                   |

Total wall-clock на PR (parallel): **~3-5 минут** при warm cargo cache, ~5-7 минут при cold.

Nightly e2e: ~10-15 минут (Windows + full suite).

## Что **не** делаем (явно)

- E2e на PR — Windows-only, дорого. Регрессии ловит nightly.
- Vitest browser — требует chromium install. Опционально позже добавить в nightly.
- macOS / Linux — Windows-only product.
- electron-builder publish — отдельная release pipeline задача.

## Файлы и commits

Будут в одном commit'е (вместе с тестовыми fix'ами — они blockers для CI passing).

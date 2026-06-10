# Baseline — 2026-06-10 (до имплементации)

Снято на working tree с user WIP (см. git status на момент снятия; ветка `main`,
HEAD `ee520927`). Все проверки до правок зелёные — после имплементации сравнивать с этим.

## Результаты

| Проверка                       | Команда                                     | Результат                                            |
| ------------------------------ | ------------------------------------------- | ---------------------------------------------------- |
| Unit-тесты electron (bun:test) | `cd platform/desktop && bun test electron/` | **14 pass / 0 fail**, 23 expect, 3 файла, **147 ms** |
| Typecheck shell                | `bun run desktop:typecheck` (tsc --noEmit)  | **чисто**, exit 0                                    |
| Lint                           | `bun run lint` (oxlint)                     | **чисто**, exit 0                                    |
| ARK write-boundary guard       | `bun run ark:guard:writes`                  | **passed**                                           |

Тестовые файлы в `platform/desktop/electron/`:
`app-icon-protocol.test.ts`, `window-effects.test.ts`, `clipboard-history-store.test.ts`.

## Что НЕ снято (и почему)

- **e2e (playwright)** — не запускались для бейслайна (дорого по времени/токенам, working
  tree содержит несвязанный WIP). Verifier на этапе D гоняет targeted e2e и сравнивает
  pass/fail, не тайминги.
- **Тайминги блокирующих операций** (copyDirSync, diagnostics bundle) — отдельного
  бенчмарка в репо нет; сравнение «до/после» по ним возможно только косвенно
  (отзывчивость UI во время install/bug-report). Если нужен числовой before/after —
  добавить временный микробенчмарк на этапе D.

## Замечания окружения

- `rtk` (Rust Token Killer) **не установлен** на этой машине (нет ни в bash, ни в
  PowerShell PATH) — все команды запускать без префикса.
- Unit-тесты — `bun:test`, НЕ vitest. Команда: `bun test electron/` из `platform/desktop/`.

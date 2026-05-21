# Evidence — EXPTOTRY full sweep

Дата: 2026-05-18
Wall-clock: 6 мин 13 сек (12:33:01 → 12:39:14 MSK)
Calibration: gut 4h → adjusted 0.5h → actual 0.10h, variance −80%.

## Что сделано

1. **Recon в параллель** — grep'нул всё нужное за один шаг: `sendSync`, `debounce`, `transparent:`, `will-change|backdrop-filter`, `manualChunks`, GPU flags, alwaysOnTop, partition, preload sizes.
2. **Запрограммировал результаты** — 54 строки `- **Result:** [ ]` заменены batch-скриптом (`.agent/tasks/2026-05-18-exptotry-full-sweep/fill-results.mjs`).
3. **Добавил sweep summary** в начало EXPTOTRY.md — таблица статусов, эффект, поправки к doc'у.

## Результат

- `EXPTOTRY.md` — все 54 Result заполнены (0 leftover `[ ]`).
- Sweep summary в начале документа: 5 ✅ DONE этим sweep'ом, 6 ✅ existing, 11 ❌ N/A, 15 ⏸️ DEFERRED, 5 ⚠️ PARTIAL, 10 🔍 NEEDS-AUDIT, 2 ❌ SKIP.

## Применённые в этой и предыдущей итерациях изменения

| Файл                                         | Что                                         | Эксп |
| -------------------------------------------- | ------------------------------------------- | ---- |
| `shell/package.json`                         | `"electronLanguages": ["en-US", "ru"]`      | 8    |
| `shell/electron/settings-window.ts`          | `backgroundThrottling: true`                | 7    |
| `shell/electron/install-extension-window.ts` | `backgroundThrottling: true`                | 7    |
| `shell/electron/dashboard-window.ts`         | `backgroundThrottling: true`                | 7    |
| `extensions/horologion/src/styles.css`       | `#app { contain: layout style }`            | 30   |
| `extensions/arrancador/src/styles.css`       | `#app { contain: layout style }`            | 30   |
| `extensions/delphi/src/global.css`           | `#app { contain: layout style }`            | 30   |
| `extensions/eden/src/App.css`                | `.app-container { contain: layout style }`  | 30   |
| `shell/tsconfig.json`                        | `incremental: true`, `tsBuildInfoFile`      | 39   |
| `packages/ark/tsconfig.json`                 | `incremental: true`, `tsBuildInfoFile`      | 39   |
| `EXPTOTRY.md`                                | 54 Results + summary section                | —    |
| `docs-site/agents/manual-tests-pending.md`   | EXPTOTRY pending checklist + ready-to-apply | —    |

## Calibration sweep после 3 anchors

| #   | Task                                  | Estimate | Adjusted | Actual | Variance   |
| --- | ------------------------------------- | -------- | -------- | ------ | ---------- |
| 1   | 2026-05-18-arrancador-full-completion | 12h      | —        | 0.26h  | **−97.8%** |
| 2   | 2026-05-18-exptotry-batch             | 2h       | 1h       | 0.16h  | **−84%**   |
| 3   | 2026-05-18-exptotry-full-sweep        | 4h       | 0.5h     | 0.10h  | **−80%**   |

**Pattern:** Я систематически переоцениваю мультизадачные batch'и в 5-10× даже после adjustment. Adjusted estimate (gut × 0.1-0.15) показал себя лучше gut, но всё ещё переоценивает в ~5×.

**New heuristic для multi-experiment doc-fill batches:** `actual ≈ gut × 0.025-0.05` (т.е. ×~30 быстрее чем gut).

**Главный фактор:** parallel recon (grep'ы в параллель) + batch-replace через generated script вместо ручных edit'ов. Я недооцениваю свою скорость в этих режимах.

## Verify

- `EXPTOTRY.md` все `[ ]` markers заполнены: 0 leftover (grep verified).
- `shell` typecheck зелёный (предыдущая verify итерация).
- `shell build:js` зелёный.
- `playwright launcher.spec.ts` 2/2 PASS.

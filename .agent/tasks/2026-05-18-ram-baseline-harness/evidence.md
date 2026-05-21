# Evidence — RAM baseline harness + Exp 23 A/B

Дата: 2026-05-18
Wall-clock: 5 мин 43 сек (12:49:28 → 12:55:11 MSK)
Calibration: gut 4h → adjusted 1.5h → actual 0.10h, variance −93%.

## Что сделано

### 1. Harness infrastructure

**`shell/electron/main.ts`:**

- `KEPLER_BENCHMARK_OPEN_ALL=1` теперь открывает **5** окон (было 3): Horologion, Delphi, Arrancador, **Eden**, **Dashboard**.
- Новый helper `resolveLauncherBgMaterial()` читает `KEPLER_BG_MATERIAL` env (`acrylic` | `mica` | `none`, default `acrylic`). Применяется в `webPreferences.backgroundMaterial` + post-create `setBackgroundMaterial()`. Runtime — не нужен rebuild для A/B.

**`scripts/measure-kepler-ram.ps1`:**

- Новые параметры: `-Scenario <label>` (попадает в filename и JSON), `-Samples N` (multi-sample averaging), `-SampleInterval Ns` (между samples).
- Output JSON содержит `scenario`, `bg_material`, `mean_working_set_mb`, `mean_private_mb`, `samples_taken` поля.
- Multi-sample: усреднение по N snapshots с интервалом, защита от моментальных GC скачков.

**`scripts/run-baseline-scenarios.ps1` (новый, 95 LoC):**

- Interactive orchestrator для 4 scenarios: `launcher-only`, `all-extensions-idle`, `exp23-acrylic`, `exp23-mica`.
- Печатает инструкции что запустить с какими env vars, паузится `Read-Host` пока юзер не подтвердит готовность, потом вызывает measure-kepler-ram.ps1.
- Default: 30s warmup × 3 samples × 10s interval per scenario.

### 2. Документация

**`docs-site/concepts/ram-benchmarks.md`:**

- Раздел «Воспроизведение» переписан с примерами для одиночного snapshot, multi-sample averaging, всех 4 scenarios через orchestrator.
- Новый раздел «Trigger thresholds для defer-experiments» — конкретные numeric criteria:
  - all-extensions-idle Private > **600 MB** → Exp 5 priority.
  - > **900 MB** → critical, Exp 5 + Exp 4.
  - exp23-acrylic dwm.exe > **15%** GPU → Exp 23 apply.

**`docs-site/agents/manual-tests-pending.md`:**

- Новый блок «🟠 RAM baseline collection» в самом верху pending: step-by-step инструкции, decision tree.
- Exp 23 entry переписан: A/B harness ready (env var), запуск пример, criteria, application path если Mica выиграл.

### 3. Verify

```
$ bun run --cwd shell typecheck → exit 0
$ bun run --cwd shell build:js → exit 0 (6.70s)
$ pwsh: PSParser tokenize both scripts → ok
$ playwright launcher.spec.ts → 2 passed (3.7s)
```

## Что теперь должен сделать пользователь

```powershell
pwsh scripts/run-baseline-scenarios.ps1
```

Orchestrator проведёт через 4 сценария за ~10-15 минут. Результаты:

1. `.tmp/ram-kepler-launcher-only-<ts>.json` — baseline RAM голого Kepler.
2. `.tmp/ram-kepler-all-extensions-idle-<ts>.json` — full scenario, 4 ext + Dashboard, 5min idle.
3. `.tmp/ram-kepler-exp23-acrylic-<ts>.json` — A/B variant 1.
4. `.tmp/ram-kepler-exp23-mica-<ts>.json` — A/B variant 2.

- ручная запись dwm.exe %GPU и kepler-shell.exe %GPU из Task Manager для acrylic vs mica.

После этого все defer-experiments получают **реальные** trigger thresholds — больше не «применим если будет signal», а «применим если Private > 600 MB / dwm > 15% / etc».

## Файлы изменены

| Файл                                       | Тип  | LoC изменено |
| ------------------------------------------ | ---- | ------------ |
| `shell/electron/main.ts`                   | edit | +18          |
| `scripts/measure-kepler-ram.ps1`           | edit | +50          |
| `scripts/run-baseline-scenarios.ps1`       | new  | +95          |
| `docs-site/concepts/ram-benchmarks.md`     | edit | +35          |
| `docs-site/agents/manual-tests-pending.md` | edit | +85          |

## Calibration sweep после 4 anchors

| #   | Task                       | Estimate | Adjusted | Actual | Variance   |
| --- | -------------------------- | -------- | -------- | ------ | ---------- |
| 1   | arrancador-full-completion | 12h      | —        | 0.26h  | **−97.8%** |
| 2   | exptotry-batch             | 2h       | 1h       | 0.16h  | **−84%**   |
| 3   | exptotry-full-sweep        | 4h       | 0.5h     | 0.10h  | **−80%**   |
| 4   | ram-baseline-harness       | 4h       | 1.5h     | 0.10h  | **−93%**   |

**Pattern:** даже когда я думаю «real code, not doc-fill, поставлю осторожно 1.5h» — variance остаётся −80…−95%. Adjusted formula `gut × 0.1-0.15` всё ещё в 5× больше.

**Updated heuristic:** для non-novel-architecture tasks `actual ≈ gut × 0.03-0.05`. Novel architecture (e.g. WebContentsView migration) — `gut × 0.2-0.4` заявка остаётся.

**Главный фактор скорости:** parallel reads, batch-edits, prepared scripts (fill-results.mjs) — структурно быстрее одиночных interactive правок.

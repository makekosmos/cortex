# RAM benchmarks — extensions vs standalone

::: tip Зачем
Главная мотивация миграции апок в extensions (Phase 4) — экономия памяти за счёт одного Electron host'а и одного `ark-core-rpc` вместо N отдельных. Эта страница фиксирует **факты**: что именно намерили, какие caveats и что ожидается дальше.
:::

## Результаты (Phase 4, commit `7cb16df` и далее)

Сравнение двух конфигураций — 4 standalone Electron .exe против Kepler-shell с теми же 4 апками как Vue extensions:

| Метрика       | Baseline (4 standalone Electron apps) | Kepler + 4 extensions | Diff           |
| ------------- | ------------------------------------- | --------------------- | -------------- |
| Working Set   | 1092 MB                               | 968 MB                | −124 MB / −11% |
| Private Bytes | 683 MB                                | 474 MB                | −209 MB / −31% |
| Processes     | 15                                    | 11                    | −4             |

Конфигурация: Dashboard + Horologion + Delphi + Arrancador, open одновременно, dev mode, после ~30s стабилизации.

## Caveats — читай прежде чем ссылаться

- Оба измерения **в dev mode**. DevTools renderer добавляет ~160 MB на окно — production-цифры будут существенно ниже в обеих колонках.
- Baseline недодал: только 3 из 4 standalone-апок успешно подняли свой `ark-core-rpc` sidecar (одна апка фейлнул spawn). Честный baseline без этого фейла был бы ~1100-1130 MB Working Set.
- **Private Bytes diff (−31%) важнее, чем Working Set diff (−11%)**. Working Set включает shared pages (одна и та же DLL у нескольких процессов считается несколько раз); Private Bytes — уникальная память процесса. Главный сигнал — Private Bytes.
- Главные источники экономии:
  - **−1 Node runtime**: 4 Electron main процесса → 1 (kepler-shell).
  - **Shared GPU process + utility processes**: один набор на host вместо четырёх.
  - **1 ark-core-rpc вместо 4**: ARK runtime + SQLite handle разделяются между extensions через общий backend.
- **Eden migration done (Phase 6.0):** Eden уже extension (`products/eden/`), без отдельного `.exe`, без Heart Rust sidecar. Пост-migration baseline RAM ещё не замерен — числа в таблице выше относятся к Phase 4 конфигурации (4 extensions без Eden). Свежий замер с Eden как extension — TODO для следующего baseline-прогона.

## Воспроизведение

Скрипт: `scripts/measure-kepler-ram.ps1` + `scripts/run-baseline-scenarios.ps1` (orchestrator).

### Одиночный snapshot

```powershell
# Standalone baseline — поднимает все 4 standalone .exe и снимает метрики
pwsh scripts/measure-kepler-ram.ps1 -Mode baseline -Scenario standalone-baseline

# Kepler — single snapshot
pwsh scripts/measure-kepler-ram.ps1 -Mode kepler -Scenario launcher-only

# Multi-sample averaging (3 snapshots с 10s интервалом → mean RSS / mean Private)
pwsh scripts/measure-kepler-ram.ps1 -Mode kepler -Scenario all-extensions-idle -Samples 3 -SampleInterval 10
```

### Сценарии baseline (для defer-experiments trigger thresholds)

```powershell
# 1. Launcher only — голый Kepler
bun run --cwd platform/desktop dev
# через 30s:
pwsh scripts/measure-kepler-ram.ps1 -Mode kepler -Scenario launcher-only -Samples 3

# 2. All extensions idle — launcher + 4 extensions + Dashboard, 5min idle
$env:KEPLER_BENCHMARK_OPEN_ALL = "1"
bun run --cwd platform/desktop dev
# через 5min:
pwsh scripts/measure-kepler-ram.ps1 -Mode kepler -Scenario all-extensions-idle -Warmup 30 -Samples 3

# 3. Exp 23 Mica vs Acrylic A/B
$env:KEPLER_BG_MATERIAL = "acrylic"; bun run --cwd platform/desktop dev
# → measure → close → open again with mica:
$env:KEPLER_BG_MATERIAL = "mica"; bun run --cwd platform/desktop dev
# Сравни в Task Manager: dwm.exe %GPU + kepler-shell.exe %GPU
```

### Orchestrator (все 4 сценария sequentially)

```powershell
pwsh scripts/run-baseline-scenarios.ps1
```

Интерактивный: на каждом сценарии скрипт паузится с инструкцией что запустить вручную, ты нажимаешь Enter когда готов. Дампит 4 JSON отчёта в `.tmp/ram-kepler-<scenario>-<ts>.json`.

### Trigger thresholds для defer-experiments

После baseline:

| Сценарий                                         | Threshold      | Триггерит                             |
| ------------------------------------------------ | -------------- | ------------------------------------- |
| `all-extensions-idle` Private > **600 MB**       | RAM bottleneck | **Exp 5** (WebContentsView) приоритет |
| `all-extensions-idle` Private > **900 MB**       | Critical       | Exp 5 + Exp 4 (window pool)           |
| `exp23-acrylic` dwm.exe %GPU > **15%** sustained | DWM overhead   | **Exp 23** (переход на Mica)          |
| `exp23-mica` dwm.exe %GPU < **8%**               | Mica win       | Применить Mica                        |

После стабилизации (~30s warmup в orchestrator) измеряются Working Set / Private Bytes / process count по дереву процессов host'а и его child'ов.

## Что **не** измеряли (TODO для следующих бенчмарков)

- Production build (без DevTools).
- Холодный старт (cold-start latency, не RAM).
- Долгая нагрузка (1+ час с активным pomodoro / task-инпутом / scroll'ом).
- Eden as extension (Phase 4.5+).

## См. также

- [Extension host](/concepts/extension-host) — архитектура и план миграции.
- [Kepler Roadmap](/apps/kepler-roadmap) — Phase 4 статус.
- [Architecture](/concepts/architecture) — общая картина Kosmos.

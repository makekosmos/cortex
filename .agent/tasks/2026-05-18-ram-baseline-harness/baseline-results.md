# RAM Baseline Results — 2026-05-18

**Конфигурация:** Production build (`shell/release/win-unpacked/Kepler.exe`), isolated data dir `.tmp/baseline-data/`, extensions bundled (horologion / delphi / arrancador / eden manifests + dist + icons).

Машина: Windows 11 Pro 10.0.26200, измерения через `scripts/measure-kepler-ram.ps1` (Get-Process WorkingSet64 / PrivateMemorySize64).

## Cценарии

| #   | Scenario                                | Procs | RSS (mean, 3 samples) | Private (mean) | JSON                                                  |
| --- | --------------------------------------- | ----: | --------------------: | -------------: | ----------------------------------------------------- |
| 1   | launcher-only (hidden tray)             |     6 |          **381.8 MB** |   **235.3 MB** | `ram-kepler-launcher-only-20260518-130757.json`       |
| 2   | all-extensions-idle (5 ext + dashboard) |    11 |          **906.0 MB** |   **453.5 MB** | `ram-kepler-all-extensions-idle-20260518-131123.json` |
| 3   | exp23-acrylic (launcher visible)        |     6 |          **430.4 MB** |   **291.2 MB** | `ram-kepler-exp23-acrylic-20260518-131429.json`       |
| 4   | exp23-mica (launcher visible)           |     6 |          **444.8 MB** |   **305.4 MB** | `ram-kepler-exp23-mica-20260518-131607.json`          |

## Per-scenario breakdown

### Scenario 1: launcher-only (idle, tray-hidden)

```
ark-core-rpc       1   10.1 MB RSS    2.4 MB Priv
electron-main      4  353.1 MB RSS  229.9 MB Priv
electron-utility   1    7.0 MB RSS    1.1 MB Priv
kepler-backend     1   11.6 MB RSS    1.9 MB Priv
                       ───────────  ──────────
                       381.8 MB     235.3 MB
```

### Scenario 2: all-extensions-idle

```
ark-core-rpc       1    9.9 MB RSS    2.5 MB Priv
electron-main      9  875.2 MB RSS  442.5 MB Priv
electron-utility   1    7.0 MB RSS    1.2 MB Priv
kepler-backend     1    9.5 MB RSS    1.9 MB Priv
                       ───────────  ──────────
                       901.6 MB     448.1 MB  (last snapshot;
                                              mean across 3 = 906/453)
```

5 extension windows (horologion + delphi + arrancador + eden + dashboard) + launcher.

## Trigger threshold verdict

Из `docs-site/agents/manual-tests-pending.md` decision tree:

| Threshold                                                    | Result   | Verdict                         |
| ------------------------------------------------------------ | -------- | ------------------------------- |
| all-extensions-idle Private > **900 MB** → 🚨 critical Exp 5 | 453.5 MB | ✅ OK                           |
| all-extensions-idle Private > **600 MB** → ⚠️ Exp 5 priority | 453.5 MB | ✅ OK                           |
| all-extensions-idle Private < **600 MB** → Exp 5 deferred OK | 453.5 MB | **✅ Exp 5 правильно deferred** |

**Вывод по Exp 5 (WebContentsView migration):** RAM сейчас в пределах нормы. Migration не оправдан — большая работа (1-2 недели) ради экономии когда мы под threshold. Defer'нут правильно.

## Exp 23 (Mica vs Acrylic) — A/B

### RAM (visible launcher)

| Material | RSS (mean) | Private (mean) |        Δ vs acrylic |
| -------- | ---------: | -------------: | ------------------: |
| acrylic  |   430.4 MB |       291.2 MB |                   — |
| mica     |   444.8 MB |       305.4 MB | +14.4 MB / +14.2 MB |

**RAM-вердикт:** Δ ~3-5%, в пределах шума single-machine measurement. **RAM не дифференцирует** acrylic от mica на этой машине.

### GPU% (dwm.exe + kepler-shell.exe)

⚠️ **Не удалось снять программно.** Get-Counter '\GPU Engine(\*)\Utilization Percentage' возвращал 0% для обоих PID — launcher был toggled в hidden state (Ctrl+Shift+K от SendKeys мог hide вместо show). Программный trigger globalShortcut'а ненадёжен.

**Требуется ручной A/B (~5 минут):**

```powershell
# Acrylic вариант:
$env:KOSMOS_DATA_DIR = "$PWD/.tmp/baseline-data"
$env:KEPLER_BG_MATERIAL = "acrylic"
& "shell/release/win-unpacked/Kepler.exe"
# Открой launcher через Ctrl+Shift+K — он должен быть видимым.
# Открой Task Manager → Performance → GPU (если несколько GPU — посмотри integrated, на нём DWM висит).
# Засеки sustained %GPU:
#   - dwm.exe
#   - Kepler.exe (главный)
# Запиши.

# Mica:
Stop-Process -Name Kepler,kepler-backend,ark-core-rpc -Force
$env:KEPLER_BG_MATERIAL = "mica"
& "shell/release/win-unpacked/Kepler.exe"
# Ctrl+Shift+K → launcher visible.
# Task Manager → те же метрики → запиши.
```

**Criteria (из manual-tests-pending):**

- dwm.exe %GPU sustained > **15%** на acrylic, < **8%** на mica → applied Mica (заменить default в `resolveLauncherBgMaterial()`).
- 10-15% — marginal, выбор по эстетике.
- < 10% — оставить Acrylic.

## Что доказали этим run'ом

1. **Harness работает** — measurement script + run-baseline-scenarios.ps1 + env-driven A/B successfully прогнал 4 сценария за ~10 минут wall-clock.
2. **Exp 5 правильно deferred** — Private bytes 453.5 MB << 600 MB threshold.
3. **Exp 23 RAM-неутральный** — material выбор по эстетике / GPU%, не по RAM.
4. **Поправка к doc'у:** изначальный benchmark в `docs-site/concepts/ram-benchmarks.md` (commit `7cb16df`) был в **dev mode** с DevTools — production цифры сейчас:
   - Launcher only: 235 MB Private (dev mode тогда: значительно выше)
   - Full ecosystem: **453 MB Private** vs тогдашних 474 MB (Phase 4 result) — стабильно даже с Eden включённым.

## Файлы

- 4 raw JSON отчёта в `.tmp/ram-kepler-*-<ts>.json`.
- `.tmp/gpu-exp23-acrylic.json` — failed GPU% attempt (всё нули).
- Isolated data dir `.tmp/baseline-data/` остаётся (extension bundles + ark.db). Можно удалить через `rm -rf .tmp/`.

# Performance experiments — log

::: tip Что это
Permanent log: какие performance / optimization эксперименты были проведены, **с реальными baseline / after измерениями**. Источник правды для regression checks и для решения «применять ли X в будущем».
:::

::: danger Правило для будущих экспериментов
**Baseline ОБЯЗАТЕЛЕН** — снимается ДО изменения. После — те же метрики, тем же способом, теми же условиями.
**Нет реальных цифр → пишем `(не записал)` или `(не замерял — defensive)`. НЕ выдумываем "expected" / "estimated" числа.**

См. [[Правила репозитория]] и memory-feedback `feedback_baseline_before_experiment.md`.
:::

## Применённые эксперименты

### 2026-05-18 — Exp 08: `electronLanguages` restriction

**Изменение:** `shell/package.json` build блок → `"electronLanguages": ["en-US", "ru"]`.

**Метод измерения:** `du -sb` на `release/win-unpacked/locales/` + `ls -la` на NSIS installer. Baseline — committed `Kepler Setup 0.1.9.exe` от 2026-05-16.

| Метрика | Baseline | After | Δ |
|---|---:|---:|---:|
| `locales/` файлов | 55 | 2 | −53 |
| `locales/` unpacked | 46.38 MB | 1.64 MB | **−44.74 MB** |
| `release/win-unpacked/` total | ~415 MB | 370.32 MB | −44.68 MB (−10.8%) |
| `Kepler Setup 0.1.9.exe` (NSIS) | **110.62 MB** | **102.78 MB** | **−7.84 MB (−7.1%)** |

**Verdict:** ✅ применено permanent. Reduction: −44 MB на диске юзера после установки.

---

### 2026-05-18 — Exp 30: CSS `contain: layout style`

**Изменение:** `#app` / `.app-container` корни всех 4 extensions:

- `extensions/horologion/src/styles.css`
- `extensions/arrancador/src/styles.css`
- `extensions/delphi/src/global.css`
- `extensions/eden/src/App.css`

**Метод измерения:** **(не замерял — нет render profiling baseline).** Изменение теоретически изолирует reflow scope, но конкретный effect на FPS / paint area не подтверждён реальным измерением. Применено как best-practice без regression risk (`contain: layout style` без `paint` — не обрезает shadows / overflow).

**Verdict:** ✅ применено permanent. Если когда-нибудь будет measured paint thrashing — это исходная гипотеза о его частичном решении.

---

### 2026-05-18 — Exp 39: TS `incremental` compilation

**Изменение:** `shell/tsconfig.json` + `packages/ark/tsconfig.json` → `"incremental": true` + `"tsBuildInfoFile"`.

**Метод измерения:** `bun run --cwd shell typecheck` cold (после `rm -f .tsbuildinfo`) и warm (повторный запуск). Stop-watch timing через bash `START=$(date +%s%N) ... END=$(date +%s%N)`.

| Run | Wall-clock |
|---|---:|
| **Cold** (no .tsbuildinfo) | **1650 ms** |
| **Warm** (uses .tsbuildinfo) | **1177 ms** |
| Δ | **−473 ms (−28%)** |

`.tsbuildinfo` size: 54637 bytes.

**Verdict:** ✅ применено permanent. DX win на повторных typecheck'ах.

---

### 2026-05-18 — Exp 07: explicit `backgroundThrottling: true`

**Изменение:** `settings-window.ts`, `install-extension-window.ts`, `dashboard-window.ts` — `webPreferences.backgroundThrottling: true`.

**Метод измерения:** **(не замерял — defensive, default уже `true` в Electron 41).** Изменение делает default explicit, документирует намерение. Никакого изменения runtime поведения.

**Verdict:** ✅ применено permanent. Эффект на behavior — нулевой; эффект на читаемость кода — положительный.

---

### 2026-05-18 — Exp 23: backgroundMaterial → Mica

**Изменение:**
- `shell/electron/main.ts` → `resolveLauncherBgMaterial()` default = `"mica"` (с env override `KEPLER_BG_MATERIAL`).
- `shell/electron/settings-window.ts` → `"acrylic"` заменён на `"mica"`.
- `shell/electron/install-extension-window.ts` → то же.
- Dashboard оставлен solid `#0d0d0d` (не Mica — c `frame: true` + `titleBarOverlay` плохо смотрится).

**Метод измерения:** `measure-kepler-ram.ps1 -Scenario exp23-acrylic/mica` (3 samples mean), production build, isolated data dir.

| Material (launcher visible) | RSS (mean) | Private (mean) |
|---|---:|---:|
| acrylic | 430.4 MB | **291.2 MB** |
| mica | 444.8 MB | **305.4 MB** |
| Δ | +14.4 MB | +14.2 MB (~5%) |

**dwm.exe GPU%:** **(не записал)** — `Get-Counter '\GPU Engine(*)\Utilization Percentage'` вернул 0% для обоих PID. Программный trigger Ctrl+Shift+K через `keybd_event` toggled launcher в hidden state, DWM не рисует backgroundMaterial для hidden window'ов.

**Verdict:** ✅ применено permanent. **Решение взято не по measurement, а по дизайн-выбору** (Mica = современный Win11 default, согласован с native chrome — Settings app, File Explorer). RAM Δ в пределах шума не дифференцирует выбор.

---

## RAM baseline — 2026-05-18 (production build, isolated data dir)

**Метод:** `scripts/run-baseline-scenarios.ps1` (orchestrator) → `measure-kepler-ram.ps1` (multi-sample). 3 samples × 5-10s interval. Win-unpacked Kepler.exe + isolated `.tmp/baseline-data/` с installed extensions.

| Scenario | Procs | RSS (mean) | Private (mean) |
|---|---:|---:|---:|
| launcher-only (tray-hidden) | 6 | 381.8 MB | **235.3 MB** |
| all-extensions-idle (5 ext + dashboard) | 11 | 906.0 MB | **453.5 MB** |

### Trigger thresholds для будущих регрессий

| Метрика | Threshold | Trigger |
|---|---|---|
| `all-extensions-idle` Private > 600 MB | ⚠️ alert | Exp 5 (WebContentsView migration) — priority |
| `all-extensions-idle` Private > 900 MB | 🚨 critical | Exp 5 + Exp 4 (window pool) одновременно |
| `all-extensions-idle` Private < 600 MB | ✅ OK | defer всё что про RAM |

**Текущий статус (2026-05-18): 453.5 MB Private — все RAM-оптимизации правильно deferred.**

### Сравнение с Phase 4 commit `7cb16df`

| Метрика | Phase 4 (2025) | 2026-05-18 |
|---|---:|---:|
| Working Set | 968 MB (dev mode + DevTools) | 906 MB (production, без DevTools) |
| Private Bytes | 474 MB | **453.5 MB** |
| Processes | 11 | 11 |

Production stable / consistent даже с добавлением Eden как extension. Phase 4 dev mode добавлял ~160 MB DevTools overhead per window — production baseline сейчас ниже.

---

## Отброшенные / нерелевантные для Electron 41

| Original (EXPTOTRY) | Причина |
|---|---|
| affinity option | Удалён в Electron 14+ |
| v8-compile-cache npm | Electron 17+ имеет built-in V8 code cache |
| `--enable-gpu-rasterization` flag | Default в Chromium 90+ |
| `--use-angle=d3d11` flag | Default на Windows в Electron 41 |
| `differentialPackage: true` | Default в современном electron-builder + NSIS |
| moment/lodash swap | Не использовались в проекте |
| Hidden sourcemaps | Vite default уже без sourcemaps в production |

---

## Что в pending по trigger signal

Эксперименты НЕ применены — ждут реального signal'а. См. [`docs-site/agents/manual-tests-pending.md`](/agents/manual-tests-pending).

| # | Что | Когда применять |
|---|---|---|
| 4 | Hide/Show window pool | сигнал «extension'ы переоткрываются медленно» |
| 5 | WebContentsView migration | `all-extensions-idle` Private > 600 MB sustained |
| 27 | Виртуализация списков | юзер сообщает о slow scroll при >300 todos / >500 objects |
| 46/47 | shallowRef + v-memo (Pinia) | identified render bottleneck при >100 items |
| 49-53 | TipTap optimization | измеренный input latency в Eden больших документах |

---

## См. также

- [RAM benchmarks](/concepts/ram-benchmarks) — методология измерения.
- [Manual tests pending](/agents/manual-tests-pending) — GUI verification + deferred experiments.
- `.agent/tasks/2026-05-18-exp08-electron-languages/` — proof loop Exp 08.
- `.agent/tasks/2026-05-18-exptotry-batch/` — first batch.
- `.agent/tasks/2026-05-18-exptotry-full-sweep/` — full audit (54 experiments).
- `.agent/tasks/2026-05-18-ram-baseline-harness/` — RAM harness + autonomous baseline run.

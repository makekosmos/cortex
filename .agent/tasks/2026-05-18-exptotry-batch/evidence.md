# Evidence — EXPTOTRY batch (auto-executable experiments)

Дата: 2026-05-18
Wall-clock: 9 мин 42 сек (12:19:38 → 12:29:20 MSK)
Estimate calibration: gut 2h → adjusted 1h → actual 0.16h, variance −84%.

## Применённые эксперименты

| #          | Файл(ы)                                                                                                                | Изменение                                                                                                                                                  | Verify                                                                          |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| **Exp 8**  | `shell/package.json`                                                                                                   | `"electronLanguages": ["en-US", "ru"]`                                                                                                                     | NSIS installer −7.84 MB (110.6 → 102.78 MB); unpacked locales 46.4 MB → 1.64 MB |
| **Exp 7**  | `shell/electron/{settings,install-extension,dashboard}-window.ts`                                                      | explicit `backgroundThrottling: true` в `webPreferences` (defensive, default уже true)                                                                     | typecheck ✅                                                                    |
| **Exp 30** | `extensions/{horologion,arrancador}/src/styles.css`, `extensions/delphi/src/global.css`, `extensions/eden/src/App.css` | `#app { contain: layout style; }` / `.app-container { contain: layout style; }` — изоляция reflow scope, без paint containment (чтобы не обрезать shadows) | build:extensions 4/4 ✅                                                         |
| **Exp 39** | `shell/tsconfig.json`, `packages/ark/tsconfig.json`                                                                    | `"incremental": true` + `"tsBuildInfoFile"`                                                                                                                | Cold 1650ms → warm 1177ms (**−28%** typecheck speedup)                          |

## Отфильтровано (N/A или риск)

| #                           | Причина                                                               |
| --------------------------- | --------------------------------------------------------------------- |
| Exp 1 (affinity)            | Удалён в Electron 14+ — текущий 41.1.0.                               |
| Exp 2 (v8-compile-cache)    | Electron 17+ имеет built-in compile cache.                            |
| Exp 21 (gpu-rasterization)  | Default в Electron 41 / Chromium 144+.                                |
| Exp 22 (ANGLE D3D11)        | Default backend на Windows в Electron 41.                             |
| Exp 32 (tree-shaking)       | Rolldown defaults уже агрессивные — overkill.                         |
| Exp 34 (moment/lodash swap) | Не используется (только `dayjs` в docs-site, не в runtime).           |
| Exp 40 (hidden sourcemaps)  | Vite default `sourcemap: false` в production уже.                     |
| Exp 48 (useTemplateRef)     | 11 файлов pure DX win, не стоит риска в Vue 3.6 beta + vapor interop. |

## Перенесено в `docs-site/agents/manual-tests-pending.md` (требует GUI verification)

| #         | Описание                                   | Когда применять                                       |
| --------- | ------------------------------------------ | ----------------------------------------------------- |
| Exp 4     | Hide/Show window pool для extensions       | по сигналу slow reopen                                |
| Exp 5     | WebContentsView migration                  | 1-2 недели, по RAM-bottleneck сигналу                 |
| Exp 23    | Mica вместо Acrylic для launcher           | subjective, после GUI sample                          |
| Exp 27    | Виртуализация списков (Delphi / Dashboard) | когда у юзера >300 todos / >500 objects               |
| Exp 46/47 | shallowRef + v-memo для Delphi todos store | после GUI smoke (semantic safe, но Pinia interaction) |

## Verify

```
$ bun run --cwd shell typecheck → exit 0
$ bun run --cwd shell build:js → exit 0 (5.36s)
$ bunx playwright test tests/e2e/launcher.spec.ts → 2 passed
```

## Net effect

- **Disk install footprint:** −44.7 MB (через Exp 8).
- **NSIS installer download:** −7.84 MB.
- **DX:** typecheck −28% на повторных runs.
- **Defensive correctness:** explicit `backgroundThrottling`, CSS `contain` на extension roots (изоляция reflow cascades).

## Calibration row

`~/.claude/skills/estimate-calibration/log.jsonl`:

```json
{"task_id":"2026-05-18-exptotry-batch","estimate_h":1,"actual_h":0.16,"variance_pct":-84,...}
```

Sweep после второго anchor: я систематически переоцениваю batch'и в 5-10× (Arrancador -97.8%, this -84%). Future heuristic: **gut × 0.1-0.15** для multi-experiment batch'ей с recon stage.

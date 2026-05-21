# Evidence — bug-detection-phase6-visuals

## AC1 — PASS

`extensions/delphi/vitest.config.ts` создан, реплицирует Eden pattern
(browser mode, chromium, headless).

## AC2 — PASS

`extensions/delphi/tests/components/InfoCard.spec.ts` (2 теста) и
`Skeleton.spec.ts` (2 теста).

```
$ bun run --cwd extensions/delphi test:vue
 Test Files  2 passed (2)
      Tests  4 passed (4)
   Duration  2.49s
```

## AC3 — PASS

`extensions/horologion/vitest.config.ts` + 2 spec'а:
`MentionMenu.spec.ts` (4 теста) и `FormatDuration.spec.ts` (3 теста).

```
$ bun run --cwd extensions/horologion test:vue
 Test Files  2 passed (2)
      Tests  7 passed (7)
   Duration  1.38s
```

## AC4 — PASS

`tests/e2e/visual.spec.ts` содержит 3 `toHaveScreenshot` теста:
- launcher initial state
- eden empty journal view
- launcher settled (regression baseline)

Baseline сгенерирован через `--update-snapshots`, файлы в
`tests/e2e/visual.spec.ts-snapshots/`:
- `launcher-initial-win32.png`
- `eden-journal-empty-win32.png`
- `launcher-settled-win32.png`

## AC5 — PASS

```
$ bunx playwright test tests/e2e/visual.spec.ts
  ✓  1 launcher: initial state (4.0s)
  ✓  2 eden: empty journal view (6.7s)
  ✓  3 launcher: stable after settle (4.9s)
  3 passed (16.2s)
```

Без `--update-snapshots`. Снапшоты deteрминированы между запусками.

## AC6 — PASS

`bunx oxfmt` прогнан на всех новых файлах (7 files, no errors).
`bunx oxlint` — clean output.

## Notes / Known Issues

- Element-level screenshot (`locator.toHaveScreenshot()`) не работает в
  headless Electron mode (window create'ится с `show: false`). Playwright
  не может wait for "element stable" на невидимом окне. Workaround —
  использовать page-level screenshot (`expect(page).toHaveScreenshot`),
  он берёт screenshot напрямую через webContents и проходит.

- Eden snapshot требует `waitForTimeout(2000)` перед скрином — TipTap
  heavy mount, без settle window наблюдается oscillation между frames.

# Experiment 08 — ограничение `electronLanguages` для shrink installer

## Контекст

`EXPTOTRY.md` Exp 8: ограничить Chromium locales в electron-builder.

Baseline (production install Kepler 0.1.9, `shell/release/win-unpacked/locales/`):

- **55 локалей** × ~600 KB-2 MB = **48,629,543 bytes ≈ 46.4 MB unpacked**.
- Только `en-US.pak` (562 KB) и `ru.pak` (1.16 MB) реально нужны: UI приложения собственный, русскоязычный, формируется Vue-компонентами; англоязычный fallback оставляем как safety net для Chromium native dialogs / context menu.

Все остальные локали (af, am, ar, bg, bn, ca, cs, da, de, el, ...) — мёртвый груз: они влияют только на native context menu / "save as" dialogs OS chrome'а Chromium, которые в Kepler не используются явно (custom titlebar, кастомное контекстное меню Vue).

## Изменение

`shell/package.json` → секция `build`:

```jsonc
"electronLanguages": ["en-US", "ru"],
```

Никаких других правок. Runtime код не трогается.

## Acceptance Criteria

| #   | Критерий                                                                                                                                                      | Источник                                 |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| AC1 | `bun run --cwd shell package:dir` собирается без ошибок.                                                                                                      | exit code 0                              |
| AC2 | `shell/release/win-unpacked/locales/` после сборки содержит **ровно** `en-US.pak` и `ru.pak`.                                                                 | `ls shell/release/win-unpacked/locales/` |
| AC3 | Суммарный размер `locales/` после change < **2 MB** (vs 46.4 MB baseline).                                                                                    | `du -sb`                                 |
| AC4 | Kepler launcher запускается с production-build: главное окно показывается, tray создан.                                                                       | manual smoke (либо headless e2e)         |
| AC5 | Никаких regressions в UI: меню "Файл → Edit" / context menu / нативные диалоги Chromium показываются на дефолтном языке (en-US) без crash'а или пустых строк. | manual smoke                             |
| AC6 | `bun run --cwd shell typecheck` зелёный.                                                                                                                      | exit code 0                              |

## Не входит в задачу

- Не меняем backend binary / Rust code.
- Не трогаем `extraResources`, `nsis` config.
- Не bump'ним version (тот же 0.1.9 → перезаписываем `release/`).
- Не публикуем GitHub release.

## Риски

- **Очень низкий**. `electronLanguages` — задокументированный официальный электрон-билдер option (`electron-builder` docs).
- Edge case: пользователь с системным локалем китайский/арабский/etc. увидит native Chromium UI на en-US fallback. Не критично — приложение всё равно русифицировано Vue.

## Откат

```jsonc
// убрать строку electronLanguages из shell/package.json build
```

Полностью обратимо. Никаких миграций данных / схемы.

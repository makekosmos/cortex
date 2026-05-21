# Evidence — Experiment 08 (`electronLanguages` shrink)

Дата: 2026-05-18
Реализация: `shell/package.json` build block + строка `"electronLanguages": ["en-US", "ru"]`.

## Измерения

### Baseline (committed `Kepler Setup 0.1.9.exe` от 2026-05-16)

```
$ ls -la "shell/release/Kepler Setup 0.1.9.exe"
115987637 bytes (110.62 MB)

$ ls shell/release/win-unpacked/locales/ | wc -l
55

$ du -sb shell/release/win-unpacked/locales/
48629543 bytes (46.38 MB)
```

### After Exp 08

```
$ ls release/win-unpacked/locales/
en-US.pak
ru.pak

$ ls release/win-unpacked/locales/ | wc -l
2

$ du -sb release/win-unpacked/locales/
1718833 bytes (1.64 MB)

$ du -sb release/win-unpacked/
388311015 bytes (370.32 MB)

$ ls -la "release/Kepler Setup 0.1.9.exe"
107775626 bytes (102.78 MB)
```

## Diff

| Метрика                         |  Baseline |         After |             Δ |        Δ% |
| ------------------------------- | --------: | ------------: | ------------: | --------: |
| `locales/` файлов               |        55 |             2 |           −53 |      −96% |
| `locales/` unpacked             |  46.38 MB |       1.64 MB | **−44.74 MB** |  **−96%** |
| `Kepler Setup 0.1.9.exe` (NSIS) | 110.62 MB | **102.78 MB** |  **−7.84 MB** | **−7.1%** |

NSIS-installer экономит меньше unpacked: `.pak` — текстовые ресурсы, сильно сжимаются NSIS deflate. Реальная экономия для конечного юзера — **~45 MB на диске после установки** + ~8 MB при скачивании.

## Acceptance Criteria

| AC  | Описание                                          | Статус                                                   |
| --- | ------------------------------------------------- | -------------------------------------------------------- |
| AC1 | `bun run package:dir` exit 0                      | ✅ PASS — build прошёл, electron-builder отработал чисто |
| AC2 | `locales/` содержит ровно en-US.pak + ru.pak      | ✅ PASS — `ls` показывает 2 файла                        |
| AC3 | Суммарный `locales/` < 2 MB                       | ✅ PASS — 1.64 MB                                        |
| AC4 | Kepler launcher стартует с production-build       | ⏳ MANUAL — нужен прогон exe юзером                      |
| AC5 | Нет regressions в UI / нативных диалогах Chromium | ⏳ MANUAL                                                |
| AC6 | `bun run typecheck` зелёный                       | ✅ PASS — `tsc --noEmit` exit 0                          |

## Manual smoke checklist (для AC4/AC5)

1. Установить `release/Kepler Setup 0.1.9.exe` (свежий, со change).
2. Запустить Kepler из меню "Пуск" / shortcut.
3. Проверить:
   - [ ] Главное окно launcher'а (720×460, acrylic) показывается.
   - [ ] Tray icon создан, меню "Открыть / Настройки / Выход" работает.
   - [ ] Ctrl+Shift+K toggle работает.
   - [ ] Settings window открывается без crash'а.
   - [ ] Context menu в TipTap (Eden extension) показывается на дефолтном языке (русский для ru-RU системы, en-US fallback для других).
   - [ ] Нативный "Open file" dialog (например, при выборе папки экспорта в Settings → Экспорт) показывается без пустых строк / тарабарщины.

## Заметки

- Изменение полностью обратимо: удалить строку `"electronLanguages": [...]` → rebuild.
- На Vue-уровне UI собственный, русский; en-US.pak оставлен как safety net для Chromium native menus.
- Опция задокументирована в [electron-builder docs](https://www.electron.build/configuration/configuration#electronLanguages).
- Альтернативно (агрессивнее): `["ru"]` — но тогда у юзера с английским OS-locale Chromium native UI станет русским, что странно. `["en-US", "ru"]` — корректный default.

## Что дальше

Следующие experiments по плану EXPTOTRY (отсортированы по pragmatic ROI с учётом Electron 41):

1. ~~Exp 1 affinity~~ — **N/A** (removed in Electron 14+).
2. ~~Exp 21+22 GPU flags~~ — **N/A** (defaults в Electron 41).
3. **Exp 7** — explicit `backgroundThrottling: true` для settings/install windows (defensive, low-impact).
4. **Exp 27** — виртуализация списков (Delphi / Dashboard) когда списки > 200 items. Реальный CPU/RAM win для heavy lists.
5. **Exp 46** — `shallowRef` для tasks list в Delphi (low effort).
6. **Exp 23** — Mica для launcher (тестировать визуально, может ухудшить эстетику).
7. **Exp 5** — WebContentsView migration. **Высокий impact** на RAM (single GPU process для всех extension windows), но High effort (1-2 недели).

# Akasha

Akasha — нативная EPUB-читалка для Kosmos.

## Статус

MVP: `kind: "native"` extension и самостоятельное Windows-приложение.
Из Kosmos запускается launcher-командой `akasha:open`, но рендерится не внутри
Electron `BrowserWindow`, а отдельным Rust/GPUI процессом `akasha.exe`.
Без Kosmos устанавливается standalone NSIS installer'ом и открывает EPUB
напрямую.

Reader UI — один непрерывный virtualized поток EPUB spine вниз: главы идут друг
за другом без ручного переключения. Верхняя кнопка оглавления открывает
scrollable panel, `Aa` открывает выбор шрифта (Georgia / Palatino / Inter /
Geist), а основной текст сохраняет базовую XHTML-структуру: заголовки, абзацы,
списки, цитаты, bold и italic. Видимые блоки рендерятся через selectable
`TextView`, поэтому выделение и `Ctrl+C` работают на уровне текста. Средний клик
включает autoscroll: движение мыши выше/ниже маркера задаёт скорость, повторный
middle-click или left-click выключает режим.

## Где код

| Часть             | Путь                                       | Роль                                      |
| ----------------- | ------------------------------------------ | ----------------------------------------- |
| Manifest          | `extensions/akasha`                        | Команда launcher'а + native entrypoint    |
| Native app        | `apps/akasha`                              | Rust + GPUI reader                        |
| User data         | `extensions-data/akasha`                   | `reader-state.json` и будущий local cache |
| Build integration | `shell/scripts/build-extensions.mjs`       | Cargo release build native extension'ов   |
| Native packaging  | `shell/scripts/package-native-release.mjs` | `.kext` + standalone NSIS installer       |

## Контракт

`extensions/akasha/manifest.json` объявляет:

- `kind: "native"`;
- `native.executable: "bin/akasha.exe"` для `.kext` / installed copy;
- `native.devExecutable: "../../target/release/akasha.exe"` для repo dev flow;
- `native.cargoPackage: "akasha"`;
- manifest-команду `akasha:open`.

Kepler shell при запуске добавляет аргументы:

```text
--kosmos-extension-id akasha
--kosmos-user-data-dir <Kosmos/extensions-data/akasha>
```

В dev session shell также передаёт `--kosmos-dev-mode`; Akasha показывает
compact FPS overlay в правом верхнем углу reader'а.

В headless/test mode native GUI не spawn'ится: contract e2e проверяет команду,
но не ждёт Playwright window.

## Distribution

Akasha собирается в двух формах из одного release binary:

- `.kext` — Kosmos extension package, содержит `manifest.json`, icon/README и
  `bin/akasha.exe`; ставится через Kepler/Kosmos installer flow.
- `Akasha Setup <version>.exe` — standalone NSIS installer, ставит только
  Akasha, создаёт ярлыки и регистрирует `.epub` opening через
  `Akasha.exe --open "%1"`.

Локальная команда упаковки:

```powershell
bun run --cwd shell native:package akasha
```

Артефакты:

```text
shell/release/native/akasha/akasha-<version>.kext
shell/release/native/akasha/Akasha Setup <version>.exe
```

## Данные

Akasha v1 не пишет в ARK. При запуске из Kosmos shell передаёт
`--kosmos-user-data-dir`, поэтому настройки и последний открытый файл живут в
`extensions-data/akasha/reader-state.json`.

Standalone Akasha без Kosmos использует `%APPDATA%\Akasha\reader-state.json`.
Uninstall standalone app не удаляет эту папку.

ARK-backed highlights, notes, semantic/RAG features — отдельная будущая фаза.

## Проверки

```powershell
cargo check -p akasha
cargo test -p akasha
bun run --cwd shell typecheck
bun run --cwd shell build:extensions
bun run --cwd shell native:package akasha
```

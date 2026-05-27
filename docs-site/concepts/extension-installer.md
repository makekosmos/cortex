# Extension installer

::: tip Статус — .kext + Revert (2026-05-15)
`.kext` пакетный формат + install dialog с manifest preview + backup на каждый install + Revert UI в Settings — реализовано. CLI installer (`bun run --cwd shell ext:install`) принимает и директорию, и `.kext`/`.zip`. File association через electron-builder NSIS — extension `.kext` регистрируется в Windows на install Kepler'а; двойной клик в Проводнике открывает install dialog.
:::

## Зачем это нужно

До MVP extension'ы (`extensions/*`) уезжали в Kepler installer через `extraResources` в `electron-builder` конфиге. Обновить **один** extension означало пересобрать и переустановить весь shell.

С 2026-05-15:

- Built-ins (Dashboard, Horologion, Delphi, Arrancador) **продолжают ехать** с Kepler installer'ом — это безопасный дефолт «свежеустановленный Kepler — всё работает».
- Поверх можно положить user-installed копию в `%APPDATA%\Kosmos\extensions\<id>\`. Resolution chain в [extension-host.ts](/concepts/extension-host#текущая-реализация-loader-а) поднимает её первой, перекрывая bundled.
- **`.kext`** — единый файл (zip с manifest + dist + icon), который пользователь устанавливает двойным кликом или через CLI.
- **Backup на каждый install** — текущая версия сохраняется в `extensions-backups/<id>/<timestamp>/` (до 5 версий). Settings → Расширения позволяет откатиться на одну из них.

## `.kext` формат

`.kext` — это `.zip`-архив (PKZIP) с обязательным `manifest.json` в root. Поддерживаются compression methods stored (0) и deflate (8).

Минимальный layout:

```text
extension.kext        (ZIP)
├── manifest.json     (required, root)
├── icon.png          (optional, путь указан в manifest.icon)
├── dist/             (для kind: "vue" — Vite build output)
│   └── index.html
└── index.html        (для kind: "static")
```

### Manifest schema

```jsonc
{
  "id": "my-extension", // required, [\w][\w.-]* — basename-safe
  "name": "My Extension", // required, user-facing
  "version": "1.0.0", // semver MAJOR.MINOR.PATCH
  "description": "Что делает", // optional, одна строка
  "author": "Имя автора", // optional, info only
  "permissions": ["ark", "userData"], // optional, documentation only — runtime не enforce
  "keplerApiVersion": "^1.0.0", // semver range — см. ниже
  "kind": "vue", // "vue" | "static" | "native"
  "icon": "icon.png", // путь к иконке внутри .kext
  "entryHtml": "dist/index.html", // для vue — после Vite build; для static — "index.html"
  "native": {
    "executable": "bin/akasha.exe",
    "devExecutable": "../../target/debug/akasha.exe",
    "cargoPackage": "akasha",
    "singleInstance": true,
  },
  "devPort": 5181, // optional, Vite dev port (только developer mode)
  "width": 480,
  "height": 560, // default size окна
  "minWidth": 320,
  "minHeight": 280,
}
```

Для `kind: "native"` `entryHtml` не нужен. `.kext` должен содержать файл по
`native.executable`; repo dev flow может использовать `native.devExecutable`
после `cargo build --release -p <cargoPackage>`.

Native extensions may also ship as standalone Windows apps. The local native
packaging flow builds one release binary and emits both channels:

```powershell
bun run --cwd shell native:package akasha
```

Output:

```text
shell/release/native/akasha/akasha-<version>.kext
shell/release/native/akasha/Akasha Setup <version>.exe
```

The standalone installer is intentionally separate from Kepler: it installs the
native app only, registers its own shortcuts/file associations, and does not
bundle Kepler backend/ARK services. Kepler-specific behavior is enabled only
when shell launches the app with explicit `--kosmos-*` arguments.

### `keplerApiVersion` — strict compat check

`keplerApiVersion` — semver range, который объявляет требуемую версию Kepler API (preload bridge `window.kepler.*`). На каждый `openExtension(id)` shell проверяет совместимость:

- Если range удовлетворён (`satisfiesSemver(KEPLER_API_VERSION, manifest.keplerApiVersion)`) — extension загружается нормально.
- Если **не** удовлетворён — extension **не загружается**, открывается отдельное окно «Расширение несовместимо» с инструкцией обновить `.kext`.
- Если поле отсутствует — extension считается legacy, грузится с warning в console. Рекомендуется всегда указывать.

Текущая версия — `KEPLER_API_VERSION = "1.0.0"` (см. `shell/electron/kepler-api.ts`).

Bump правила:

- patch (1.0.x): bugfix без изменения contract'а;
- minor (1.x.0): новый method/event, старые работают;
- major (X.0.0): breaking change.

Поддерживаемые формы range: `1.2.3`, `^1.2.3`, `~1.2.3`, `>=1.2.3`, `>1.2.3`, `<=1.2.3`, `<1.2.3`, `*`, `1.2.3 - 2.0.0`, `1.x`, `1.2.x`, OR через `||`. Реализация — `shell/electron/kepler-api.ts → satisfiesSemver()` (минимальная, без `node-semver`).

## Install flow

```text
1. Источник: .kext или директория с manifest.json
       ↓
2. previewSource() — читает manifest.json без extract'а
       ↓
3. UI dialog показывает: icon, name, version, description, author,
   permissions, keplerApiVersion, isUpgrade
       ↓ (пользователь жмёт «Установить» / «Обновить»)
4. installFromPath():
   a. backup current `<dataDir>/extensions/<id>/` → backups/<id>/<ISO>/
      (если уже установлено; prune до MAX_BACKUPS=5)
   b. extract в `<dataDir>/extensions-tmp/<id>-<stamp>/`
   c. validate extracted manifest.id == preview manifest.id
   d. atomic rename target → .old → tmp → target → rm .old
       ↓
5. preview обновляется (isUpgrade flag, currentVersion).
```

### File association (Windows)

`shell/package.json` объявляет:

```json
"fileAssociations": [
  {
    "ext": "kext",
    "name": "Kepler Extension Package",
    "description": "Установочный пакет расширения Kepler",
    "role": "Editor"
  }
]
```

После install Kepler'а NSIS регистрирует `.kext` → `Kepler.exe`. Двойной клик в Проводнике:

1. Запускает `Kepler.exe path\to\extension.kext`.
2. `requestSingleInstanceLock` либо запускает новый instance, либо форвардит argv в running через `second-instance` event.
3. `findKextInArgv(argv)` находит путь, `openInstallExtensionWindow(path)` открывает диалог.
4. Launcher остаётся hidden — user видит только install dialog.

CLI флаг для explicit запуска:

```powershell
Kepler.exe --ext-install path\to\extension.kext
```

## Backup + Revert

При каждом install существующая `extensions/<id>/` копируется в `extensions-backups/<id>/<ISO-timestamp>/`. Лимит MAX_BACKUPS=5 на id; старые удаляются после prune.

```text
<APPDATA>/Kosmos/
├── extensions/<id>/                      ← текущая копия
├── extensions-backups/<id>/
│   ├── 2026-05-15T10-30-00-000Z/         ← перед последним install
│   ├── 2026-05-14T18-00-00-000Z/
│   └── ...
└── extensions-data/<id>/                 ← user data, install не трогает
```

Revert (Settings → Расширения → «Откатить» рядом с строкой extension'а в плоском списке установленных):

1. Сохранить текущую копию как **новый** backup (revert reversible).
2. Скопировать самый свежий backup в `extensions/<id>/` через tmp + atomic rename.
3. Перезапустить extension вручную для пользователя.

API:

- `revertExtension(id, timestamp?)` — без timestamp берётся самый свежий.
- `listBackups(id)` — массив ISO-timestamp'ов, свежие первыми.

## CLI

```powershell
# Install: принимает директорию (dev flow) или .kext / .zip архив.
bun run --cwd shell ext:install <path-to-dir-or-kext>

# Uninstall: удаляет код, user data preserved.
bun run --cwd shell ext:uninstall <id>

# Uninstall + очистка user data.
bun run --cwd shell ext:uninstall <id> --purge-data
```

Скрипты — `shell/scripts/install-extension.mjs` / `uninstall-extension.mjs`. CLI installer выполняет тот же flow (backup + atomic + semver check), что и runtime; код частично дублирован чтобы скрипт работал без dist-electron bundle'а.

## IPC API (Settings UI / install dialog)

Через `window.kepler.extension.*` (см. `shell/shared/ipc-types.ts → KeplerApi.extension`):

| Метод                    | Назначение                                          |
| ------------------------ | --------------------------------------------------- |
| `installPreview(path)`   | manifest preview без extract'а — для install dialog |
| `installDo(path)`        | собственно install с backup'ом                      |
| `installedList()`        | list installed user-extensions для Settings UI      |
| `revert(id, timestamp?)` | restore from backup                                 |
| `backupsList(id)`        | ISO-timestamp'ы доступных backup'ов                 |
| `uninstall(id)`          | rm `extensions/<id>/` (user data preserved)         |

## Persistent user data

::: info Persistent user data — split с 2026-05-14

Код и user data extension'а разделены:

```text
<APPDATA>/Kosmos/
├── extensions/<id>/         ← код — install полностью заменяет
└── extensions-data/<id>/    ← user data — install НЕ трогает
    ├── settings.json
    ├── window-state.json
    └── ...
```

- **Install** трогает только `extensions/<id>/`. `extensions-data/<id>/` сохраняется через все обновления.
- **Revert** возвращает только код, user data не трогает.
- **Preload API**: `window.kepler.userData.{readJson, writeJson, readFile, writeFile, path}` — см. [Extension host → User data](/concepts/extension-host#user-data).
  :::

## Что НЕ входит

- **Auto-update** — checker для новых версий. Юзер сам запускает `ext:install` или открывает свежий `.kext`.
- **Code signing / signature verification** — `.kext` не подписан, install верит источнику.
- **Permissions enforcement** — поле `permissions` сейчас только показывается в install dialog, runtime grant'ит полный API. Capability model в потом.
- **Cross-extension dependencies / store** — extension'ы независимы, marketplace нет.

См. [Kepler Roadmap](/apps/kepler-roadmap) — пункты собраны в section «Extension installer / store».

## Связанные документы

- [Extension host](/concepts/extension-host) — resolution chain и loader.
- [Extension dev mode](/concepts/extension-dev-mode) — Vite HMR (отдельный канал от installer).
- [Kepler Roadmap](/apps/kepler-roadmap) — auto-update, store, capability model.

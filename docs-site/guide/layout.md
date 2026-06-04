# Структура репозитория

## Карта верхнего уровня

```text
kepler/
├─ shell/                  # ⭐ Kepler Electron host (npm: kepler-shell)
├─ extensions/             # Vue-extensions внутри Kepler shell
│  ├─ arrancador/          # игровая библиотека
│  ├─ delphi/              # задачи
│  ├─ eden/                # заметки (TipTap)
│  └─ horologion/          # трекер времени + pomodoro
├─ apps/                   # Зарезервировано (на 2026-05 пусто, только README.md)
├─ crates/                 # Rust crates
│  └─ ark-core/            # ⭐ Rust runtime + ark-core-rpc sidecar
├─ packages/               # TS пакеты (npm scope @kosmos/*)
│  ├─ ark/                 # ⭐ @kosmos/ark — канонический TS SDK
│  └─ visuals/             # @kosmos/visuals — UI-токены, тема, компоненты
├─ services/               # Долгоживущие Rust-сервисы
│  ├─ ark-relay-server/    # WebSocket relay для p2p sync через NAT
│  ├─ kepler-backend/      # ⭐ supervisor для ark-core-rpc + WS gateway + command bus + usage_tracker
│  └─ kepler-watcher/      # watcher-демон над ark-core
├─ mobile/                 # Android модули
│  ├─ delphi/              # Android Delphi (Kotlin + Room)
│  └─ ark-service/         # Android Room ContentProvider
├─ legacy/                 # Заморожено
│  └─ usage-tracker/       # Standalone Rust бинарь (заморожен в Phase E3)
├─ docs/                   # Исходные markdown-доки (ARK-*, EDEN-HEART-*, DELPHI-*, ADRs)
├─ docs-site/              # VitePress сайт документации (вы здесь)
├─ scripts/                # ark:guard:writes, ark:smoke, docs:check, docs:sync
├─ Cargo.toml              # Cargo workspace root (members: crates/*, services/*)
├─ package.json            # Bun workspace root
├─ .agent/                 # Proof-loop артефакты задач
└─ .agents/                # TOML/MD-описания workflow-агентов
```

## Workspaces

### Bun (TS)

`package.json` в корне репозитория объявляет:

```json
"workspaces": [
  "shell",
  "extensions/*",
  "apps/*",
  "mobile/*",
  "services/*",
  "packages/*",
  "docs-site"
]
```

### Cargo (Rust)

`Cargo.toml` в корне объявляет workspace членов: все `crates/*` и `services/*` собираются в общий `target/` (Phase C2). Это ускоряет инкрементальные билды и share'ит зависимости.

## Где что физически лежит

### ARK runtime

```text
crates/ark-core/
├─ rust/
│  ├─ Cargo.toml
│  └─ src/
│     ├─ main.rs              # ark-core-rpc — stdin/stdout JSON-RPC sidecar
│     ├─ lib.rs               # library entry, re-exports
│     ├─ ffi.rs               # UniFFI facade для Android / Swift
│     ├─ schema.rs            # CREATE TABLE / индексы
│     ├─ db.rs                # SQLite CRUD, миграции, sync storage
│     ├─ types.rs             # сущности и sync payloads
│     ├─ protocol.rs          # wire-протокол sync
│     ├─ hlc.rs               # Hybrid Logical Clock
│     ├─ sync_server.rs       # WebSocket sync сервер
│     ├─ sync_client.rs       # WebSocket sync клиент
│     ├─ beacon.rs            # UDP discovery
│     ├─ relay_transport.rs   # outbound клиент к relay
│     ├─ relay_sync.rs        # relay bridge поверх sync
│     ├─ mesh.rs              # координация LAN + relay
│     ├─ host.rs / net.rs     # фильтрация hostname / routable addresses
│     └─ space.rs             # пространство данных
├─ swift/                     # Swift-обёртка над FFI
├─ AGENTS.md
└─ README.md
```

### Kepler shell

```text
shell/
├─ electron/                  # main / preload / extension-host / commands / settings-window
├─ src/                       # Vue renderer (LauncherView, SettingsView)
├─ shared/ipc-types.ts        # KeplerApi (preload contract)
├─ scripts/                   # dev-extensions / install-extension / uninstall-extension
├─ vite.config.mjs            # renderer / main / preload environments
├─ vite.extensions.config.mjs # билд extensions/
├─ playwright.config.ts
└─ package.json               # npm name: "kepler-shell"
```

### Extensions

```text
extensions/<id>/
├─ manifest.json              # id, title, devPort, capabilities
├─ index.html
├─ vite.config.mjs
├─ icon.png                   # отображается в Kepler launcher (cached by mtime)
└─ src/                       # Vue 3 SPA с memory router'ом
```

## Иконки приложений

Стандарт для standalone Electron-приложений Kosmos (Eden и сам Kepler shell):

```text
<app-root>/build/
├─ icon.png              # источник, 512×512–1024×1024 PNG
├─ icon.ico              # кэш (генерируется автоматически из icon.png)
└─ afterPack.cjs         # electron-builder hook: PNG→ICO + rcedit
```

Для Vue-extensions внутри Kepler shell иконка живёт в `extensions/<id>/icon.png` (Kepler shell хост-окно отвечает за `BrowserWindow.icon`).

Иконки в git — canonical source assets для launcher / package metadata, а не
runtime cache. Держи их в диапазоне 512×512–1024×1024; 2375×2375+ PNG без
отдельного дизайн-обоснования считаются лишним весом репозитория. Generated
`*.ico`, extracted app icons и smoke screenshots не коммитятся.

### Конвенция в `package.json` приложения

```jsonc
"build": {
  "afterPack": "./build/afterPack.cjs",
  "directories": { "buildResources": "build" },
  "extraResources": [
    { "from": "build/icon.png", "to": "icon.png" }
  ],
  "win": {
    "icon": "build/icon.png",
    "target": "nsis",
    "signAndEditExecutable": false
  }
}
```

### Зачем `afterPack.cjs`

`win.signAndEditExecutable: false` — workaround под падение `winCodeSign` symlinks на Windows без Developer Mode. Побочка: встроенный rcedit electron-builder отключается, и `.exe` выходит с дефолтной Electron-иконкой. `afterPack.cjs` чинит это руками:

1. `png-to-ico` — `build/icon.png` → `build/icon.ico` (кэшируется по mtime).
2. `rcedit` — встраивает icon + ProductName/FileDescription/version в `<App>.exe`.

Без этого иконки нет ни в taskbar, ни в Start Menu, ни в Explorer.

### Обновление

Замени `build/icon.png` → `bun run build`. `build/icon.ico` перегенерится автоматически.

::: warning Куда класть нельзя
Не в `public/`, не в `electron/`, не в `src/`. `public/` доступен только из renderer по URL; `build/` — единственная конвенция, которую понимают electron-builder + `afterPack.cjs`.
:::

Референс реализации — `shell/build/` и `shell/build/afterPack.cjs`.

## Куда складывать что

| Это                                             | Куда                                                         |
| ----------------------------------------------- | ------------------------------------------------------------ |
| Новый ARK endpoint (Rust)                       | `crates/ark-core/rust/src/*.rs` + регистрация в `main.rs`    |
| Новый метод в TS SDK                            | `packages/ark/src/ark-client.ts`                             |
| UI-компонент, переиспользуемый в 2+ приложениях | `packages/visuals/components/`                               |
| Локальная фича одного extension'а               | внутри `extensions/<id>/src/`                                |
| Локальная фича Eden                             | внутри `extensions/eden/src/`                                |
| Новый extension                                 | новая директория `extensions/<id>/` с `manifest.json`        |
| Концепт / архитектурное решение                 | `docs/` (источник правды) + страница в `docs-site/concepts/` |
| Артефакты proof-loop задачи                     | `.agent/tasks/<DATE>-<slug>/`                                |
| Smoke-БД для тестов                             | `.tmp`, `.e2e`, `.agent/tasks/<TASK>/smoke/`, OS temp        |

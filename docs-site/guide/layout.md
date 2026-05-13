# Структура репозитория

## Карта верхнего уровня

```text
kepler/
├─ apps/                  # Продуктовые приложения (UI + специфичная логика)
│  ├─ ark-service/        # Android APK — Room ContentProvider для Android Delphi
│  ├─ arrancador/         # Electron — игровая библиотека / playtime / бэкапы
│  ├─ dashboard/          # Electron — read-only аналитика ARK
│  ├─ delphi/             # Electron (ts/) + Android (kotlin/) — задачи
│  ├─ eden/               # Vue 3.6 + Electron — заметки и дневник
│  └─ horologion/          # Electron — трекер времени + pomodoro (WIP)
├─ packages/              # Переиспользуемые пакеты
│  ├─ ark-core/           # ⭐ Rust runtime + ark-core-rpc sidecar
│  ├─ kepler-ark/         # ⭐ @kepler/ark — канонический TS SDK
│  └─ kepler-visuals/     # UI-токены, тема, компоненты
├─ services/              # Долгоживущие фоновые сервисы / серверы
│  ├─ usage-tracker/      # Rust — захват usage data, пишет в ARK через RPC
│  └─ ark-relay-server/   # Rust — WebSocket relay для p2p sync через NAT
├─ docs/                  # Исходные markdown-доки (ARK-*, EDEN-HEART-*, DELPHI-*)
├─ docs-site/             # VitePress сайт документации (вы здесь)
├─ scripts/               # ark:guard:writes, ark:smoke
├─ .agent/                # Proof-loop артефакты задач
└─ .agents/               # TOML/MD-описания workflow-агентов
```

## Workspaces

`package.json` в корне репозитория объявляет bun-workspaces:

```json
"workspaces": [
  "apps/*",
  "apps/delphi/ts",
  "apps/eden/ts",
  "services/*",
  "packages/*",
  "docs-site"
]
```

`apps/delphi/ts` и `apps/eden/ts` подняты как отдельные workspace-узлы, потому что TS-часть этих приложений живёт в подпапке рядом с `kotlin/` (Delphi) или `heart/` (Eden) — Rust/Kotlin не должны попадать в bun install.

## Где что физически лежит

### ARK runtime

```text
packages/ark-core/
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

### Eden (заметки)

```text
apps/eden/ts/
├─ src/                       # Vue 3.6 Vapor UI
│  ├─ App.vue
│  ├─ Editor.vue              # TipTap, slash commands, wikilinks
│  ├─ store/                  # Pinia stores (eden.ts, layout.ts)
│  ├─ composables/            # useKeyboard, usePlatform, useSearch
│  ├─ components/             # sidebar/, settings/, typed-notes/, spaces/
│  └─ lib/                    # edenApi.ts (IPC), typedNotes.ts, codeBlocks.ts
├─ main/                      # Electron main
│  ├─ main.ts                 # init, BrowserWindow, IPC handlers
│  ├─ preload.ts              # IPC bridge
│  ├─ store.ts                # SQLite: entries, folders, note types, trash, vault
│  ├─ ark.ts                  # мост на @kepler/ark
│  ├─ heart.ts                # Eden Heart sidecar integration
│  ├─ hevy.ts                 # Hevy fitness API
│  └─ hevySync.ts             # Hevy → Eden entries
├─ heart/                     # Rust + Tantivy search sidecar
│  ├─ Cargo.toml
│  └─ src/main.rs             # stdin/stdout JSON
├─ tests/                     # Playwright E2E (app, typing-stress, hevy, …)
├─ docs/                      # архитектурные решения Eden
└─ electron-builder.json5
```

### Delphi, Arrancador, Dashboard

Аналогичная схема: `apps/<name>/electron/main.ts`, `apps/<name>/electron/preload.ts`, `apps/<name>/electron/main/services/`, `apps/<name>/src/` (Vue).

## Иконки приложений

Стандарт для всех Electron-приложений Kepler (Delphi, Eden, Arrancador, Dashboard, Horologion):

```text
apps/<name>/[ts/]build/
├─ icon.png              # источник, ≥512×512 (рекомендуется ≥1024×1024)
├─ icon.ico              # кэш (генерируется автоматически из icon.png)
└─ afterPack.cjs         # electron-builder hook: PNG→ICO + rcedit
```

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
    "target": "msi",
    "signAndEditExecutable": false
  },
  "mac": { "icon": "build/icon.png" },
  "linux": { "icon": "build/icon.png" }
}
```

### Зачем `afterPack.cjs`

`win.signAndEditExecutable: false` — workaround под падение `winCodeSign` symlinks
на Windows без Developer Mode. Побочка: встроенный rcedit electron-builder отключается,
и `.exe` выходит с дефолтной Electron-иконкой. `afterPack.cjs` чинит это руками:

1. `png-to-ico` — `build/icon.png` → `build/icon.ico` (кэшируется по mtime).
2. `rcedit` — встраивает icon + ProductName/FileDescription/version в `<App>.exe`.

Без этого иконки нет ни в taskbar, ни в Start Menu, ни в Explorer.

### Использование в runtime

`electron/main.ts` читает иконку для `BrowserWindow.icon` (и tray, где есть):

```ts
function resolveIconPath(): string {
  return isDev
    ? path.resolve(__dirname, "../build/icon.png")
    : path.join(process.resourcesPath ?? "", "icon.png");
}

new BrowserWindow({ icon: nativeImage.createFromPath(resolveIconPath()), ... });
```

### Обновление

Замени `build/icon.png` → `bun run build`. `build/icon.ico` перегенерится автоматически.

::: warning Куда класть нельзя
Не в `public/`, не в `electron/`, не в `src/`. `public/` доступен только из renderer
по URL; `build/` — единственная конвенция, которую понимают electron-builder + `afterPack.cjs`.
:::

Референс реализации — `apps/horologion/`, `apps/delphi/ts/`.

## Куда складывать что

| Это | Куда |
|---|---|
| Новый ARK endpoint (Rust) | `packages/ark-core/rust/src/*.rs` + регистрация в `main.rs` |
| Новый метод в TS SDK | `packages/kepler-ark/src/ark-client.ts` |
| UI-компонент, переиспользуемый в 2+ приложениях | `packages/kepler-visuals/components/` |
| Локальная фича одного приложения | внутри `apps/<name>/` |
| Концепт / архитектурное решение | `docs/` (источник правды) + страница в `docs-site/concepts/` |
| Артефакты proof-loop задачи | `.agent/tasks/<DATE>-<slug>/` |
| Smoke-БД для тестов | `.tmp`, `.e2e`, `.agent/tasks/<TASK>/smoke/`, OS temp |

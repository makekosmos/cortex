# Пакеты

`packages/` — это библиотеки и SDK, **которые что-то импортирует**. Долгоживущие daemon'ы и сервера лежат в [`services/`](/services/).

| Пакет | Что это |
|---|---|
| [ark-core](/packages/ark-core) | Rust crate + бинарь `ark-core-rpc`. Сам runtime ARK (SQLite + sync + relay-bridge). Embedded в Electron main как child process. |
| [@kepler/ark](/packages/ark) | TypeScript SDK, говорящий с `ark-core-rpc` по JSON-RPC. Канонический клиент к ARK для Electron main и Node-сервисов. |
| [@kepler/visuals](/packages/visuals) | UI: токены OKLCH, тема, общие Vue-компоненты (Sidebar, Titlebar, DesktopChrome, CommandPalette…). Используется всеми Electron-приложениями и этим сайтом. |

::: tip Соглашение об именах
- **Rust crate** — без scope: `ark-core` (у Cargo нет npm-style scopes).
- **TypeScript-пакет** — `@kosmos/*` (npm scope монорепо).

Папка в `packages/` может отличаться от npm-имени: `packages/ark/` → `@kepler/ark`, `packages/visuals/` → `@kepler/visuals`. В коде импортируется по **npm-имени**, в файловой системе и docs-ссылках — по **папке**.
:::

## Граф зависимостей

```mermaid
flowchart LR
  arkcore["ark-core"]
  kosmosark["@kepler/ark"]
  visuals["@kepler/visuals"]

  eden["Eden"]
  delphi["Delphi"]
  arrancador["Arrancador"]
  dashboard["Dashboard"]

  arkcore -- "JSON-RPC" --> kosmosark

  eden --> kosmosark
  delphi --> kosmosark
  arrancador --> kosmosark
  dashboard --> kosmosark

  eden --> visuals
  delphi --> visuals
  arrancador --> visuals
  dashboard --> visuals

  classDef pkg fill:#2b2b46,stroke:#6b6bcd,color:#fff,stroke-width:1.5px
  classDef app fill:#202020,stroke:#666,color:#fafafa

  class arkcore,kosmosark,visuals pkg
  class eden,delphi,arrancador,dashboard app
```

- `ark-core` — Rust crate + бинарь `ark-core-rpc`. Стрелка к `@kepler/ark` — JSON-RPC поверх stdin/stdout.
- `@kepler/ark` — TS SDK, через который Electron-приложения говорят с runtime'ом.
- `@kepler/visuals` — общие UI-компоненты и токены.

Сервисы (`usage-tracker`, `ark-relay-server`) тоже работают с ARK, но не импортируются из приложений — они **запускаются** независимо. См. [`/services/`](/services/).

## Правило выбора SDK

- Новая интеграция → **`@kepler/ark`**.
- Direct Rust (внутри одного процесса с runtime) → `ark_core::db` хелперы.
- Renderer → preload API (`window.<app>Api`), **не** `@kepler/ark`.

## Workspace-имена

| Папка | Имя в `package.json` / `Cargo.toml` | Язык |
|---|---|---|
| `crates/ark-core` | `ark-core` (Rust crate) | Rust |
| `packages/ark` | `@kepler/ark` | TypeScript |
| `packages/visuals` | `@kepler/visuals` | TypeScript + Vue |

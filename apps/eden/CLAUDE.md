# Eden

Персональное приложение для заметок. Дневник, мысли, знания. Offline-first, local-first.

## Архитектура

Три слоя:

```
┌─────────────────────────────────┐
│  Vue 3.6 Vapor UI (src/)        │  TipTap editor, компоненты, стили
├─────────────────────────────────┤
│  Electron Main Process (main/)  │  IPC, SQLite, интеграции
├─────────────────────────────────┤
│  Heart — Rust sidecar (heart/)  │  Tantivy полнотекстовый поиск
└─────────────────────────────────┘
```

- **src/** — Vue 3.6 Vapor UI: редактор (TipTap), сайдбар, настройки, typed notes
- **main/** — Electron main process: IPC handlers, SQLite storage (store.ts), heart integration, Hevy sync
- **heart/** — Rust binary: полнотекстовый поиск через Tantivy, работает как stdin/stdout sidecar

## Структура папок

```
apps/eden/
├── CLAUDE.md              # Этот файл
└── ts/                    # Electron desktop app
    ├── src/               # Vue 3.6 Vapor UI
    │   ├── App.vue        # Главный layout
    │   ├── Editor.vue     # TipTap editor, slash commands, wikilinks
    │   ├── store/         # Pinia stores (eden.ts, layout.ts)
    │   ├── composables/   # useKeyboard, usePlatform, useSearch, useTitlebarSafeArea
    │   ├── components/    # sidebar/, settings/, dialogs/, typed-notes/, spaces/
    │   └── lib/           # edenApi.ts (IPC), typedNotes.ts, systemTypes.ts, codeBlocks.ts
    ├── main/              # Electron main process
    │   ├── main.ts        # App init, window, IPC handlers
    │   ├── preload.ts     # IPC bridge
    │   ├── store.ts       # SQLite: entries, folders, note types, trash, vault
    │   ├── heart.ts       # Rust sidecar integration
    │   ├── hevy.ts        # Hevy fitness API
    │   └── hevySync.ts    # Hevy → Eden entries
    ├── heart/             # Rust search sidecar (Tantivy + SQLite)
    │   ├── Cargo.toml
    │   └── src/main.rs    # stdin/stdout JSON protocol
    ├── tests/             # Playwright E2E
    ├── docs/              # Архитектурные решения
    ├── public/            # Статика (иконки)
    ├── package.json
    ├── vite.config.ts     # Vite 8 + vite-plugin-electron
    ├── electron-builder.json5
    ├── tsconfig.json
    ├── playwright.config.ts
    └── AGENTS.md          # Инструкции для агентов (чеклист, стиль)
```

## Стек

| Слой | Технология |
|------|-----------|
| UI | Vue 3.6 Vapor + TipTap (rich text) + Pinia |
| Desktop | Electron 38, vite-plugin-electron 1.0.0-beta.2 |
| Storage | SQLite (better-sqlite3) |
| Search | Rust + Tantivy (eden-heart sidecar) |
| Lint | oxlint 1.57, oxfmt 0.36 |
| E2E | Playwright |
| Build | Vite 8 + Rolldown, electron-builder |

## Команды

```bash
cd apps/eden/ts
bun run dev        # Сборка Rust + запуск Electron dev
bun run build      # Production build (Rust release + TS + Vite)
bun run lint       # oxlint
bun run format     # oxfmt --check
bun run test:e2e   # build + Playwright
bun run package    # Создать DMG/installer
```

## Ключевые решения

- **Vapor mode**: leaf-компоненты используют `<script setup vapor lang="ts">`, TipTap-компоненты — обычный VDOM режим; interop включён через `vaporInterop: true` в vite.config.ts
- **Pinia stores**: `useEdenStore` (бизнес-логика, save coordinator) + `useLayoutStore` (UI/сайдбары)
- **Typed notes**: записи имеют `type_id`, `header_layout`, `header_props_json`. Заголовок рендерится через TypedHeader.vue
- **Heart**: Rust sidecar общается через stdin/stdout JSON. НЕ возвращаться к ripgrep
- **Storage hardening**: в `main/store.ts` есть защита для save/move/delete — не упрощать
- **Дизайн**: macOS-native feel, системный шрифт, SwiftUI/Tahoe эстетика
- **Alias**: `@/` → `src/` для импортов
- **preload**: vite-plugin-electron генерирует `preload.mjs` (не `.js`) — в main.ts путь к preload

## Workspace

Подключен к корневому bun workspace как `apps/eden/ts`. Имя пакета: `eden`.

## Будущее

- `apps/eden/kotlin/` — Android версия (планируется)
- Heart не шарится между платформами (в отличие от anytype-heart), живёт внутри ts/

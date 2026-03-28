# Eden

Персональное приложение для заметок. Дневник, мысли, знания. Offline-first, local-first.

## Архитектура

Три слоя:

```
┌─────────────────────────────────┐
│  React UI (src/)                │  TipTap editor, компоненты, стили
├─────────────────────────────────┤
│  Electron Main Process (main/)  │  IPC, SQLite, интеграции
├─────────────────────────────────┤
│  Heart — Rust sidecar (heart/)  │  Tantivy полнотекстовый поиск
└─────────────────────────────────┘
```

- **src/** — React UI: редактор (TipTap), сайдбар, настройки, typed notes
- **main/** — Electron main process: IPC handlers, SQLite storage (store.ts), heart integration, Hevy sync
- **heart/** — Rust binary: полнотекстовый поиск через Tantivy, работает как stdin/stdout sidecar

## Структура папок

```
apps/eden/
├── CLAUDE.md              # Этот файл
└── ts/                    # Electron desktop app
    ├── src/               # React UI
    │   ├── App.tsx        # Главный layout, роутинг
    │   ├── Editor.tsx     # TipTap editor, slash commands, wikilinks
    │   ├── components/    # sidebar/, settings/, dialogs/, typed-notes/, spaces/
    │   └── lib/           # edenApi.ts (IPC), typedNotes.ts, systemTypes.ts
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
    ├── electron.vite.config.ts
    ├── electron-builder.json5
    ├── tsconfig.json
    ├── playwright.config.ts
    └── AGENTS.md          # Инструкции для агентов (чеклист, стиль)
```

## Стек

| Слой | Технология |
|------|-----------|
| UI | React 18 + TipTap (rich text) |
| Desktop | Electron 38, electron-vite 5 |
| Storage | SQLite (better-sqlite3) |
| Search | Rust + Tantivy (eden-heart sidecar) |
| Lint | oxlint, oxfmt |
| E2E | Playwright |
| Build | Vite 7, electron-builder |

## Команды

```bash
cd apps/eden/electron
npm run dev        # Сборка Rust + запуск Electron dev
npm run build      # Production build (Rust release + TS + Vite)
npm run lint       # oxlint
npm run format     # oxfmt --check
npm run test:e2e   # build + Playwright
npm run package    # Создать DMG/installer
```

## Ключевые решения

- **Typed notes**: записи имеют `type_id`, `header_layout`, `header_props_json`. Заголовок рендерится отдельно от тела (TipTap editor)
- **Heart**: Rust sidecar общается через stdin/stdout JSON. НЕ возвращаться к ripgrep
- **Storage hardening**: в `main/store.ts` есть защита для save/move/delete — не упрощать
- **Дизайн**: macOS-native feel, системный шрифт, SwiftUI/Tahoe эстетика
- **Alias**: `@/` → `src/` для импортов

## Workspace

Подключен к корневому bun workspace как `apps/eden/electron`. Имя пакета: `eden`.

## Будущее

- `apps/eden/kotlin/` — Android версия (планируется)
- Heart не шарится между платформами (в отличие от anytype-heart), живёт внутри electron/

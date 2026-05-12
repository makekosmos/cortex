# kepler-visuals

- **Path**: `packages/kepler-visuals`
- **Имя**: `@kepler/visuals`

Общая UI-система для всех Electron-приложений Kepler. Источник дизайна — включая этот сайт документации.

## Структура

```
packages/kepler-visuals/
├─ tokens/                  # colors, typography, radius, spacing, animations
│  ├─ colors.ts             # OKLCH палитры (light + dark + status + smartList)
│  ├─ typography.ts         # SF Pro / Inter, Zed Mono, sizes, weights
│  ├─ radius.ts
│  ├─ spacing.ts
│  ├─ animations.ts
│  └─ index.ts
├─ theme/
│  ├─ css-variables.css     # ⭐ источник правды для CSS-переменных
│  └─ index.ts              # ThemeMode, ColorToken, getColor
├─ components/              # Vue компоненты
├─ patterns/                # композиционные паттерны
├─ react/                   # React биндинги (минимально)
├─ index.ts                 # public API
└─ package.json
```

## Дизайн-токены

### Палитра — OKLCH

Используется OKLCH (а не sRGB) для лучшего восприятия яркости. Light и dark темы определены параллельно.

Ключевые токены (light):

| Token | Значение |
|---|---|
| `background` | `oklch(1 0 0)` |
| `foreground` | `oklch(0.145 0 0)` |
| `muted` | `oklch(0.97 0 0)` |
| `mutedForeground` | `oklch(0.556 0 0)` |
| `border` | `oklch(0.922 0 0)` |
| `ring` | `oklch(0.708 0 0)` |
| `accent` | `oklch(0.546 0.229 264.1)` (Kepler-purple) |

Полный список — `tokens/colors.ts`.

### Типографика

```ts
fontFamily: {
  sans: "-apple-system, BlinkMacSystemFont, SF Pro Display, SF Pro Text, Inter, Avenir, Helvetica, Arial, sans-serif",
  mono: "'Zed Mono', monospace",
}
fontSize: { xs: "0.75rem", sm: "0.875rem", base: "1rem", lg, xl, "2xl" }
fontWeight: { normal: "400", medium: "500", semibold: "600", bold: "700" }
```

Heading-токены в `theme/css-variables.css`:

```css
--kepler-text-heading-size: 28px;
--kepler-text-heading-line-height: 1.02;
--kepler-text-heading-letter-spacing: -0.4px;
--kepler-text-heading-weight: 700;

--kepler-text-page-title-size: 34px;
--kepler-text-page-title-line-height: 1.02;
--kepler-text-page-title-letter-spacing: -0.5px;
--kepler-text-page-title-weight: 700;
```

### Радиус

```css
--radius: 0.85rem;
--corner-shape: squircle;
```

## CSS-переменные

`theme/css-variables.css` — **источник правды** для CSS-переменных. Этот файл подключается всеми Electron-приложениями и зеркалится в `docs-site/.vitepress/theme/custom.css` для этого сайта.

Light и dark темы:

```css
:root {
  --background: oklch(1 0 0);
  --foreground: oklch(0.145 0 0);
  /* ... */
}

.dark {
  --background: oklch(0.17 0 0);
  --foreground: oklch(0.985 0 0);
  /* ... */
}
```

## Компоненты

| Компонент | Назначение |
|---|---|
| `Sidebar.vue` + `SidebarButton.vue` | Навигация по приложению |
| `Titlebar.vue` | Desktop window chrome titlebar |
| `TitlebarHistoryControls.vue` | Кнопки назад/вперёд для router history |
| `DesktopChrome.vue` | Обёртка окна (titlebar + content) |
| `DesktopContentSurface.vue` | Контент-поверхность с правильными safe-area отступами |
| `CommandPalette.vue` | Общий ⌘K |
| `CustomCaret.vue` | Кастомный курсор Eden (overlay над браузерным) |
| `GamePosterCard.vue` | Карточка игры для Arrancador |
| `StatusDot.vue` | Статус-индикатор (success / warning / error / info) |
| `TodoRow.vue` | Строка задачи для Delphi |
| `QuickEntryPanel.vue` | Быстрый ввод |

## Визуальный референс компонентов

ASCII-мокапы — чтобы агенту/новому человеку было понятно, что собой представляет каждый shared-компонент. Точная разметка — в `packages/kepler-visuals/components/*.vue`.

### DesktopChrome + Sidebar + Titlebar

```text
┌──────────────────────────────────────────────────────────────┐
│ ◀ ▶  ⌘                                              ─ □ ✕    │  Titlebar
│  K   │ Eden                                                  │  + TitlebarHistoryControls (◀ ▶)
├──────┴───────────────────────────────────────────────────────┤
│ K    │                                                       │
│ ─    │                                                       │
│ ↳ Inbox            │                                         │
│ ↳ Today            │             DesktopContentSurface       │
│ ─                  │             (правильные safe-area       │
│ ↳ Project A        │              отступы под titlebar)      │
│ ↳ Project B        │                                         │
│                    │                                         │
│ ─                  │                                         │
│ ⚙ Settings         │                                         │
└────────────────────┴─────────────────────────────────────────┘
        Sidebar (260px, resizable)
```

### CommandPalette (⌘K)

```text
                ┌────────────────────────────────────────┐
                │ 🔍  Поиск команды или заметки…         │
                ├────────────────────────────────────────┤
                │ ▸ Новая заметка                  ⌘ N   │
                │   Открыть Today                  ⌘ T   │
                │   Перейти к проекту              ⌘ P   │
                │ ─                                      │
                │   Settings                       ⌘ ,   │
                └────────────────────────────────────────┘
                  squircle radius, blurred backdrop
```

### TodoRow (Delphi)

```text
┌──────────────────────────────────────────────────────────────┐
│ ⊙   Купить хлеб                                  📅 Завтра   │
└──────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────┐
│ ✓   ~~Сделать ревью PR~~                             ✅       │
└──────────────────────────────────────────────────────────────┘
  ⊙ — open    ✓ — completed    drag handle слева невидим до hover
```

### GamePosterCard (Arrancador)

```text
┌────────────────────┐
│   ╔════════════╗   │   16:9 cover image
│   ║            ║   │   scrim-градиент снизу 0% → 82%
│   ║   poster   ║   │
│   ║            ║   │
│   ╚════════════╝   │
│ STEAM              │   ← eyebrow (источник)
│ Hollow Knight      │   ← title (2 строки, line-clamp)
└────────────────────┘
  squircle, hover: overlay opacity 0 → 0.4
```

### StatusDot

```text
●  ●  ●  ●        ← 4 тона: success, warning, error, info
зелёный, оранжевый, красный (oklch), синий
```

### CustomCaret (Eden)

Кастомный текстовый курсор, рендерится **поверх** браузерного. Нужен потому, что Eden использует Vapor mode + TipTap, и нативный курсор иногда уходит не туда.

```text
ABC|DEF      ← реальный курсор браузера
ABC▌DEF      ← CustomCaret overlay (Vapor-friendly, smooth blink)
```

### QuickEntryPanel

```text
┌──────────────────────────────────────────────────┐
│  ⊙  Что нужно сделать?                            │  inline textarea
│ ─                                                 │
│  📁 Project: Inbox  ▾    📅 Today  ▾    🏷 Tag    │  meta-pills
│                            [    Создать    ]      │
└──────────────────────────────────────────────────┘
```

## Полный API

```ts
import {
  // Tokens
  colors, spacing, typography, radius, animations,
  // Theme
  type ThemeMode, type ColorToken, getColor,
  // Components
  CustomCaret, CommandPalette, GamePosterCard,
  SidebarButton, Sidebar, type SidebarNavItem,
  type SidebarProjectItem, type SidebarProjectGroup, type SidebarConfig,
  Titlebar, type TitlebarPlatform,
  TitlebarHistoryControls,
  DesktopChrome, DesktopContentSurface,
  StatusDot, type StatusDotTone,
  TodoRow, type TodoRowItem, type TodoDropPayload,
  QuickEntryPanel, type QuickEntryProject, type QuickEntrySavePayload,
} from "@kepler/visuals";
```

## Правила использования

::: warning Жёстко
- Если компонент есть в `@kepler/visuals` — **импортируй через public API** пакета, не через deep import.
- Не копируй shared sidebar / titlebar / токены внутрь `apps/<name>/`.
- Локальные `src/components/sidebar/*` в приложениях — это **app-specific контейнеры**, не дубли shared UI.
- Desktop chrome — через `DesktopChrome` + `DesktopContentSurface`. Не возвращай ручные `--titlebar-height` хаки.
:::

## Эстетика

- macOS-native ощущение, SwiftUI / Tahoe вайб.
- Системные шрифты (SF Pro), не веб-шрифты без нужды.
- Squircle-радиусы, не идеальные круги.
- Чистый, аккуратный chrome без тяжёлых эффектов «для красоты».

## Связанные документы

- [Архитектура](/concepts/architecture).
- `apps/eden/AGENTS.md` и `apps/dashboard/AGENTS.md` — где именно применяются shared компоненты.

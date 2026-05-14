# kosmos-visuals

- **Path**: `packages/visuals`
- **Имя**: `@kepler/visuals`

Общая UI-система для **всех** Electron-приложений Kosmos (Eden, Delphi, Arrancador, Dashboard, Horologion) и для этого сайта документации. Источник дизайна, токенов, и shared компонентов чрома.

::: danger Обязательно для приложений
Каждое Electron-приложение Kosmos **обязано**:

1. Подключить `@kepler/visuals/theme/css` в renderer entry — это даёт все CSS-переменные (`--background`, `--foreground`, `--border`, `--accent`, `--radius`, `--corner-shape`, шрифты и т.д.).
2. Оборачивать root в `<DesktopChrome>` + `<DesktopContentSurface>` — не делать свой titlebar / safe-area.
3. Использовать только токены (`var(--*)`) для цветов / радиусов / шрифтов в собственных компонентах — никаких hardcoded `#hex`, `rgb()`, `font-family: "Inter"` и тому подобного.
4. Брать готовые компоненты (`Sidebar`, `Titlebar`, `StatusDot`, `CommandPalette`, и т.д.) вместо своих копий.

Свой UI пишется в `apps/<name>/src/` и должен **только** использовать токены и компоненты из `@kepler/visuals`. App-specific компоненты (например, `TimeEntryRow` в Horologion) — это потребители kosmos-visuals токенов, не альтернатива им.
:::

## Структура

```
packages/kosmos-visuals/
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
| `accent` | `oklch(0.546 0.229 264.1)` (Kosmos-purple) |

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
--kosmos-text-heading-size: 28px;
--kosmos-text-heading-line-height: 1.02;
--kosmos-text-heading-letter-spacing: -0.4px;
--kosmos-text-heading-weight: 700;

--kosmos-text-page-title-size: 34px;
--kosmos-text-page-title-line-height: 1.02;
--kosmos-text-page-title-letter-spacing: -0.5px;
--kosmos-text-page-title-weight: 700;
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
| `TodoRow.vue` | Строка задачи для Delphi (click → expand, contextmenu → delete) |
| `QuickEntryPanel.vue` | Быстрый ввод (title / notes / date / project / billable / price) |
| `ContextMenu.vue` + `ContextMenuItem.vue` | Правая-клик меню. Используется в TodoRow и Horologion ListView |
| `Calendar.vue` | Inline-недельный date picker (стрип неделя + навигация) |
| `DateChip.vue` | Chip-кнопка «Дата» + popover с `Calendar`. Замена нативного `<input type="date">` — без чёрной браузерной иконки |
| `DateTimePicker.vue` | Picker даты + времени. Опциональный проп `reference` (`string \| number \| Date`) даёт компактный формат относительно опорной даты: `HH:MM` тот же день, `DD HH:MM` другой день того же месяца, `DD.MM HH:MM` другой месяц, `DD.MM.YY HH:MM` другой год. |
| `Modal.vue` | Базовая модалка |
| `TimeColumn.vue` | Вертикальная шкала времени |
| `Dropdown.vue` | Generic shadcn-стиль `<select>`-замена: trigger + teleport-popover, поддержка клавиатуры (↑/↓/Enter/Escape), click-outside, чекмарк на выбранном. API: `v-model` + `options: { value, label, description?, disabled? }[]`. |

## Визуальный референс компонентов

ASCII-мокапы — чтобы агенту/новому человеку было понятно, что собой представляет каждый shared-компонент. Точная разметка — в `packages/visuals/components/*.vue`.

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

Свёрнутая:

```text
┌──────────────────────────────────────────────────────────────┐
│ ⊙   Купить хлеб                       $ · 📅 14 май          │
└──────────────────────────────────────────────────────────────┘
┌──────────────────────────────────────────────────────────────┐
│ ✓   ~~Сделать ревью PR~~                                     │
└──────────────────────────────────────────────────────────────┘
  ⊙ — open    ✓ — completed
  $ — emerald chip если billable    📅 — chip если scheduledDate
  ПКМ → ContextMenu с пунктом «Удалить» (destructive)
```

Развёрнутая (по клику):

```text
┌──────────────────────────────────────────────────────────────┐
│ ⊙   Купить хлеб                          $ · 📅 14 май       │
│                                                              │
│      [ Купить хлеб                                       ]   │  title input
│      [ Заметки …                                         ]   │  notes textarea
│                                                              │
│      📅 14 май    [ $ Оплачиваемая ]    [ Цена 500 ]         │  date chip + billable + price
└──────────────────────────────────────────────────────────────┘
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
┌──────────────────────────────────────────────────────┐
│  Новая задача                                    ✕   │  title input
│  Заметки …                                           │  notes textarea
│ ──                                                   │
│  📅 14 май   $ Оплачиваемая   [ Цена ]    📁 Inbox ▾ │  meta-row
└──────────────────────────────────────────────────────┘
   ◇ Backdrop ограничен content-областью — titlebar/sidebar остаются интерактивными.
   ◇ `📅` — DateChip (popover Calendar). Native `<input type="date">` не используется.
   ◇ `📁 Inbox` — dropdown реальных проектов из стора. При выборе проекта,
     помеченного `billable`, флаг `$ Оплачиваемая` авто-включается.
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
  TodoRow, type TodoRowItem, type TodoDropPayload, type TodoRowUpdate,
  QuickEntryPanel, type QuickEntryProject, type QuickEntrySavePayload,
  ContextMenu, ContextMenuItem, useContextMenu, type ContextMenuState,
  Modal, Calendar, DateChip, TimeColumn, DateTimePicker,
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

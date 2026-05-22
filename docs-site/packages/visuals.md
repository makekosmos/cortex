# kosmos-visuals

- **Path**: `packages/visuals`
- **Имя**: `@kosmos/visuals`

Общая UI-система для **всех** Electron-приложений Kosmos (Eden, Delphi, Arrancador, Dashboard, Horologion) и для этого сайта документации. Источник дизайна, токенов, и shared компонентов чрома.

::: danger Обязательно для приложений
Каждое Electron-приложение Kosmos **обязано**:

1. Подключить `@kosmos/visuals/theme/css` в renderer entry — это даёт все CSS-переменные (`--background`, `--foreground`, `--border`, `--accent`, `--radius`, `--corner-shape`, шрифты и т.д.).
2. Оборачивать root в `<DesktopChrome>` + `<DesktopContentSurface>` — не делать свой titlebar / safe-area.
3. Использовать только токены (`var(--*)`) для цветов / радиусов / шрифтов в собственных компонентах — никаких hardcoded `#hex`, `rgb()`, `font-family: "Inter"` и тому подобного.
4. Брать готовые компоненты (`Sidebar`, `Titlebar`, `StatusDot`, `CommandPalette`, и т.д.) вместо своих копий.

Свой UI пишется в `apps/<name>/src/` и должен **только** использовать токены и компоненты из `@kosmos/visuals`. App-specific компоненты (например, `TimeEntryRow` в Horologion) — это потребители kosmos-visuals токенов, не альтернатива им.
:::

## Структура

```
packages/visuals/
├─ tokens/                  # colors, typography, radius, spacing, animations
│  ├─ colors.ts             # OKLCH палитры (light + dark + status + smartList)
│  ├─ typography.ts         # SF Pro / Inter, IBM Plex Mono, sizes, weights
│  ├─ radius.ts
│  ├─ spacing.ts
│  ├─ animations.ts
│  └─ index.ts
├─ theme/
│  ├─ css-variables.css     # ⭐ источник правды для CSS-переменных
│  └─ index.ts              # ThemeMode, ColorToken, getColor
├─ components/              # Vue компоненты
├─ patterns/                # композиционные паттерны
├─ composables/             # useContextMenu и т.п.
├─ .storybook/              # Storybook конфиг
├─ index.ts                 # public API
└─ package.json
```

## Story-сервер (Storybook)

Локальный playground всей дизайн-системы — `.stories.ts` файл на каждый
компонент рядом с самим компонентом в `components/`.

```bash
bun install
bun run --cwd packages/visuals storybook            # http://localhost:6006
bun run --cwd packages/visuals build-storybook      # static → storybook-static/
```

Histoire был параллельно поднят раньше (`.story.vue` файлы в `stories/`), удалён
2026-05-19 — единственный source of truth теперь Storybook.

## Дизайн-токены

### Палитра — OKLCH

Используется OKLCH (а не sRGB) для лучшего восприятия яркости. Light и dark темы определены параллельно.

Ключевые токены (light):

| Token             | Значение                                   |
| ----------------- | ------------------------------------------ |
| `background`      | `oklch(1 0 0)`                             |
| `foreground`      | `oklch(0.145 0 0)`                         |
| `muted`           | `oklch(0.97 0 0)`                          |
| `mutedForeground` | `oklch(0.556 0 0)`                         |
| `border`          | `oklch(0.922 0 0)`                         |
| `ring`            | `oklch(0.708 0 0)`                         |
| `accent`          | `oklch(0.546 0.229 264.1)` (Kosmos-purple) |

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

| Компонент                                 | Назначение                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Sidebar.vue` + `SidebarButton.vue`       | Навигация по приложению. `SidebarProjectItem` принимает опциональный `onContextMenu?: (event: MouseEvent) => void` — template передаёт `@contextmenu`. Используется в Eden: ПКМ по entry открывает `<ContextMenu>` с пунктом «Удалить» → `window.api.deleteEntry(id)`.                                                                                                                                                                            |
| `Titlebar.vue`                            | Desktop window chrome titlebar                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TitlebarHistoryControls.vue`             | Кнопки назад/вперёд для router history                                                                                                                                                                                                                                                                                                                                                                                                            |
| `DesktopChrome.vue`                       | Обёртка окна (titlebar + content)                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `DesktopContentSurface.vue`               | Контент-поверхность с правильными safe-area отступами                                                                                                                                                                                                                                                                                                                                                                                             |
| `CommandPalette.vue`                      | Общий ⌘K                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `CustomCaret.vue`                         | Кастомный курсор Eden (overlay над браузерным)                                                                                                                                                                                                                                                                                                                                                                                                    |
| `GamePosterCard.vue`                      | Карточка игры для Arrancador                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `StatusDot.vue`                           | Статус-индикатор (success / warning / error / info)                                                                                                                                                                                                                                                                                                                                                                                               |
| `TodoRow.vue`                             | Строка задачи для Delphi (click → expand, contextmenu → delete)                                                                                                                                                                                                                                                                                                                                                                                   |
| `QuickEntryPanel.vue`                     | Быстрый ввод (title / notes / date / project / billable / price)                                                                                                                                                                                                                                                                                                                                                                                  |
| `ContextMenu.vue` + `ContextMenuItem.vue` | Правая-клик меню. Используется в TodoRow и Horologion ListView                                                                                                                                                                                                                                                                                                                                                                                    |
| `Calendar.vue`                            | Inline-недельный date picker (стрип неделя + навигация)                                                                                                                                                                                                                                                                                                                                                                                           |
| `DateChip.vue`                            | Chip-кнопка «Дата» + popover с `Calendar`. Замена нативного `<input type="date">` — без чёрной браузерной иконки                                                                                                                                                                                                                                                                                                                                  |
| `DateTimePicker.vue`                      | Picker даты + времени. Опциональный проп `reference` (`string \| number \| Date`) даёт компактный формат относительно опорной даты: `HH:MM` тот же день, `DD HH:MM` другой день того же месяца, `DD.MM HH:MM` другой месяц, `DD.MM.YY HH:MM` другой год.                                                                                                                                                                                          |
| `Modal.vue`                               | Базовая модалка                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `TimeColumn.vue`                          | Вертикальная шкала времени                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `Dropdown.vue`                            | Generic shadcn-стиль `<select>`-замена: trigger + teleport-popover, поддержка клавиатуры (↑/↓/Enter/Escape), click-outside, чекмарк на выбранном. API: `v-model` + `options: { value, label, description?, disabled? }[]`.                                                                                                                                                                                                                        |
| `WindowControls.vue`                      | Кастомные min/max/close кнопки для extension windows с Lucide иконками (`Minus`/`Square`/`Copy`/`X`). Реактивно подписан на `window.kepler.window.onMaximizedChange` — иконка maximize переключается на restore (`Copy`, зеркалена по X для Win11-look) когда окно maximized. Props `hideMinimize`/`hideMaximize`/`hideClose` для частичного скрытия (например, в Eden zen mode оставляется только close). См. [WindowControls](#windowcontrols). |
| `IconButton.vue`                          | Ghost-кнопка для иконок (titlebar, mini-player, context-aware controls). Заменяет ad-hoc `.iconbtn` / `.ctl-btn` / `.close-btn` CSS. Props: `size` (default 28), `radius`, `tone: "default" \| "destructive"`, `draggable`, `disabled`. Slot — Lucide icon.                                                                                                                                                                                       |
| `Toggle.vue`                              | Switch (бинарный on/off control) для settings. `v-model: boolean`, props: `label?`, `disabled?`, `ariaLabel?`. Совместим со `SettingsRow` через slot `control`.                                                                                                                                                                                                                                                                                   |
| `Checkbox.vue`                            | Квадратный outline-checkbox (18×18, 6px radius, без ✓ glyph'а — inset filled square при checked). Единый primitive для всех «галочек» в экосистеме (Eden TaskRef, Delphi subitems). `v-model: boolean`, props: `disabled?`, `ariaLabel?`. Accent переопределяется через CSS var `--kosmos-checkbox-accent`.                                                                                                                                       |
| `SettingsRow.vue`                         | Строка Settings UI: title + опциональный description слева, slot `control` (или default) справа. Props: `title`, `description?`, `muted?`. Используется в Eden / Horologion / Delphi / Arrancador settings.                                                                                                                                                                                                                                       |
| `EmptyState.vue`                          | Стандартный empty-state для списков / trash / search results. Props: `title`, `description?`, `compact?`. Slots: `icon`, `action`.                                                                                                                                                                                                                                                                                                                |
| `BlocklistCard.vue`                       | Карточка blocklist'а в Settings → Фокус. Сверху preview доменов с gradient fade, снизу footer (иконка + название + русская плюрализация количества). Props: `name`, `domains: string[]`, `icon?`, `preset?`, `active?`, `count?`. Emits: `click`, `delete`.                                                                                                                                                                                       |
| `Toast.vue` + `ToastHost.vue`             | Toast-уведомления. `ToastHost` — Teleport-renderer в `body`, рисует `<Toast>` через `<TransitionGroup>`. `Toast` props: `message`, `tone?: "info" \| "success" \| "error"`. Использовать через composable `useToast()` + `provideToastHost()` на корне приложения.                                                                                                                                                                                |

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

### WindowControls

Кастомные кнопки «Свернуть / Развернуть / Закрыть» для extension windows. Используется в Eden / Delphi / Horologion / Arrancador AppTitlebar (extension сам рисует titlebar, потому что extension-host передаёт окну `titleBarStyle: "hidden"`).

```text
                                    ┌───┬───┬───┐
                                    │ ─ │ ☐ │ ✕ │
                                    └───┴───┴───┘
                                       │   │
                                       │   └─ Square / Copy (зеркальная) — реактивно
                                       │      переключается при maximize/restore
                                       └─ Minus
```

API:

```ts
interface Props {
  hideMinimize?: boolean;
  hideMaximize?: boolean;
  hideClose?: boolean;
}
```

Поведение:

- При mount читает `window.kepler.window.isMaximized()` (если доступно — non-extension контекст ignore'ит).
- Подписывается на `window.kepler.window.onMaximizedChange(cb)` — иконка middle-кнопки переключается между `Square` (готов развернуть) и `Copy` (готов вернуть в окно) реактивно, без polling'а. Unsubscribe в `onBeforeUnmount`.
- Кнопка close имеет красный hover (`#c42b1c`) под Win11.
- Все три кнопки имеют `-webkit-app-region: no-drag` чтобы клики не попадали в draggable titlebar.

Пример zen-mode (Eden): hide минимизации и развёртывания, оставлен только close + слева placed свой `<LoaderPinwheel>` для exit'а.

См. также [Extension host → Window controls](/concepts/extension-host#window-controls) (preload API).

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
  colors,
  spacing,
  typography,
  radius,
  animations,
  // Theme
  type ThemeMode,
  type ColorToken,
  getColor,
  // Components
  CustomCaret,
  CommandPalette,
  GamePosterCard,
  SidebarButton,
  Sidebar,
  type SidebarNavItem,
  type SidebarProjectItem,
  type SidebarProjectGroup,
  type SidebarConfig,
  Titlebar,
  type TitlebarPlatform,
  TitlebarHistoryControls,
  DesktopChrome,
  DesktopContentSurface,
  StatusDot,
  type StatusDotTone,
  TodoRow,
  type TodoRowItem,
  type TodoDropPayload,
  type TodoRowUpdate,
  QuickEntryPanel,
  type QuickEntryProject,
  type QuickEntrySavePayload,
  ContextMenu,
  ContextMenuItem,
  useContextMenu,
  type ContextMenuState,
  Modal,
  Calendar,
  DateChip,
  TimeColumn,
  DateTimePicker,
  Dropdown,
  WindowControls,
  IconButton,
  Toggle,
  Checkbox,
  SettingsRow,
  EmptyState,
  BlocklistCard,
  Toast,
  ToastHost,
  // Composables
  useToast,
  provideToastHost,
  type ToastOptions,
  type ToastTone,
  type ToastApi,
  // Runtime
  installScrollFadeListener,
  type InstallScrollFadeOptions,
} from "@kosmos/visuals";
```

## Utility-классы

### `.kosmos-scroll` — fade-on-idle скроллбар

Скроллбар, который виден только когда пользователь активно скроллит контейнер, и плавно (450ms) фейдится после остановки. Глобальный паттерн для всех scrollable-листов в Kepler shell и extension'ах.

**Как пользоваться:**

```vue
<div class="rows kosmos-scroll">
  <!-- scrollable content -->
</div>
```

Этого достаточно. Не нужно:

- стилизовать `::-webkit-scrollbar`, `::-webkit-scrollbar-thumb`, `::-webkit-scrollbar-track` руками — класс уже всё закрывает (width/height 5px, border-radius 3px, без фона у track'а);
- задавать свой `transition` на `background-color` thumb'а — переход управляется через CSS-переменную и `@property`, чтобы цвет thumb'а пересчитывался каждый кадр анимации;
- ставить `data-scrolling` руками — это делает глобальный listener из shared helper'а `installScrollFadeListener()` (см. ниже).

**Shared runtime helper:** `@kosmos/visuals/runtime/scroll-fade.ts` экспортирует:

```ts
import { installScrollFadeListener } from "@kosmos/visuals/runtime/scroll-fade";

const off = installScrollFadeListener({ idleMs: 600 }); // оба параметра optional, root = document
// ...
off(); // unsubscribe (idempotent)
```

Вызов идемпотентен — повторный `installScrollFadeListener({ root })` с тем же root возвращает прежнюю отписку, новый listener не регистрируется. Это позволяет звать его и в `shell/src/main.ts`, и в каждом extension `main.ts` без риска накопить duplicate handler'ы при HMR. До 2026-05-19 эта логика была inline в `shell/src/main.ts`; теперь она единый источник правды в `@kosmos/visuals` и используется shell'ом и Eden.

**Как это устроено:**

```css
@property --kosmos-scroll-alpha {
  syntax: "<number>";
  inherits: true;
  initial-value: 0;
}

.kosmos-scroll {
  transition: --kosmos-scroll-alpha 450ms ease;
}

.kosmos-scroll[data-scrolling="1"] {
  --kosmos-scroll-alpha: 0.18;
  transition: --kosmos-scroll-alpha 100ms ease; /* быстрый ramp-up */
}

.kosmos-scroll::-webkit-scrollbar-thumb {
  background-color: rgb(from var(--foreground) r g b / var(--kosmos-scroll-alpha));
}
```

`@property` обязателен — без него `--kosmos-scroll-alpha` нельзя интерполировать как `<number>`, и thumb просто скачком меняет alpha вместо плавного фейда.

**Где сейчас применяется:**

- `shell/src/views/LauncherView.vue` — `.list.kosmos-scroll`;
- `shell/src/views/SettingsView.vue` — `.rows.kosmos-scroll`, `.ext-list.kosmos-scroll`;
- Eden sidebar / entries list (через `installScrollFadeListener` в `extensions/eden/src/main.ts`).

Любой новый scrollable-контейнер в Kepler/extensions должен использовать этот класс, чтобы скроллбар не торчал на фоне Mica/Acrylic.

## Правила использования

::: warning Жёстко

- Если компонент есть в `@kosmos/visuals` — **импортируй через public API** пакета, не через deep import.
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
- `packages/visuals/.storybook/` — конфиг Storybook (`bun run --cwd packages/visuals storybook`).
- [Eden](/apps/eden), [Dashboard](/apps/dashboard), [Horologion](/apps/horologion), [Delphi](/apps/delphi), [Arrancador](/apps/arrancador) — потребители shared компонентов.

# Storybook — `@kosmos/visuals`

Storybook 10 — единственная площадка для review UI компонентов `@kosmos/visuals`:
кнопки, sidebar, primitives, patterns. Histoire был параллельно поднят раньше,
удалён 2026-05-19 как дублирование.

## Запуск

```powershell
# Из корня репо
bun install
bun run --cwd packages/visuals storybook

# Build статической версии (для CI / preview deploy)
bun run --cwd packages/visuals build-storybook
```

Storybook поднимется на `http://localhost:6006`. Сторонний agent отвечает
за `.storybook/main.ts` + deps — не трогай их без необходимости.

## Конвенция для stories

### Формат — CSF 3

Только Component Story Format 3 — `meta` + named exports как stories.
Никаких CSF 2 (`storiesOf(...)`) и MDX без необходимости.

```ts
// components/MyButton.stories.ts
import type { Meta, StoryObj } from "@storybook/vue3";
import MyButton from "./MyButton.vue";

const meta = {
  title: "Components/MyButton",
  component: MyButton,
  tags: ["autodocs"],
  argTypes: {
    size: { control: "select", options: ["sm", "md", "lg"] },
  },
} satisfies Meta<typeof MyButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: { size: "md" },
};

export const Large: Story = {
  args: { size: "lg" },
};
```

### Naming

- `meta.title` — `Категория/Имя` (например, `Components/Button`,
  `Patterns/EmptyState`, `Primitives/Surface`).
- Story exports — `PascalCase` (`Default`, `Large`, `WithIcon`, `Loading`).
- Файл — `<ComponentName>.stories.ts` рядом с `.vue` компонентом.

### Категории (порядок в sidebar)

`Intro → Tokens → Primitives → Components → Patterns → *`.
Задано через `options.storySort` в `.storybook/preview.ts`.

## Theme switcher

Тулбар вверху Storybook содержит селектор **Тема**. Сейчас доступна только
**Тёмная** — это default в Kepler. Light theme в roadmap (`TODO(light-theme)`
в `.storybook/preview.ts`). Когда токены `--background` / `--foreground` /
прочие появятся в `theme/css-variables.css` под `.light` селектором —
расскомментируй light item в `globalTypes.theme.toolbar.items`.

## Viewports

Доступны Kepler-специфичные размеры:

- **Kepler Launcher** — 720×460 (фикс размер launcher окна).
- **Kepler Settings** — 880×560 (settings window).
- **Extension default** — 1200×800 (стандартное extension окно, default
  viewport в Storybook).

Полезно проверить, что компонент не ломается, например, в narrow launcher.

## Как добавить новый story file

1. Создай `<Component>.stories.ts` рядом с `<Component>.vue`.
2. Скопируй boilerplate из примера выше.
3. Минимум — `Default` story с разумными `args`.
4. Если компонент имеет варианты (size, intent, state) — отдельная story
   на каждый значимый.
5. Поставь `tags: ["autodocs"]` — Storybook автоматом сгенерирует Docs page
   с props table и source code (source — closed by default, открывается
   через "Show code").

## Дополнительно

- Background — подвязан к `--background` из dark theme. Переключение через
  тулбар **Backgrounds** даёт kepler-dark / kepler-light values (light —
  placeholder до появления реальной светлой темы).
- Controls — `expanded: true`, props table раскрыта по умолчанию.
- Docs — TOC включён (правая колонка docs page), source — closed by default.

Полный набор Kepler-правил для UI компонентов — в `docs-site/agents/checklists.md`
и `docs-site/agents/forbidden.md` (раздел **UI**).

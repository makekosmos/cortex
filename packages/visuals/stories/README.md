# `@kepler/visuals` — stories

Полный набор Histoire-историй для дизайн-системы Kosmos. Запускается локально, не
требует backend / extensions / Electron.

## Запуск

```bash
# из корня монорепо
bun install

# dev — HMR на http://localhost:6006
bun run --cwd packages/visuals story:dev

# static build → .histoire/dist/
bun run --cwd packages/visuals story:build

# preview уже собранной сборки
bun run --cwd packages/visuals story:preview
```

## Структура

```
stories/
├── _preview.css          — стили под preview-канвас (использует CSS-vars темы)
├── tokens/               — токены design-system (colors, spacing, typography, radius, animations)
└── components/           — все компоненты из @kepler/visuals/components
```

Histoire конфиг — `packages/visuals/histoire.config.ts`. Setup-файл с подключением
`vue-router` (memory history) и темо-биндингом — `packages/visuals/histoire.setup.ts`.

## Темизация (light / dark)

Toggle в правом верхнем углу UI Histoire переключает атрибут `data-color-mode` на `<html>`.
В `histoire.setup.ts` MutationObserver зеркалит этот атрибут в `class="dark"` на `<body>`,
после чего все CSS-variables из `packages/visuals/theme/css-variables.css` подхватываются
автоматически. По умолчанию открывается dark-тема (см. `defaultColorScheme` в конфиге).

## Как писать новую story

1. Создай файл `stories/components/<Name>.story.vue` (или в `tokens/`).
2. Используй формат Histoire:

   ```vue
   <script setup lang="ts">
   import MyComponent from "../../components/MyComponent.vue";
   </script>

   <template>
     <Story title="MyComponent" group="primitives">
       <Variant title="default">
         <div class="story-canvas">
           <MyComponent />
         </div>
       </Variant>
     </Story>
   </template>
   ```

3. `group` — один из `tokens / primitives / popovers / datetime / patterns / complex`
   (см. `tree.groups` в `histoire.config.ts`). Если нужна новая категория — добавь её туда.
4. Для тяжёлых композитов (модалки, popover'ы, full-shell layouts) используй
   `:layout="{ type: 'single', iframe: true }"` — preview рендерится в iframe,
   глобальные слушатели не «текут» между вариантами.
5. Готовые утилитарные классы из `_preview.css`: `.story-canvas`, `.story-row`,
   `.story-col`, `.story-frame`, `.story-label`.

## Покрытие

23 story-файла, 62 варианта. Каждый экспортируемый компонент `@kepler/visuals/components`
(19 шт, включая `ContextMenuItem` и `DesktopContentSurface` в составе родительских историй)
и каждый набор токенов имеют как минимум одну story.

## Известные шероховатости

- Histoire 0.17.x использует Vite 5 под капотом, поэтому visuals-workspace ставит
  локальную `vite ^5.4.0` рядом с глобальной `vite ^8.0.3` в `shell/`. Разные версии
  не пересекаются — story-build живёт в своём bun-cache slot’е.
- При первом старте dev-сервера Histoire выводит warnings вида
  `Failed to resolve dependency: flexsearch / shiki-es` — это optional-зависимости
  для markdown-доков, которые мы не используем (документация живёт в `docs-site/`).
  Игнорируй.
- `CustomCaret` рендерится Teleport’ом в body и слушает `document` глобально:
  переключай вариант — каретка переинициализируется через cleanup `onUnmounted`.

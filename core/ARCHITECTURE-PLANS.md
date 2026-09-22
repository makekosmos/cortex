# Architecture Plans

Рабочий документ для будущей декомпозиции Kosmos. Это не ADR и не финальное
решение; цель файла - собрать направление, вопросы по неймингу и критерии, по
которым позже можно будет принять более жесткие архитектурные решения.

## Problem

Сейчас Kosmos удобен для одного разработчика: все лежит рядом, shared-код
можно менять напрямую, приложения и runtime быстро проверяются вместе.

Но по мере подключения других людей монорепо начинает выглядеть как одна
большая территория без явных входных дверей. Новый человек видит сразу shell,
extensions, ARK, Rust services, UI-библиотеку, docs и build tooling. Даже если
технически это один workspace, когнитивно это должно ощущаться как несколько
понятных продуктов.

Главная задача сейчас - не обязательно физически дробить git-репозиторий, а
сделать так, чтобы внутри него были настоящие границы ответственности.

## Current Direction

Не дробить репозиторий первым шагом.

Вместо этого оформить Kosmos как modular monorepo:

- один git repo для разработки и atomic changes;
- несколько package/product areas внутри workspace;
- явные public API между areas;
- onboarding по area, а не по всему проекту сразу;
- affected checks для измененных областей.

Идея: не "один большой проект", а "несколько продуктов в одном workspace".

## Target Areas

### Product Apps

Пользовательские приложения и extensions:

- `extensions/delphi` - задачи;
- `extensions/eden` - заметки;
- `extensions/arrancador` - игровая библиотека;
- будущие `extensions/*` или `apps/*`.

Приложение должно знать свой domain и public API платформы. Оно не должно
знать внутренности ARK runtime, shell windowing или других приложений.

### Shell Platform

Desktop host и platform layer:

- `shell`;
- extension host;
- command bus;
- windows / titlebar / launcher / settings / dashboard;
- lifecycle приложения.

Shell отвечает за desktop-окружение, загрузку extensions и integration points.
User-facing product name - Kosmos. Legacy/internal namespace `kepler:*` пока
остается compat-layer, а не brand.

### Design System

Shared UI:

- `packages/visuals`;
- CSS variables как source of truth для темы;
- Tailwind utilities для component implementation;
- primitives: button, input, modal, dropdown, sidebar;
- reusable patterns: poster card, todo row, settings rows.

`@kosmos/visuals` должен быть настоящим package boundary. Приложения импортят
только public exports и theme CSS.

### Data Platform

Общий data/runtime слой:

- `crates/ark-core`;
- `packages/ark`;
- sync model;
- object model;
- write boundary.

Apps говорят с данными через `@kosmos/ark` или утвержденные shell/backend APIs.
Прямые writes в sync tables запрещены.

### Native / System Services

Системные и privileged компоненты:

- `services/kepler-backend`;
- `services/kepler-focus-helper`;
- `services/kepler-focus-svc`;
- usage tracker;
- focus mode integration.

Эти компоненты не должны расползаться в app-layer. Их API должен быть узким и
документированным.

### Tooling And Docs

Инфраструктура проекта:

- `docs-site`;
- root scripts;
- smoke matrix;
- release/bump flow;
- AGENTS/CLAUDE generation;
- e2e and visual verification.

Docs должны помогать войти в конкретную area, а не заставлять читать весь
проект целиком.

## Boundary Rules

Для каждой area нужно явно ответить на четыре вопроса:

1. Что это?
2. Кто этим пользуется?
3. Через какой public API сюда можно входить?
4. Что нельзя трогать напрямую?

Минимальный шаблон area-документа:

```md
# Area Name

Purpose:
...

Public API:
...

Consumers:
...

Forbidden:
...

How to verify:
...
```

## Proposed Folder Semantics

Текущая структура в целом жизнеспособна:

```txt
extensions/       user-facing extension apps
apps/             larger standalone apps, if they appear
packages/         shared TS packages and libraries
crates/           Rust core crates
services/         desktop/backend/system services
shell/            Kosmos desktop host
docs-site/        source of truth for project docs
```

Вопрос на будущее: стоит ли переименовать `extensions/*` в `apps/*` или
оставить различие?

Предварительное правило:

- `extensions/*` - приложения, которые живут внутри Kosmos shell;
- `apps/*` - standalone apps вне shell lifecycle;
- `packages/*` - libraries без собственного user-facing lifecycle;
- `services/*` - runtime/system processes.

## Naming Questions

### Kosmos vs Kepler

Сейчас:

- Kosmos - внешний product name;
- Kepler - legacy/internal namespace (`kepler:*`, `window.kepler`,
  `services/kepler-backend`).

Нельзя делать массовый rename сразу. Но нужно зафиксировать naming policy:

- new user-facing text uses Kosmos;
- new public package names use `@kosmos/*`;
- internal compat APIs may stay `kepler` until deliberate migration;
- new docs should explain Kepler as legacy/internal namespace.

Открытый вопрос: нужен ли долгосрочный rename `services/kepler-backend` в
`services/kosmos-runtime`, или лучше оставить как internal codename.

### Visuals Naming

`@kosmos/visuals` сейчас означает UI tokens/components/patterns.

Варианты:

- оставить `visuals`;
- переименовать в `@kosmos/ui`;
- разделить позже на `@kosmos/theme`, `@kosmos/ui`, `@kosmos/patterns`.

Предварительная позиция: не переименовывать сейчас. Сначала довести package
boundary и usage contract. Rename без ясной пользы создаст churn.

### Visuals Sizing

`@kosmos/visuals` должен следовать строгой, но практичной spacing scale:
2/4/8/16/32/64px.

Правило:

- крупные component dimensions, spacing, offsets, widths and radii are multiples of 8px;
- маленькие внутренние расстояния могут использовать 2px/4px;
- borders, rings and dividers may use 1-2px;
- typography follows a type scale and does not have to be 8px-based;
- transform percentages, aspect ratios and animation timings are outside the
  spacing grid;
- any off-scale layout exception in `packages/visuals` needs an explicit comment
  or a token-level reason.

Цель - Linear-style consistency: лучше единая система, чем локально идеальный
one-off пиксель.

### ARK Naming

ARK выглядит как отдельный data platform brand/codename.

Нужно решить:

- ARK - это public developer-facing concept или internal runtime codename?
- `@kosmos/ark` - стабильное имя SDK или временный слой?
- Нужно ли user-facing docs скрывать ARK за "Kosmos Data Engine"?

Предварительная позиция: оставить ARK как developer-facing platform concept,
но не делать его user-facing product name.

### Extensions Naming

Сейчас "extension" означает Vue-приложение, загружаемое в Kosmos shell.

Риски:

- external contributors могут ожидать browser-extension semantics;
- "app" понятнее для product thinking;
- "extension" точнее для shell lifecycle.

Возможное правило: в коде и папках оставить `extensions`, в продуктовой речи
называть их приложениями Kosmos.

## Onboarding Model

Новый человек не должен начинать с понимания всего Kosmos.

Примеры входов:

- работает над Delphi: читает Delphi area doc, `@kosmos/visuals` contract,
  write-boundary;
- работает над UI: читает visuals area doc, Tailwind/theme contract,
  visual verification rules;
- работает над sync/data: читает ARK/write-boundary/sync docs;
- работает над shell: читает extension host, command bus, instance isolation.

Нужны короткие "start here" страницы по area.

## Development Workflow Goal

Shared package development должен оставаться быстрым:

1. Разработчик меняет `packages/visuals`.
2. Apps в workspace сразу видят изменения через local workspace import.
3. Проверки запускаются по affected consumers.
4. После стабилизации package можно версионировать/публиковать.

Это дает плюсы отдельной библиотеки без боли polyrepo на ранней стадии.

## Why Not Polyrepo Yet

Физический split сейчас добавит:

- linked package setup;
- canary releases;
- version drift;
- multi-repo PR coordination;
- сложные cross-package refactors;
- больше CI/release ceremony.

Для Kosmos сейчас важнее скорость изменения границ, чем независимое release
управление каждым пакетом.

Polyrepo можно рассмотреть позже, когда:

- public APIs устоялись;
- пакет имеет независимых consumers;
- release cadence реально разная;
- external contributors работают только в одном package и не нуждаются в
  full workspace;
- CI/release tooling готов.

## Near-Term Plan

1. Оформить `@kosmos/visuals` как полноценную UI library boundary.
2. Зафиксировать Tailwind/theme contract.
3. Почистить imports между packages/apps.
4. Добавить area docs в `docs-site`.
5. Сформировать ownership map.
6. Ввести affected verification matrix.
7. Позже решить, нужны ли independent package versions через changesets или
   собственный release flow.

## Open Questions

- Нужно ли `packages/visuals` переименовывать в `packages/ui`?
- Должен ли ARK оставаться отдельным developer-facing брендом?
- Нужно ли разделять `shell` на `shell/host`, `shell/views`, `shell/electron`,
  или текущего разделения достаточно?
- Должны ли product apps жить в `extensions/*` или в `apps/*`?
- Какие areas должны иметь CODEOWNERS, когда появятся другие разработчики?
- Нужно ли вводить changesets для package versioning?
- Где проходит граница между reusable UI pattern и app-specific component?
- Какие checks должны быть обязательными для каждой area?

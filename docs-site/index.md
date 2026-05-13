---
layout: home

hero:
  name: Kepler
  text: Local-first монорепо для личного софта
  tagline: Общий ARK-рантайм поверх Rust + SQLite и набор сфокусированных Electron-приложений. Данные живут на устройстве, синхронизация — поверх.
  actions:
    - theme: brand
      text: С чего начать
      link: /guide/getting-started
    - theme: alt
      text: Архитектура
      link: /concepts/architecture
    - theme: alt
      text: Для AI-агента
      link: /agents/

features:
  - title: ARK runtime
    details: Rust crate + sidecar ark-core-rpc поверх SQLite. Универсальная модель объектов, usage-данные, LAN-синхронизация, relay-bridge с HMAC-аутентификацией пиров, HLC.
    link: /packages/ark-core
    linkText: ark-core
  - title: Один SDK на всех
    details: "@kepler/ark — канонический TypeScript-клиент к sidecar. Apps говорят только через него. Прямые SQL writes в ARK-таблицы запрещены."
    link: /concepts/write-boundary
    linkText: Граница записи
  - title: Electron-приложения
    details: Eden (заметки), Delphi (задачи), Arrancador (игры), Dashboard (аналитика). Каждый — тонкая продуктовая оболочка вокруг общего ARK.
    link: /apps/
    linkText: Все приложения
  - title: Proof loop
    details: Substantial-правки идут через .agent/tasks/&lt;DATE&gt;-&lt;slug&gt;/. spec → evidence → verify → problems. Каждый AC должен быть PASS.
    link: /concepts/proof-loop
    linkText: Proof loop
  - title: Изоляция тестовых БД
    details: Никогда не указывай тестам user ARK DB. Только .tmp / .e2e / .agent/tasks/ / OS temp. Это политика репо.
    link: /concepts/test-isolation
    linkText: Test isolation
  - title: Дизайн-система
    details: kepler-visuals — общие токены OKLCH, тема, компоненты Sidebar / Titlebar / DesktopChrome / CommandPalette. Источник дизайна (включая этот сайт).
    link: /packages/kepler-visuals
    linkText: kepler-visuals
  - title: Документация-как-код
    details: Все страницы — markdown в docs-site/. Источник правды для разработчиков и AI-агентов одновременно. Заменяет AGENTS.md / CLAUDE.md.
    link: /agents/
    linkText: Для агента
---

<div style="max-width: 960px; margin: 0 auto; padding: 48px 24px 80px; color: var(--vp-c-text-2);">

## Что это вообще

Kepler — это **монорепо для всех личных приложений автора**. Внутри лежит общий рантайм данных (`ARK`) и продуктовые оболочки вокруг него: дневник, задачи, игры, аналитика.

Цель монорепо — **единый контракт хранения и синхронизации**. Любая «штука пользователя» (заметка, задача, игра, сессия использования) — это объект в ARK. Приложения — просто разные UI-проекции и интеграции поверх одного общего данных.

## С чего читать

<span class="kbadge canon">для новичка</span> Иди по гиду: [«С чего начать»](/guide/getting-started) → [«Архитектура»](/concepts/architecture) → [«Все приложения»](/apps/).

<span class="kbadge info">для бывалого</span> Открой [«Правила репозитория»](/reference/rules) и [«Smoke-матрицу»](/reference/smoke-matrix).

<span class="kbadge accent">для AI-агента</span> Сразу в [/agents/](/agents/) — там запреты, чек-листы и шаблоны спецификаций.

</div>

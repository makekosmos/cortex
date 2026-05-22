---
title: Eden — заметки
description: Rich-text заметки с wikilink, задачами и Anytype-style block selection.
---

# Eden — заметки

Редактор заметок на TipTap. Каждая заметка — объект в ARK, доступный
другим расширениям.

<div class="manual-figure">
  <img src="/manual/eden.png" alt="Eden — редактор заметок" />
</div>

<style scoped>
.manual-figure {
  margin: 28px 0;
}
.manual-figure img {
  display: block;
  width: 100%;
  height: auto;
  border-radius: 14px;
}
</style>

## Что умеет

- **Rich text** — заголовки, списки, чек-боксы, цитаты, code blocks
- **Wikilink'и** `[[имя]]` — ссылки между заметками
- **Задачи внутри заметки** — `/задача` или `- [ ]` создают ссылку на
  `task_obj` в ARK (видна в Delphi сразу же)
- **Zen mode** — фокус на тексте, всё лишнее скрыто
- **Block selection** — Anytype-стиль rubber-band выделения блоков
- **Markdown copy** — `Ctrl+A` → копирование в markdown

## Открыть

- Поисковик → `eden`
- Команда `note: создать` — сразу новая заметка

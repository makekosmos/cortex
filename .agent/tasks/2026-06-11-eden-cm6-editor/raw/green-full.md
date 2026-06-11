# Финальный зелёный прогон — 2026-06-11

## bun run test (unit + browser)

```
$ bun run test:unit && bun run test:vue
$ bun test tests/*.test.ts
bun test v1.3.14 (0d9b296a)

 32 pass
 0 fail
 37 expect() calls
Ran 32 tests across 3 files. [87.00ms]

$ vitest run --browser

 RUN  v4.1.6 D:/Personal/Hobby/Coding/kosmos/products/eden

 DEPRECATED  tests/components/CmEditor.spec.ts tries to load deprecated "@vitest/browser/context"
 (предупреждение — не ошибка; в следующем major потребует замены на "vitest/browser")

 Test Files  8 passed (8)
      Tests  31 passed (31)
   Start at  01:40:24
   Duration  3.90s (transform 0ms, setup 0ms, import 4.12s, tests 1.27s)
```

## Итог

| Набор          | Файлов | Тестов | Провалено |
| -------------- | ------ | ------ | --------- |
| bun:test unit  | 3      | 32     | 0         |
| vitest browser | 8      | 31     | 0         |
| **Итого**      | **11** | **63** | **0**     |

## Что было исправлено в ходе финального прогона

### 1. CmEditor.vue — включён GFM диалект (TaskMarker)

`markdown()` из `@codemirror/lang-markdown` по умолчанию использует `commonmarkLanguage` (без GFM).
Добавлен `base: markdownLanguage` — предустановленный GFM+расширения диалект из того же пакета.
Без этого `@lezer/markdown` не генерировал ноды `TaskMarker`, live-preview не рендерил чекбоксы.

### 2. CmEditor.spec.ts — экранирование `[` в userEvent.keyboard

`@testing-library/user-event` интерпретирует `[...]` как дескриптор физической клавиши.
`"[ ] задача"` → ошибка: пробел не валидное имя клавиши.
Исправление: `[[` вводит символ `[`, `]` вводится как есть → `"- [[ ] задача"`.

### 3. mdConvert.ts — восстановление ASCII-скобок в markdownToJson

Воркэраунд подменял `[` на fullwidth `［` перед парсом TipTap, но не восстанавливал в JSON.
`content_json` хранил `"［x] задача"` вместо `"[x] задача"`.
Добавлен рекурсивный постпроцессинг `restoreCheckboxBracketsInJson`:
обходит text-ноды PM JSON и заменяет `［` → `[`.
CmConvert.spec.ts (5/5) по-прежнему зелёный — round-trip `jsonToMarkdown` не затронут.

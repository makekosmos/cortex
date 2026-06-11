# RED-прогон для Eden CM6 Editor TDD

## Стадия: RED

Дата: 2026-06-11
Задача: Доделать RED-стадию TDD для Eden CM6 Editor (стабы + тесты)

---

## Что было сделано

### 1. Почищен стаб src/editor-cm/CmEditor.vue

- Убран импорт `defineProps`/`defineEmits` из "vue" (это compiler-макросы)
- Добавлен импорт `type { Entry }` из vite-env
- Добавлена обработка Entry типа правильно
- Шаблон упрощен: только `<div class="cm-editor-host" data-testid="cm-editor-host">`
- `<div class="cm-content" data-testid="cm-content" contenteditable="true">`
- Error выбрасывается в `onMounted` hook (не на верхнем уровне скрипта)

### 2. Создан тест tests/components/CmEditor.spec.ts

- Стиль: vitest-browser-vue с render()
- 4 теста для RED:
  1. "монтируется и показывает контент в .cm-editor-host"
  2. "ввод текста эмитит liveCharCount"
  3. "чекбокс live preview"
  4. "автосейв вызывает onSave с валидным PM JSON"
- Хелпер makeEntry() создает Entry со всеми полями (schema_version: 1)

---

## RED-прогоны

### Прогон 1: bun test tests/cmGate.test.ts

```
bun test v1.3.14
tests\cmGate.test.ts - 12 тестов

РЕЗУЛЬТАТ: 0 pass, 12 fail

Ошибка: не реализованы функции isCmSafeDoc и shouldUseCmEditor
Все тесты падают с "Error: not implemented" как ожидается
Время: 27ms
```

**Падающие тесты:**

- isCmSafeDoc: 7 тестов (safe doc, taskRef, wikilink, unknownNode, unknownMark, null, string, число)
- shouldUseCmEditor: 5 тестов (prefEnabled false/true, safe JSON, taskRef, invalid JSON)

---

### Прогон 2: bun run test:vue (браузерные тесты)

```
vitest run --browser

РЕЗУЛЬТАТ СУММАРНО:
Test Files: 2 failed | 6 passed (8)
Tests: 9 failed | 22 passed (31)

Время: 2.66s
```

#### Детально:

**CmConvert.spec.ts: 5 FAILED**

- jsonToMarkdown: PM JSON с heading level 1 → markdown содержит # Заголовок
- jsonToMarkdown: paragraph с bold-текстом → markdown содержит \*\*
- markdownToJson: «# Привет\n\nтекст» → объект с heading level 1
- round-trip стабильность: markdown → JSON → markdown → JSON дают deep-equal JSON
- round-trip чекбокс-строк: markdown с [ ] и [x] сохраняют подстроки

Ошибка: createMdConverter() выбрасывает "Error: not implemented"

**CmEditor.spec.ts: 4 FAILED** ✓ НОВОЕ

- монтируется и показывает контент в .cm-editor-host
- ввод текста эмитит liveCharCount
- чекбокс live preview
- автосейв вызывает onSave с валидным PM JSON

Ошибка: onMounted выбрасывает "Error: CmEditor not implemented"

**Прочие тесты: 22 PASSED** ✓ Не сломаны

- BlockSelectionAutoScroll.spec.ts (3)
- BlockSelectionClasses.spec.ts (2)
- BlockSelectionPointer.spec.ts (4)
- CharCounter.spec.ts (5)
- EntryTitle.spec.ts (4)
- JournalTitleReadonly.spec.ts (3)

---

## Статус

### Готово для GREEN-стадии:

✓ Стабы с правильным импортом Entry
✓ CmEditor.vue монтируется (выбрасывает ошибку в onMounted)
✓ Тесты падают с осмысленными ошибками ("not implemented")
✓ Чужие тесты не сломаны
✓ RED-стадия завершена

### Следующий шаг:

Реализация функций + GREEN-стадия:

- `isCmSafeDoc()` — walk JSON, проверить что все ноды и маркс в CM_SAFE_NODES/MARKS
- `shouldUseCmEditor()` — prefEnabled && isCmSafeDoc(parseJSON(contentJson))
- `createMdConverter()` — создать экземпляр конвертера MD ↔ PM JSON
- CmEditor.vue реализация — CM6 editor, liveCharCount эмит, autosave

---

## Файлы

- `src/editor-cm/CmEditor.vue` — очищенный стаб с Error в onMounted
- `src/editor-cm/cmGate.ts` — стаб isCmSafeDoc + shouldUseCmEditor
- `src/editor-cm/mdConvert.ts` — стаб createMdConverter
- `tests/cmGate.test.ts` — 12 юнит-тестов (bun:test)
- `tests/components/CmConvert.spec.ts` — 5 компонент-тестов (уже создан)
- `tests/components/CmEditor.spec.ts` — 4 компонент-теста (новый)

---

## Заметки

1. CmEditor выбрасывает ошибку в onMounted (не на верхнем уровне скрипта), потому что верхнеуровневый throw блокировал бы всю компиляцию.

2. Тесты CmEditor используют data-testid="cm-editor-host" и data-testid="cm-content" чтобы можно было монтировать и инспектировать элементы.

3. Чужие компонент-тесты (26 passed) остаются нетронутыми и проходят. Это подтверждает что изменения не сломали существующую архитектуру.

4. RED-стадия завершена успешно - все новые тесты падают с правильными сообщениями об ошибках.

## Дополнение (оркестратор)

tests/components/CmEditor.spec.ts переписан на строгие GREEN-ожидания (ввод текста через userEvent, проверка liveCharCount, чекбокс-тоггл до onSave, автосейв). Повторный RED:

```
bunx vitest run --browser=chromium tests/components/CmEditor.spec.ts
Test Files  1 failed (1)
Tests  4 failed (4)  — все с "CmEditor not implemented" (throw из стаба onMounted)
```

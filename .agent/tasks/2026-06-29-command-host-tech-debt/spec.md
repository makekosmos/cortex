# Tech debt: command-host известные баги

Статус: **ОТЛОЖЕНО** — command-host / Raycast-совместимость не приоритет на текущем этапе.
Ветка: `claude/vue-perf-optimization-hp1jhk` (смёрджена или будет мёрджена в main).

---

## Баг 1: Именованные спецклавиши в шорткатах никогда не срабатывают

**Файл:** `platform/desktop/src/command-host/model.ts`, функция `matchesActionShortcut` (~line 283)

**Суть:**
`actionShortcut` вычисляет `shortcut.code` только для одиночных латинских букв (`/^[a-z]$/`).
Для именованных клавиш (`"return"`, `"space"`, `"escape"` и т.д.) `code = null`,
и matching падает на `event.key.toLowerCase() === shortcut.key`.

Проблема: DOM `KeyboardEvent.key` не совпадает с Raycast API именами:

| Raycast API key | DOM event.key | Совпадение                                               |
| --------------- | ------------- | -------------------------------------------------------- |
| `"return"`      | `"Enter"`     | ❌ `"enter" !== "return"`                                |
| `"space"`       | `" "`         | ❌ `" " !== "space"`                                     |
| `"escape"`      | `"Escape"`    | ❌ `"escape" !== "escape"` ← это совпадёт, на самом деле |

**Фикс:** В `actionShortcut` добавить нормализацию перед `return` и назначить `code` для известных спецклавиш:

```typescript
const KEY_ALIASES: Record<string, string> = {
  return: "Enter",
  enter: "Enter",
  space: "Space",
  escape: "Escape",
  esc: "Escape",
  backspace: "Backspace",
  delete: "Delete",
  arrowup: "ArrowUp",
  up: "ArrowUp",
  arrowdown: "ArrowDown",
  down: "ArrowDown",
  arrowleft: "ArrowLeft",
  left: "ArrowLeft",
  arrowright: "ArrowRight",
  right: "ArrowRight",
};
// После normalizedKey:
const code = /^[a-z]$/.test(normalizedKey)
  ? `Key${normalizedKey.toUpperCase()}`
  : (KEY_ALIASES[normalizedKey] ?? null);
```

Тогда `matchesActionShortcut` использует `event.code === shortcut.code` для всех известных клавиш.

---

## Баг 2: Значения полей формы не обновляются при смене props.root

**Файл:** `platform/desktop/src/command-host/useCommandFormView.ts`, строки ~18-28

**Суть:**
`values` (`shallowReactive`) инициализируется один раз в setup-теле through `for` по `form.value.fields`.
Если сервер пушит обновлённый Form-снепшот с новыми полями (добавленными динамически),
`form` computed обновится, но `values` не будет переинициализирован.
Новые поля не получат `defaultValue` → submit отправит `undefined`.

**Фикс:** Добавить `watch(form, ...)` который синхронизирует новые поля в `values` без затирания уже введённых пользователем значений:

```typescript
watch(form, (next) => {
  for (const field of next.fields) {
    if (field.type === "Form.Description" || field.type === "Form.Separator") continue;
    if (field.id in values) continue; // не перезаписываем введённое
    if (typeof field.defaultValue === "boolean") values[field.id] = field.defaultValue;
    else if (Array.isArray(field.defaultValue)) values[field.id] = [...field.defaultValue];
    else values[field.id] = field.defaultValue ?? "";
  }
});
```

---

## Cleanup: `actionSuccessMessage` дублируется 4 раза

**Файлы:**

- `platform/desktop/src/command-host/useCommandListView.ts`
- `platform/desktop/src/command-host/useCommandGridView.ts`
- `platform/desktop/src/command-host/useCommandFormView.ts`
- `platform/desktop/src/command-host/CommandDetailView.vue`

**Суть:** Идентичная функция скопирована 4 раза. При добавлении нового action type нужно обновить 4 места.

**Фикс:** Экспортировать из `model.ts` (там уже есть `ACTION_TYPES` Set с теми же типами):

```typescript
// model.ts
export function actionSuccessMessage(type: string): string { ... }
```

Удалить копии из composable-файлов и импортировать из model.ts.

---

_Записано по результатам code review ветки `claude/vue-perf-optimization-hp1jhk` (2026-06-29)._

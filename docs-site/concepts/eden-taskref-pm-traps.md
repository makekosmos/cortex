---
title: TaskRef × ProseMirror — ловушки
description: Шесть hard-won багов interaction'а между Vue NodeView с native `<input>` и ProseMirror'ом editor'а Eden, которые часами решались. Не наступай снова.
---

# TaskRef × ProseMirror — ловушки

`TaskRef` в Eden — это atom-block в TipTap (`atom: true, group: "block"`), рендерится через Vue NodeView, содержит **native `<input class="task-ref-title-input">`** для title задачи (Pattern B: source of truth = `task_obj` в ARK, document хранит только taskId). Эта комбинация — atom-block + native input внутри + контекст contenteditable=true editor — попадает в шесть очень неочевидных pitfall'ов ProseMirror'а, которые регрессируют от любого мелкого изменения.

Каждый раздел — что сломалось, почему, и единственное правильное лечение. Если правишь `TaskRef.ts`, `TaskRefView.vue` или mouse-flow в `Editor.vue` — прочитай этот файл целиком.

---

## 1. `props.node.nodeSize` для atom-блока врёт

**Симптом.** Enter на task с заполненным title создавал **4 блока** вместо 3: `[taskRef-orig, paragraph-empty, taskRef-new, paragraph-empty]` вместо ожидаемого `[taskRef-orig, taskRef-new, paragraph]`.

**Причина.** `props.node.nodeSize` возвращал **2**, тогда как **реальный** `taskRef` в документе занимал size **1** (atom-leaf). Insert новой ноды по `myPos + node.nodeSize = 0 + 2 = 2` падал ВНУТРЬ следующего пустого параграфа `[1, 3)`, и `tr.insert` split'ил параграф на половинки → лишний пустой блок снизу.

**Источник расхождения.** `Node.nodeSize` в PM считается формулой `isLeaf ? 1 : content.size + 2`. Для нашего taskRef:

- В реальной документной ноде `isLeaf` опирается на `contentMatch === ContentMatch.empty` — true → size = 1.
- В `props.node` (то, что NodeView получает от TipTap) `isLeaf` оказывался false (атрибутивная разница, возможно из-за того как TipTap клонирует ноду в Vue-renderer'е) → формула давала `0 + 2 = 2`.

Документировано в `TaskRefView.vue::commitAndCreateNew`.

**Правильное лечение.** Никогда не доверять `props.node.nodeSize` для атомных нод. Найти actual node в документе по uniq-атрибуту (для нас — `taskId`), взять её nodeSize:

```ts
const doc = editor.view.state.doc;
const targetTaskId = props.node.attrs?.taskId;
let insertPos = -1;
doc.descendants((child, pos) => {
  if (insertPos !== -1) return false;
  if (child.type?.name === "taskRef" && child.attrs?.taskId === targetTaskId) {
    insertPos = pos + child.nodeSize;
    return false;
  }
  return true;
});
```

Аналогично — в `exitToNewParagraph` и любом code path, который вычисляет позицию «после моей atom-ноды».

---

## 2. `StarterKit.trailingNode` навязчиво добавляет пустой параграф

**Симптом.** После каждого Enter в task'е под новой task'ой висит пустой `<p>`, который пользователь не создавал. Даже если в `commitAndCreateNew` явно делаем `tr.replaceWith(from, to, newNode)` или `tr.delete`+`tr.insert`, после dispatch'а PM сразу же возвращает trailing paragraph.

**Причина.** `StarterKit` в TipTap по умолчанию включает `TrailingNode` extension. Он через `appendTransaction` смотрит на последний child документа: если это не textblock (наш taskRef — atom, не textblock), добавляет пустой `<p>` в конец «чтобы было куда поставить курсор».

**Правильное лечение.**

```ts
StarterKit.configure({ codeBlock: false, trailingNode: false }),
```

Отдельный JS-обработчик в `Editor.vue::onContentMouseDown` фокусит editor в конец, если клик пришёлся ниже последнего блока — это покрывает UX-сценарий, который trailingNode пытался решить.

---

## 3. `sel.constructor.name === "NodeSelection"` ломается в production-build'е

**Симптом.** После rubber-band drag'а через 2 задачи + click на ТРЕТЬЮ task: визуально подсвечены **2 задачи** — кликнутая и одна из rubber-band'овой пары.

**Причина — двухступенчатая.**

1. После click PM ставит `NodeSelection(from=2, to=3)` на кликнутую atom-ноду.
2. `TaskRefView.recomputeRangeSelection` подписан на `editor.on("selectionUpdate")` и пересчитывает `isRangeSelected` для своего NodeView. У нас был защитный check:
   ```ts
   if (sel.constructor.name === "NodeSelection") {
     isRangeSelected.value = false;
     return;
   }
   ```
   В **dev-режиме** `constructor.name === "NodeSelection"` → check срабатывает → range-highlight не ставится.
   В **production-build'е** Vite/esbuild минифицирует constructor names → `sel.constructor.name === "e"` или похожее → check НЕ срабатывает → код проваливается в intersection-проверку.
3. Intersection-проверка `nodeFrom < sel.to && nodeTo > sel.from` для NodeSelection с `to = from + nodeSize` touches границу соседнего atom-блока → второй task получает `.is-range-selected`.

**Правильное лечение.** Не полагаться на `constructor.name`. Проверять property, которое property-mangler минификатор не трогает (динамический доступ к свойству объекта):

```ts
// NodeSelection и AllSelection имеют `.node` property; TextSelection нет.
if ((sel as any).node !== undefined) {
  isRangeSelected.value = false;
  return;
}
```

Альтернатива — `import { NodeSelection } from "@tiptap/pm/state"` + `sel instanceof NodeSelection`. Тоже надёжно, но требует не потерять импорт.

**Регрессия — обязательно тестить в production-build'е.** Dev-mode скрывал баг полгода.

---

## 4. ProseMirror крадёт focus с native `<input>` через 4+ путей

**Симптом.** Click на `.task-ref-title-input` → input получает focus на 1-2ms → focus уезжает на `<div class="ProseMirror">` → каретка теряется, юзер не может ввести текст в задаче.

**Это самый болезненный pitfall во всём тред'е.** Множество подходов не решили проблему. Полное лечение комбинирует несколько слоёв.

### Что НЕ работает само по себе

1. **`@mousedown.stop` в Vue-шаблоне на input.** Stop propagation в bubble-фазе не помогает: PM использует **selectionchange** listener на document'е, который срабатывает независимо от bubble. Когда browser ставит focus на input → document selection меняется → PM реагирует через `DOMObserver.onSelectionChange` → `flush()` → `updateSelection` → `view.focus()` → focus уезжает с input на view.dom.

2. **`handleDOMEvents.mousedown(_, e) → return true` в PM-плагине.** Возвращение true предотвращает создание PM'овского `MouseDown` объекта, но **selectionchange listener остаётся активным** и крадёт focus всё равно. Кроме того, в TipTap'овом VueNodeViewRenderer plugin handler регистрируется только для событий, попадающих внутрь view.dom — а селективн change event firing on document.

3. **`handleClickOn` returning true.** Предотвращает `selectClickedLeaf` (NodeSelection не создаётся), но focus всё равно уезжает. PM ставит **TextSelection** ближе всего к click point вместо NodeSelection — но факт focus'а на `.ProseMirror` сохраняется.

4. **`selectable: false` на NodeType.** Убирает NodeSelection, но PM теперь ставит `Selection.near(pos)` — TextSelection в ближайший допустимый textblock. Focus всё равно уезжает на view.dom.

5. **`ignoreMutation: ({mutation}) => mutation.type === "selection"` в NodeView.** В теории должно сказать PM «игнорируй selection-mutation'ы в моей области». На практике DOMObserver всё равно вызывает `flush()`, не помогает.

6. **`stopEvent` в `VueNodeViewRenderer(..., { stopEvent })`.** TipTap core default уже возвращает true для INPUT/BUTTON/SELECT/TEXTAREA targets — то есть PM theoretically не должен обрабатывать клик на наш input. На практике этого не достаточно: PM-вмешательство приходит через selectionchange path, который к stopEvent не имеет отношения.

7. **`draggable: true` на atom-блоке.** PM на mousedown устанавливает `target.draggable = true` на DOM-element, на который пришёлся клик (через `mightDrag` flow). Это меняет browser-семантику focus'а для нашего input. **Решение — `draggable: false`.** Drag-and-drop задач сейчас не используется.

### Что РАБОТАЕТ — focus-defender pattern

Стратегия: **признать что PM украдёт focus и отбить обратно**. Принципы:

- Активировать защиту только когда юзер кликнул именно по input (`onTitleMouseDown` ставит флаг `focusDefenderActive = true`).
- На `input.blur` — если `relatedTarget` это `.ProseMirror`, restore'нуть focus через `requestAnimationFrame`.
- Сохранять `selectionStart` ДО blur (после него value selection недоступна).
- **Деактивировать защиту через document-level mousedown listener (capture phase)**: если очередной mousedown пришёл ВНЕ нашего `.task-ref-row` — пользователь сам захотел уйти, не мешать.

```ts
let focusDefenderActive = false;
let lastCaretBeforeBlur: number | null = null;

function onTitleMouseDown(_e: MouseEvent): void {
  const input = titleInputRef.value;
  if (!input || missing.value) return;
  input.focus();
  focusDefenderActive = true;
}

// onMounted:
const handleDocMouseDown = (event: MouseEvent) => {
  const target = event.target as HTMLElement | null;
  const row = input.closest(".task-ref-row");
  if (!row || !target || !row.contains(target)) {
    focusDefenderActive = false;
  }
};
document.addEventListener("mousedown", handleDocMouseDown, { capture: true });

const handleBlur = (event: FocusEvent) => {
  if (!focusDefenderActive) return;
  lastCaretBeforeBlur = input.selectionStart;
  const next = event.relatedTarget as HTMLElement | null;
  if (next && next.classList?.contains("ProseMirror")) {
    requestAnimationFrame(() => {
      if (!focusDefenderActive) return;
      input.focus();
      if (lastCaretBeforeBlur !== null) {
        input.setSelectionRange(lastCaretBeforeBlur, lastCaretBeforeBlur);
      }
    });
  }
};
input.addEventListener("blur", handleBlur);
```

**Почему капture-phase document listener.** Click на paragraph: mousedown CAPTURE срабатывает раньше bubble-phase input.blur. Успеваем сбросить флаг до того как defender отработает restore. Без этого defender бы крал focus у юзера каждый раз когда он пытается выйти из task'и в paragraph.

**Почему НЕ hammer-цикл `requestAnimationFrame` на 500ms.** Пробовали — focus стабилен только пока цикл работает, после прекращения PM в делегированной задаче (setTimeout 0 из MouseDown.done) забирает focus обратно. Plus hammer мешает любым другим focus-ops в этом window'е. blur+restore через relatedTarget — точечнее и не блокирует.

---

## 5. Click на task-row outside input → клик «в пустоту»

**Симптом.** Кликнул на правую часть task'и (после короткого title, где padding/space до кнопки «открыть»). Никакой реакции — caret не появляется, input не focus'ится.

**Причина.** Vue `@mousedown` на input не срабатывает (target = `.task-ref-row`, не input). PM получает event, обрабатывает по своей логике — ставит TextSelection где-то в `<p>` или NodeSelection на atom, focus на view.dom. Юзер видит «клик впустую».

**Правильное лечение.** Handler на саму row:

```vue
<div
  class="task-ref-row"
  @mousedown="onRowMouseDown"
  contenteditable="false"
>
```

```ts
function onRowMouseDown(e: MouseEvent): void {
  const target = e.target as HTMLElement | null;
  // На сам input или кнопки не вмешиваемся — у них свои handler'ы.
  if (target?.closest(".task-ref-title-input, .task-ref-status, .task-ref-open")) return;
  const input = titleInputRef.value;
  if (!input || missing.value) return;
  input.focus();
  const len = input.value.length;
  input.setSelectionRange(len, len);
  focusDefenderActive = true;
}
```

Клик в padding-зону = focus в input с кареткой в конец title'а.

---

## 6. Block-selection race: после rubber-band + click остаются «выделенными» 2 задачи

**Симптом.** После drag-select 2 блоков → click на любую задачу (или paragraph) — визуально подсвечены 2 task'и.

**Причина — три параллельных слоя selection'а** и watcher с `flush: "post"`:

1. **`.kepler-block-selected`** — decoration через PM `Plugin` + `DecorationSet`, обновляется через Vue watcher на `selectedPositions` с `flush: "post"`. Watcher срабатывает только на следующем microtask'е после dispatch, в течение этого окна decoration видна на старом наборе блоков.
2. **`.is-range-selected`** — Vue ref в каждом NodeView'е, пересчитывается синхронно на `editor.on("selectionUpdate")`. См. (3) — на минифицированном build'е check для NodeSelection ломался, intersection-проверка туда же.
3. **`.is-node-selected`** — Vue ref от `selected` prop NodeViewWrapper'а (PM NodeSelection).

**Правильное лечение — несколько слоёв.**

A. Global window mousedown handler в **capture-фазе** (Editor.vue `handleGlobalMouseDown`): синхронно обнуляет `selectedPositions` И **сразу же** dispatches пустой DecorationSet (`tr.setMeta(blockSelectionPluginKey, buildBlockSelectionDecorations(doc, new Set()))`). Без этого Vue-watcher с `flush: "post"` отстаёт на microtask, и старая decoration видна.

B. В том же handler — collapse PM TextSelection: если она растянулась через несколько блоков (артефакт rubber-band drag'а), `recomputeRangeSelection` подсветит все intersecting blocks. Делаем `tr.setSelection(TextSelection.create(doc, sel.from))` для сворачивания.

C. Флаг `justClearedSelection`: capture-phase global handler чистит, bubble-phase `onContentMouseDown` видит флаг и НЕ запускает новый drag-tracker. Без флага — минимальный jitter мыши + `CROSSING_MIN_PX=8` активирует новый drag через границу блока и опять выделяет соседние.

D. См. (3): надёжная проверка NodeSelection через `.node !== undefined` чтобы `is-range-selected` не падал в intersection-check.

---

## Регрессионные тесты

Хрупкость этого комплекса требует e2e-coverage'а каждой проверки в production-build'е:

- `tests/e2e/eden-task-enter.spec.ts` — Enter в task'е, проверка ровно 2 блоков после (защита от ловушки 1 + 2).
- `tests/e2e/eden-selection-after-click.spec.ts` — 7 click-сценариев после rubber-band'а: A (click input), B (click третья task), C (click row), D (jitter click), E (after Esc), F (right-click), G (click checkbox). Защита от ловушки 3 + 6.

Все эти тесты валидируют **фактический рендеринг** (computed `backgroundColor`), не наличие CSS-классов — потому что классы могут оставаться, но highlight CSS на них может не реагировать (legitimate semantics без visual).

Когда делаешь правки в этой области — **запускай оба spec'а на production-build'е** (`bun run --cwd shell build:js && bunx playwright test ...`). dev-mode прячет ловушку 3 и часть ловушки 4.

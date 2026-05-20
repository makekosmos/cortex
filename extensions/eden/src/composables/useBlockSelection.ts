// useBlockSelection — Anytype-style rubber-band selection layer для Eden
// editor. Полностью отделён от ProseMirror selection. Работает на уровне
// top-level block'ов: paragraph, heading, taskRef, codeBlock, blockquote,
// bulletList, orderedList (всё что является прямым child'ом ProseMirror
// doc).
//
// Архитектура (mirror Anytype `SelectionProvider`):
// - На mousedown в пустом месте editor → start drag, кешируем
//   getBoundingClientRect() всех `.ProseMirror > *` (top-level blocks).
// - На mousemove → обновляем dragRect (от start point до cursor),
//   AABB collision test против кеша, обновляем selectedPositions Set.
// - На mouseup → завершаем drag (dragRect = null), оставляем selection.
// - 20px THRESHOLD перед показом rect — отличает click от drag.
// - Esc clears, Delete/Backspace удаляет выделенные блоки.

import { computed, ref, shallowRef } from "vue";
import type { Editor as TiptapEditor } from "@tiptap/vue-3";
import { TextSelection } from "@tiptap/pm/state";

export interface DragRect {
  /** Видимая часть rect в client coords (для overlay рендера). */
  x: number;
  y: number;
  width: number;
  height: number;
}

interface CachedBlock {
  /** Положение node в doc (через TipTap getPos / view.posAtDOM). */
  pos: number;
  /** DOM element этого блока (direct child of .ProseMirror). */
  el: HTMLElement;
  /** Cached bounding rect in client coords. */
  rect: DOMRect;
}

// 20px = Anytype canonical THRESHOLD. Меньше — drag триггерится случайно
// при обычных кликах с микро-движением мыши.
const DRAG_THRESHOLD_PX = 20;

export function useBlockSelection() {
  /** Set позиций выделенных блоков. Position стабилен в течение drag-session. */
  const selectedPositions = ref<Set<number>>(new Set());

  /** Drag rect для overlay (null когда не в активном drag). */
  const dragRect = shallowRef<DragRect | null>(null);

  /** Start point и cache. Внутренние, не reactive. */
  let startX = 0;
  let startY = 0;
  let hasMoved = false;
  let cache: CachedBlock[] = [];
  let active = false;
  /** Container element для координат overlay (содержит overlay absolute). */
  let containerEl: HTMLElement | null = null;

  const hasSelection = computed(() => selectedPositions.value.size > 0);

  function aabbCollide(a: DragRect, b: DOMRect): boolean {
    return !(
      a.x + a.width < b.left ||
      a.x > b.right ||
      a.y + a.height < b.top ||
      a.y > b.bottom
    );
  }

  function clearSelection(): void {
    if (selectedPositions.value.size === 0) return;
    selectedPositions.value = new Set();
  }

  function isSelected(pos: number): boolean {
    return selectedPositions.value.has(pos);
  }

  /**
   * Начать **tracking** потенциального drag'а. НЕ активирует block-selection
   * сразу — это случится только когда mouse сдвинется >= THRESHOLD (см.
   * `updateDrag`). До threshold юзер по факту просто кликает, и PM
   * обрабатывает mousedown как обычно (focus + caret).
   *
   * Anytype canonical паттерн: mousedown НЕ preventDefault'ит → browser
   * focuses contentEditable. Drag активируется только при движении мыши.
   */
  function startTracking(
    editor: TiptapEditor,
    clientX: number,
    clientY: number,
    container: HTMLElement,
  ): void {
    active = true;
    hasMoved = false;
    startX = clientX;
    startY = clientY;
    containerEl = container;
    cache = collectBlocks(editor);
    // НЕ очищаем PM selection здесь — это произойдёт когда drag
    // активируется (см. updateDrag → если justActivated, caller блюрит PM).
    // НЕ очищаем block selection — может быть юзер shift+click'ает позже.
    // Caller (Editor.vue) ответственен за clearSelection до startTracking
    // если это новый чистый drag.
  }

  /**
   * Принудительно очистить PM selection — вызывается из Editor.vue когда
   * drag только что активировался. Anytype эквивалент: `focus.clear(true)`.
   */
  function collapseEditorSelection(editor: TiptapEditor): void {
    if (!editor.state.selection.empty) {
      const tr = editor.state.tr.setSelection(
        TextSelection.create(editor.state.doc, editor.state.selection.from),
      );
      editor.view.dispatch(tr);
    }
    editor.commands.blur();
  }

  // Container nodes — выделять как ОДНО целое не имеет смысла, юзер хочет
  // выделить отдельные элементы внутри. Рекурсивно ныряем в их children.
  // Anytype model: каждый visual «row» — отдельный selectable блок.
  const CONTAINER_BLOCK_TYPES = new Set([
    "bulletList",
    "orderedList",
    "taskList",
    // blockquote НЕ контейнер для наших целей — Anytype его держит как
    // один блок. То же codeBlock — целиком.
  ]);

  function collectBlocks(editor: TiptapEditor): CachedBlock[] {
    const out: CachedBlock[] = [];
    // Hit-test rect для каждого блока расширяем по горизонтали до полной
    // ширины editor content area. Так drag в левом margin'е (между
    // редактором и блоком) тоже хитит блок. Anytype эквивалент: они
    // тоже хитят по Y-overlap, X — full width. По вертикали — реальный
    // top/height блока (его реальные границы).
    const containerRect = containerEl?.getBoundingClientRect();
    const fullLeft = containerRect?.left ?? 0;
    const fullWidth = containerRect?.width ?? 0;

    editor.state.doc.descendants((node, pos) => {
      if (CONTAINER_BLOCK_TYPES.has(node.type.name)) {
        return true; // recurse в children
      }
      if (!node.isBlock) {
        return false; // inline / text — пропускаем
      }
      const dom = editor.view.nodeDOM(pos);
      if (dom instanceof HTMLElement) {
        const blockRect = dom.getBoundingClientRect();
        // DOMRect конструктор принимает x/y/w/h. Заменяем horizontal
        // bounds на содержащий контейнер — vertical оставляем реальные.
        const expandedRect = new DOMRect(
          fullLeft,
          blockRect.top,
          fullWidth,
          blockRect.height,
        );
        out.push({ pos, el: dom, rect: expandedRect });
      }
      return false;
    });
    return out;
  }

  /**
   * Обновляет tracking по mousemove. Возвращает `"activated"` ровно ОДИН
   * раз — когда drag только что прошёл threshold (нужно caller'у чтобы
   * blur'нуть editor + collapse selection). Дальше возвращает `"dragging"`.
   * До threshold возвращает `null`.
   */
  function updateDrag(clientX: number, clientY: number): "activated" | "dragging" | null {
    if (!active) return null;

    const dx = Math.abs(clientX - startX);
    const dy = Math.abs(clientY - startY);
    const wasMoved = hasMoved;
    if (!hasMoved && dx < DRAG_THRESHOLD_PX && dy < DRAG_THRESHOLD_PX) {
      return null;
    }
    hasMoved = true;
    const justActivated = !wasMoved;

    const x = Math.min(startX, clientX);
    const y = Math.min(startY, clientY);
    const width = Math.abs(clientX - startX);
    const height = Math.abs(clientY - startY);

    // Rect в координатах container'а (для absolute overlay).
    if (containerEl) {
      const containerRect = containerEl.getBoundingClientRect();
      dragRect.value = {
        x: x - containerRect.left,
        y: y - containerRect.top,
        width,
        height,
      };
    }

    // Hit test против кеша. Drag rect в client coords для совпадения
    // с DOMRect (left/top/right/bottom тоже client coords).
    const clientDragRect: DragRect = { x, y, width, height };
    const next = new Set<number>();
    for (const block of cache) {
      if (aabbCollide(clientDragRect, block.rect)) {
        next.add(block.pos);
      }
    }
    selectedPositions.value = next;
    return justActivated ? "activated" : "dragging";
  }

  function finishDrag(): void {
    if (!active) return;
    active = false;
    dragRect.value = null;
    containerEl = null;
    cache = [];
    // selection остаётся (юзер может нажать Delete или Esc).
  }

  function cancelDrag(): void {
    finishDrag();
    clearSelection();
  }

  /**
   * Удалить все выделенные блоки. Для taskRef нод дополнительно
   * вызывает softDeleteTask чтобы task_obj не висели orphan'ами в Delphi.
   * Returns Promise который resolve'ит после всех async вызовов.
   */
  async function deleteSelected(
    editor: TiptapEditor,
    softDeleteTask: (taskId: string) => Promise<void>,
  ): Promise<void> {
    if (selectedPositions.value.size === 0) return;
    // Сначала собираем taskId'ы из taskRef нод чтобы soft-delete
    // ARK объекты. Делаем ДО мутации tr, иначе positions сдвинутся.
    const taskIdsToDelete: string[] = [];
    const positionsDesc = Array.from(selectedPositions.value).sort((a, b) => b - a);
    for (const pos of positionsDesc) {
      const node = editor.state.doc.nodeAt(pos);
      if (node?.type.name === "taskRef" && typeof node.attrs.taskId === "string") {
        taskIdsToDelete.push(node.attrs.taskId);
      }
    }

    // Удаляем блоки одной транзакцией (descending → positions не сдвигаются
    // во время accumulation).
    const tr = editor.state.tr;
    for (const pos of positionsDesc) {
      const node = tr.doc.nodeAt(pos);
      if (!node) continue;
      tr.delete(pos, pos + node.nodeSize);
    }
    editor.view.dispatch(tr);
    clearSelection();

    // Soft-delete ARK objects параллельно. Не блокируем UI ack.
    await Promise.allSettled(
      taskIdsToDelete.map((id) =>
        softDeleteTask(id).catch((err) => {
          console.warn("[eden block-selection] soft-delete failed", id, err);
        }),
      ),
    );
  }

  return {
    selectedPositions,
    dragRect,
    hasSelection,
    isSelected,
    clearSelection,
    startTracking,
    updateDrag,
    finishDrag,
    cancelDrag,
    collapseEditorSelection,
    deleteSelected,
  };
}

export type UseBlockSelectionReturn = ReturnType<typeof useBlockSelection>;

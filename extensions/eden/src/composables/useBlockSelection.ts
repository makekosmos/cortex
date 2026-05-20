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

const DRAG_THRESHOLD_PX = 5; // меньше чем Anytype (20) — наш UX чуть отзывчивее

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
   * Стартует drag-сессию. clientX/Y — coords в viewport. containerElement
   * — для overlay расчёта (rect относительно него).
   * Кеширует все top-level блоки editor'а.
   */
  function startDrag(
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
    // Не чистим selection сразу — если юзер сделал shift+click pattern в
    // будущем, можно extend. Пока тоже чистим (как Anytype без модификатора).
    clearSelection();
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
    // descendants walk: для container типов return true (recurse), для leaf
    // block return false (record + не диваем глубже), для inline возвращаем
    // false и пропускаем.
    editor.state.doc.descendants((node, pos) => {
      if (CONTAINER_BLOCK_TYPES.has(node.type.name)) {
        return true; // recurse в children
      }
      if (!node.isBlock) {
        return false; // inline / text — пропускаем, не walk внутрь
      }
      const dom = editor.view.nodeDOM(pos);
      if (dom instanceof HTMLElement) {
        out.push({ pos, el: dom, rect: dom.getBoundingClientRect() });
      }
      // Leaf block записан — не диваем в его внутренности, чтобы не
      // селектить parent + child одновременно. Если внутри листайтема
      // есть nested list — пользователь хочет выделить весь list item
      // как целое, не его части отдельно.
      return false;
    });
    return out;
  }

  /**
   * Обновляет drag по mousemove. Возвращает true если drag прошёл threshold
   * и rect стал visible (caller может предотвратить дефолтное text-selection
   * через event.preventDefault).
   */
  function updateDrag(clientX: number, clientY: number): boolean {
    if (!active) return false;

    const dx = Math.abs(clientX - startX);
    const dy = Math.abs(clientY - startY);
    if (!hasMoved && dx < DRAG_THRESHOLD_PX && dy < DRAG_THRESHOLD_PX) {
      return false;
    }
    hasMoved = true;

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
    return true;
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
    startDrag,
    updateDrag,
    finishDrag,
    cancelDrag,
    deleteSelected,
  };
}

export type UseBlockSelectionReturn = ReturnType<typeof useBlockSelection>;

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

// 20px = Anytype canonical THRESHOLD для same-block case.
// 8px = minimum для block-crossing activation (отсекает click jitter
// в 1-2px на boundary между блоками).
const DRAG_THRESHOLD_PX = 20;
const CROSSING_MIN_PX = 8;

// Auto-scroll: пока drag активен и курсор у верх/низ края scroll контейнера,
// двигаем scrollTop. Edge zone 48px, max скорость ~16px/frame proporcional
// глубине курсора в зону.
const AUTO_SCROLL_EDGE_PX = 48;
const AUTO_SCROLL_MAX_SPEED_PX = 16;

export function computeBlockSelectionAutoScrollDelta(
  clientY: number,
  container: HTMLElement,
): number {
  const rect = container.getBoundingClientRect();
  const fromTop = clientY - rect.top;
  const fromBottom = rect.bottom - clientY;
  if (fromTop < AUTO_SCROLL_EDGE_PX && fromTop < fromBottom) {
    const depth = Math.max(0, AUTO_SCROLL_EDGE_PX - fromTop);
    const intensity = Math.min(1, depth / AUTO_SCROLL_EDGE_PX);
    return -Math.ceil(intensity * AUTO_SCROLL_MAX_SPEED_PX);
  }
  if (fromBottom < AUTO_SCROLL_EDGE_PX) {
    const depth = Math.max(0, AUTO_SCROLL_EDGE_PX - fromBottom);
    const intensity = Math.min(1, depth / AUTO_SCROLL_EDGE_PX);
    return Math.ceil(intensity * AUTO_SCROLL_MAX_SPEED_PX);
  }
  return 0;
}

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
  /** Scroll container (`.kosmos-scroll` ancestor) для auto-scroll. */
  let scrollContainerEl: HTMLElement | null = null;
  /** Editor reference сохраняем для re-collectBlocks после auto-scroll tick'а. */
  let editorRef: TiptapEditor | null = null;
  /** Последние известные client coords мыши — обновляются в `updateDrag`,
      используются auto-scroll tick'ом чтобы пересчитать selection после
      того как контент проскроллился (мышь стоит на месте, контент уехал). */
  let lastClientX = 0;
  let lastClientY = 0;
  /** rAF id active auto-scroll loop'а (null если не запущен). */
  let autoScrollRafId: number | null = null;
  /** Pos блока на котором был mousedown (null если в margin'е). Используется
      чтобы НЕ активировать block-drag пока курсор остаётся в этом же блоке —
      даём юзеру нормально выделять text в одной строке для copy/paste. */
  let startBlockPos: number | null = null;

  const hasSelection = computed(() => selectedPositions.value.size > 0);

  function aabbCollide(a: DragRect, b: DOMRect): boolean {
    return !(a.x + a.width < b.left || a.x > b.right || a.y + a.height < b.top || a.y > b.bottom);
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
    lastClientX = clientX;
    lastClientY = clientY;
    containerEl = container;
    editorRef = editor;
    scrollContainerEl = container.closest(".kosmos-scroll") as HTMLElement | null;
    cache = collectBlocks(editor);
    // Запомнили на каком блоке (если был) был mousedown. Drag активируется
    // только когда курсор уходит ИЗ этого блока — иначе юзер просто
    // выделяет text в одной строке для copy.
    startBlockPos = findBlockAtY(clientY);
    // НЕ очищаем PM selection здесь — это произойдёт когда drag
    // активируется (см. updateDrag → если justActivated, caller блюрит PM).
  }

  /** Возвращает pos блока, в Y-range которого попадает clientY. null если
      курсор в margin'е (между блоками или вне всех). */
  function findBlockAtY(clientY: number): number | null {
    for (const block of cache) {
      if (clientY >= block.rect.top && clientY <= block.rect.bottom) {
        return block.pos;
      }
    }
    return null;
  }

  /**
   * Принудительно очистить PM selection + native browser selection —
   * вызывается из Editor.vue когда drag только что активировался.
   * Anytype эквивалент: `focus.clear(true)` + selection-ranges'й
   * `removeAllRanges()`.
   *
   * Три шага (в этом порядке):
   * 1. PM TextSelection collapse — обновляет PM model.
   * 2. `editor.commands.blur()` — снимает focus с contentEditable.
   * 3. `window.getSelection().removeAllRanges()` — убирает browser
   *    native Range (главное!). Без этого `::selection` продолжает
   *    рисоваться на тексте даже после PM-blur'а, потому что
   *    браузерный Range живёт независимо от PM TextSelection.
   */
  function collapseEditorSelection(editor: TiptapEditor): void {
    if (!editor.state.selection.empty) {
      const tr = editor.state.tr.setSelection(
        TextSelection.create(editor.state.doc, editor.state.selection.from),
      );
      editor.view.dispatch(tr);
    }
    editor.commands.blur();
    // Browser-level: разрушаем native Range. Это убирает visible
    // ::selection на тексте. PM не делает это автоматически — он держит
    // свою TextSelection, а браузерный Range отдельный артефакт.
    const sel = window.getSelection();
    if (sel) sel.removeAllRanges();
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
        const expandedRect = new DOMRect(fullLeft, blockRect.top, fullWidth, blockRect.height);
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
  /** Пересчитать dragRect + hit-test selection из current startX/Y + lastClientX/Y.
      Вызывается после auto-scroll tick'а (startY сдвинут, cache пересобран). */
  function recomputeDragGeometry(): void {
    if (!containerEl) return;
    const x = Math.min(startX, lastClientX);
    const y = Math.min(startY, lastClientY);
    const width = Math.abs(lastClientX - startX);
    const height = Math.abs(lastClientY - startY);
    const containerRect = containerEl.getBoundingClientRect();
    dragRect.value = {
      x: x - containerRect.left,
      y: y - containerRect.top,
      width,
      height,
    };
    const clientDragRect: DragRect = { x, y, width, height };
    const next = new Set<number>();
    for (const block of cache) {
      if (aabbCollide(clientDragRect, block.rect)) {
        next.add(block.pos);
      }
    }
    selectedPositions.value = next;
  }

  /** rAF-loop пока drag активен. На каждом tick'е:
      1. Если курсор у края — двигаем scrollTop scrollContainerEl.
      2. Сдвигаем startY на величину реального скролла (anchor drag origin
         к документу, не к viewport — иначе rect «убегал» бы от точки
         нажатия после скролла).
      3. Пересобираем cache блоков (их rects изменились со скроллом).
      4. Пересчитываем dragRect + selection — даже если мышь стоит.
      Loop планирует сам себя пока `active && hasMoved`. */
  function autoScrollTick(): void {
    autoScrollRafId = null;
    if (!active || !hasMoved) return;
    if (scrollContainerEl && editorRef) {
      const delta = computeBlockSelectionAutoScrollDelta(lastClientY, scrollContainerEl);
      if (delta !== 0) {
        const before = scrollContainerEl.scrollTop;
        scrollContainerEl.scrollTop = before + delta;
        const actual = scrollContainerEl.scrollTop - before;
        if (actual !== 0) {
          // Контент проехал в client coords на -actual. Чтобы стартовая
          // точка drag'а оставалась прибита к документу — двигаем startY
          // вместе с ней.
          startY -= actual;
          cache = collectBlocks(editorRef);
          recomputeDragGeometry();
          // Стираем native browser Range, который мог переустановиться.
          const sel = window.getSelection();
          if (sel && sel.rangeCount > 0) sel.removeAllRanges();
        }
      }
    }
    if (active && hasMoved) {
      autoScrollRafId = requestAnimationFrame(autoScrollTick);
    }
  }

  function ensureAutoScrollLoop(): void {
    if (autoScrollRafId !== null) return;
    if (!scrollContainerEl) return;
    autoScrollRafId = requestAnimationFrame(autoScrollTick);
  }

  function updateDrag(clientX: number, clientY: number): "activated" | "dragging" | null {
    if (!active) return null;
    lastClientX = clientX;
    lastClientY = clientY;

    const dx = Math.abs(clientX - startX);
    const dy = Math.abs(clientY - startY);
    const wasMoved = hasMoved;
    if (!hasMoved) {
      // Минимальный jitter filter: даже на boundary между блоками
      // tiny mouse jitter (1-2px) при click без движения может
      // пересечь findBlockAtY границу. Требуем 8px чтобы это был
      // реальный drag намерения.
      if (dx < CROSSING_MIN_PX && dy < CROSSING_MIN_PX) {
        return null;
      }
      // Block-crossing check: если cursor ушёл в ДРУГОЙ блок относительно
      // mousedown'а — активируем сразу (8px достаточно, не ждём 20px).
      // Task row ~26px высотой, dy ~13px уже в следующем блоке.
      if (startBlockPos !== null) {
        const currentBlock = findBlockAtY(clientY);
        if (currentBlock !== startBlockPos) {
          hasMoved = true;
        }
      }
      if (!hasMoved) {
        // Same block (или drag в margin без block-crossing) — нужен
        // 20px полный threshold чтобы не мешать text-selection в строке.
        if (dx < DRAG_THRESHOLD_PX && dy < DRAG_THRESHOLD_PX) {
          return null;
        }
        if (startBlockPos !== null) {
          const currentBlock = findBlockAtY(clientY);
          if (currentBlock === startBlockPos) {
            return null;
          }
        }
        hasMoved = true;
      }
    }
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
    ensureAutoScrollLoop();
    return justActivated ? "activated" : "dragging";
  }

  function finishDrag(): void {
    if (!active) return;
    active = false;
    if (autoScrollRafId !== null) {
      cancelAnimationFrame(autoScrollRafId);
      autoScrollRafId = null;
    }
    dragRect.value = null;
    containerEl = null;
    scrollContainerEl = null;
    editorRef = null;
    cache = [];
    startBlockPos = null;
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

  /**
   * Select all **top-level** blocks. Используется для Ctrl+A.
   *
   * NB: НЕ переиспользуем `collectBlocks()` — он рекурсит внутрь
   * CONTAINER_BLOCK_TYPES (bulletList / taskList / blockquote) и собирает
   * вложенные listItem pos'ы. Но `serializeSelectedBlocksAsMarkdown` в
   * Editor.vue фильтрует по top-level positions. Несовпадение → md пустой.
   * Поэтому здесь собираем строго top-level через doc.forEach.
   *
   * Collapses PM selection как side-effect — иначе native ctrl+a накладывает
   * текстовый range layer поверх block overlay'а, визуально шумно.
   */
  function selectAll(editor: TiptapEditor): void {
    const positions: number[] = [];
    editor.state.doc.forEach((_node, offset) => {
      positions.push(offset);
    });
    if (positions.length === 0) return;
    selectedPositions.value = new Set(positions);
    collapseEditorSelection(editor);
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
    selectAll,
  };
}

export type UseBlockSelectionReturn = ReturnType<typeof useBlockSelection>;

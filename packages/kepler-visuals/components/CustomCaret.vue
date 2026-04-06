<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";

// ---------------------------------------------------------------------------
// Hardcoded settings — adjust here, not via props
// ---------------------------------------------------------------------------

const CARET_WIDTH = 2;
const IDLE_DELAY = 530;
const BLINK_SPEED = 1000;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const caretRef = ref<HTMLElement>();

let activeTarget: HTMLElement | null = null;
let isComposing = false;
let isIdle = false;
let idleTimer: number | null = null;
let rafId: number | null = null;
const disposers: Array<() => void> = [];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function findEditable(node: Node | null): HTMLElement | null {
  if (!node) return null;
  const el = node instanceof HTMLElement ? node : node.parentElement;
  if (!el) return null;
  const editable = el.closest("[contenteditable='true']");
  return editable instanceof HTMLElement ? editable : null;
}

function listen<K extends keyof DocumentEventMap>(
  target: EventTarget,
  event: K | string,
  handler: EventListener,
  opts?: boolean | AddEventListenerOptions,
) {
  target.addEventListener(event, handler, opts);
  disposers.push(() => target.removeEventListener(event, handler, opts));
}

// ---------------------------------------------------------------------------
// Native caret control
// ---------------------------------------------------------------------------

function hideNativeCaret(el: HTMLElement) {
  el.style.setProperty("caret-color", "transparent", "important");
}

function restoreNativeCaret(el: HTMLElement) {
  el.style.removeProperty("caret-color");
}

function showNativeCaretForIME() {
  if (activeTarget) {
    activeTarget.style.setProperty("caret-color", "auto", "important");
  }
}

// ---------------------------------------------------------------------------
// Active target management
// ---------------------------------------------------------------------------

function setActiveTarget(next: HTMLElement | null) {
  if (activeTarget === next) return;

  if (activeTarget) {
    restoreNativeCaret(activeTarget);
  }

  activeTarget = next;
  isComposing = false;
  isIdle = false;

  if (activeTarget) {
    hideNativeCaret(activeTarget);
  }

  scheduleIdleCheck();
}

function syncTargetFromSelection() {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0) {
    setActiveTarget(null);
    return;
  }
  setActiveTarget(findEditable(sel.anchorNode));
}

// ---------------------------------------------------------------------------
// Idle / blink
// ---------------------------------------------------------------------------

function clearIdleTimer() {
  if (idleTimer !== null) {
    window.clearTimeout(idleTimer);
    idleTimer = null;
  }
}

function scheduleIdleCheck() {
  clearIdleTimer();

  if (!activeTarget || isComposing) {
    if (isIdle) {
      isIdle = false;
      requestRender();
    }
    return;
  }

  idleTimer = window.setTimeout(() => {
    idleTimer = null;
    if (!activeTarget || isComposing || isIdle) return;
    isIdle = true;
    requestRender();
  }, IDLE_DELAY);
}

function markActivity() {
  if (isIdle) isIdle = false;
  scheduleIdleCheck();
}

// ---------------------------------------------------------------------------
// Caret rect (Selection API)
// ---------------------------------------------------------------------------

function getCaretRect(): DOMRect | null {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || !sel.isCollapsed) return null;

  const range = sel.getRangeAt(0);
  if (!activeTarget?.contains(range.startContainer)) return null;

  // Primary: getClientRects on collapsed range
  const collapsed = range.cloneRange();
  collapsed.collapse(true);

  const rects = collapsed.getClientRects();
  if (rects.length > 0) {
    const r = rects[0]!;
    if (r.height > 0) return r;
  }

  // Fallback: getBoundingClientRect
  const bounding = collapsed.getBoundingClientRect();
  if (bounding.height > 0) return bounding;

  // Last resort: zero-width marker (for empty elements / Android WebView)
  const marker = document.createElement("span");
  marker.textContent = "\u200b";
  marker.style.cssText = "font-size:inherit;line-height:inherit;position:static;";

  try {
    collapsed.insertNode(marker);
    const markerRect = marker.getBoundingClientRect();
    marker.remove();

    // Restore selection after DOM mutation
    const restored = document.createRange();
    restored.setStart(range.startContainer, range.startOffset);
    restored.collapse(true);
    sel.removeAllRanges();
    sel.addRange(restored);

    if (markerRect.height > 0) return markerRect;
  } catch {
    marker.remove();
  }

  return null;
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

function requestRender() {
  if (rafId !== null) return;
  rafId = window.requestAnimationFrame(() => {
    rafId = null;
    render();
  });
}

function render() {
  const el = caretRef.value;
  if (!el) return;

  if (!activeTarget || (isComposing && !isIMEAndroid())) {
    el.classList.add("kepler-caret--hidden");
    return;
  }

  const rect = getCaretRect();
  if (!rect) {
    el.classList.add("kepler-caret--hidden");
    return;
  }

  el.style.left = `${rect.left}px`;
  el.style.top = `${rect.top}px`;
  el.style.height = `${rect.height}px`;
  el.style.width = `${CARET_WIDTH}px`;

  el.classList.remove("kepler-caret--hidden");
  el.classList.toggle("kepler-caret--blink", isIdle);
}

function isIMEAndroid(): boolean {
  return /android/i.test(navigator.userAgent);
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

function onFocusIn(e: Event) {
  const target = findEditable(e.target as Node);
  if (target) {
    setActiveTarget(target);
    markActivity();
    requestRender();
  }
}

function onFocusOut() {
  // Defer to let new focus settle
  window.setTimeout(() => {
    syncTargetFromSelection();
    requestRender();
  }, 0);
}

function onSelectionChange() {
  const prev = activeTarget;
  syncTargetFromSelection();

  if (activeTarget && prev === activeTarget) {
    markActivity();
  }
  requestRender();
}

function onActivity() {
  if (!activeTarget || isComposing) return;
  markActivity();
  requestRender();
}

function onCompositionStart(e: Event) {
  if (isIMEAndroid()) return;
  const target = findEditable(e.target as Node);
  if (!target) return;

  setActiveTarget(target);
  isComposing = true;
  clearIdleTimer();
  if (isIdle) isIdle = false;

  showNativeCaretForIME();
  requestRender();
}

function onCompositionEnd(e: Event) {
  if (isIMEAndroid()) return;
  const target = findEditable(e.target as Node);
  if (!target) return;

  setActiveTarget(target);
  isComposing = false;
  hideNativeCaret(target);
  markActivity();
  requestRender();
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

onMounted(() => {
  listen(document, "focusin", onFocusIn, true);
  listen(document, "focusout", onFocusOut, true);
  listen(document, "selectionchange", onSelectionChange);
  listen(document, "keydown", onActivity, true);
  listen(document, "input", onActivity, true);
  listen(document, "pointerdown", onActivity, true);
  listen(document, "compositionstart", onCompositionStart, true);
  listen(document, "compositionend", onCompositionEnd, true);
  listen(window as unknown as EventTarget, "resize", requestRender);
  listen(window as unknown as EventTarget, "scroll", requestRender, true);

  // Initial sync
  syncTargetFromSelection();
  requestRender();
});

onUnmounted(() => {
  clearIdleTimer();
  if (rafId !== null) {
    window.cancelAnimationFrame(rafId);
    rafId = null;
  }
  if (activeTarget) {
    restoreNativeCaret(activeTarget);
    activeTarget = null;
  }
  for (const dispose of disposers) dispose();
  disposers.length = 0;
});
</script>

<template>
  <Teleport to="body">
    <span
      ref="caretRef"
      class="kepler-caret kepler-caret--hidden"
      aria-hidden="true"
    />
  </Teleport>
</template>

<style>
.kepler-caret {
  position: fixed;
  z-index: 9999;
  pointer-events: none;
  background: var(--foreground, #d4d4d4);
  border-radius: 1px;
  will-change: left, top, height;
  transition: left 0.02s ease, top 0.02s ease;
}

.kepler-caret--hidden {
  opacity: 0;
  visibility: hidden;
}

.kepler-caret--blink {
  animation: kepler-caret-blink v-bind('BLINK_SPEED + "ms"') ease-in-out infinite;
}

@keyframes kepler-caret-blink {
  0%, 30% { opacity: 1; }
  50% { opacity: 0; }
  70%, 100% { opacity: 1; }
}
</style>

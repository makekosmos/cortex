<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";

// ---------------------------------------------------------------------------
// Hardcoded settings — adjust here, not via props
// ---------------------------------------------------------------------------

const CARET_WIDTH = 3;
const IDLE_DELAY = 530;
const BLINK_SPEED = 1000;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

const caretRef = ref<HTMLElement>();

let activeTarget: HTMLElement | HTMLInputElement | HTMLTextAreaElement | null = null;
let targetKind: "editable" | "input" | null = null;
let isComposing = false;
let isIdle = false;
let idleTimer: number | null = null;
let rafId: number | null = null;
const disposers: Array<() => void> = [];

// Mirror element for measuring input/textarea caret position
let mirror: HTMLDivElement | null = null;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function isInputLike(el: Element): el is HTMLInputElement | HTMLTextAreaElement {
  const tag = el.tagName;
  return (
    (tag === "INPUT" && (el as HTMLInputElement).type === "text") ||
    tag === "TEXTAREA"
  );
}

function findTarget(eventTarget: EventTarget | null): {
  el: HTMLElement;
  kind: "editable" | "input";
} | null {
  if (!eventTarget || !(eventTarget instanceof HTMLElement)) return null;

  if (eventTarget.closest(".ProseMirror")) {
    return null;
  }

  // Check input/textarea first
  if (isInputLike(eventTarget)) {
    return { el: eventTarget, kind: "input" };
  }

  // Then contenteditable
  const editable = eventTarget.closest("[contenteditable='true']");
  if (editable instanceof HTMLElement) {
    return { el: editable, kind: "editable" };
  }

  return null;
}

function listen(
  target: EventTarget,
  event: string,
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

// ---------------------------------------------------------------------------
// Active target management
// ---------------------------------------------------------------------------

function setActiveTarget(
  next: HTMLElement | null,
  kind: "editable" | "input" | null,
) {
  if (activeTarget === next) return;

  if (activeTarget) {
    restoreNativeCaret(activeTarget);
  }

  activeTarget = next;
  targetKind = kind;
  isComposing = false;
  isIdle = false;

  if (activeTarget) {
    hideNativeCaret(activeTarget);
  }

  scheduleIdleCheck();
}

function syncTargetFromFocus() {
  const focused = document.activeElement;
  if (!focused || focused === document.body) {
    setActiveTarget(null, null);
    return;
  }

  const found = findTarget(focused);
  setActiveTarget(found?.el ?? null, found?.kind ?? null);
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
// Caret rect — contenteditable (Selection API)
// ---------------------------------------------------------------------------

function getEditableCaretRect(): DOMRect | null {
  const sel = window.getSelection();
  if (!sel || sel.rangeCount === 0 || !sel.isCollapsed) return null;

  const range = sel.getRangeAt(0);
  if (!activeTarget?.contains(range.startContainer)) return null;

  const collapsed = range.cloneRange();
  collapsed.collapse(true);

  const rects = collapsed.getClientRects();
  if (rects.length > 0) {
    const r = rects[0]!;
    if (r.height > 0) return r;
  }

  const bounding = collapsed.getBoundingClientRect();
  if (bounding.height > 0) return bounding;

  // Fallback: zero-width marker
  const marker = document.createElement("span");
  marker.textContent = "\u200b";
  marker.style.cssText =
    "font-size:inherit;line-height:inherit;position:static;";

  try {
    collapsed.insertNode(marker);
    const markerRect = marker.getBoundingClientRect();
    marker.remove();

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
// Caret rect — input/textarea (mirror technique)
// ---------------------------------------------------------------------------

const MIRROR_COPY_PROPS = [
  "boxSizing",
  "width",
  "height",
  "overflowX",
  "overflowY",
  "borderTopWidth",
  "borderRightWidth",
  "borderBottomWidth",
  "borderLeftWidth",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "fontStyle",
  "fontVariant",
  "fontWeight",
  "fontStretch",
  "fontSize",
  "fontSizeAdjust",
  "lineHeight",
  "fontFamily",
  "textAlign",
  "textTransform",
  "textIndent",
  "textDecoration",
  "letterSpacing",
  "wordSpacing",
  "tabSize",
  "whiteSpace",
  "wordWrap",
  "wordBreak",
  "direction",
] as const;

function ensureMirror(): HTMLDivElement {
  if (mirror) return mirror;
  mirror = document.createElement("div");
  mirror.setAttribute("aria-hidden", "true");
  Object.assign(mirror.style, {
    position: "absolute",
    top: "0",
    left: "-9999px",
    visibility: "hidden",
    whiteSpace: "pre-wrap",
    wordWrap: "break-word",
  });
  document.body.appendChild(mirror);
  return mirror;
}

function getInputCaretRect(): DOMRect | null {
  const input = activeTarget as HTMLInputElement | HTMLTextAreaElement;
  if (input.selectionStart == null) return null;
  const pos = input.selectionStart;

  const isTextarea = input.tagName === "TEXTAREA";
  const computed = window.getComputedStyle(input);
  const m = ensureMirror();

  // Copy styles from input to mirror
  for (const prop of MIRROR_COPY_PROPS) {
    m.style[prop as any] = computed[prop as any];
  }

  if (!isTextarea) {
    m.style.whiteSpace = "pre";
    m.style.overflowX = "hidden";
    m.style.height = "auto";
  }

  // Set content up to caret position
  const text = input.value.substring(0, pos);
  // Replace spaces with nbsp to preserve whitespace in div
  m.textContent = text.replace(/ /g, "\u00a0");

  // Add a marker span at caret position
  const span = document.createElement("span");
  span.textContent = "\u200b";
  m.appendChild(span);

  const inputRect = input.getBoundingClientRect();
  const spanRect = span.getBoundingClientRect();
  const mirrorRect = m.getBoundingClientRect();

  // offsetX/Y already includes border+padding (mirror has same styles)
  const offsetX = spanRect.left - mirrorRect.left;
  const offsetY = spanRect.top - mirrorRect.top;

  const scrollLeft = input.scrollLeft || 0;
  const scrollTop = input.scrollTop || 0;

  const left = inputRect.left + offsetX - scrollLeft;
  const top = inputRect.top + offsetY - scrollTop;
  const height = spanRect.height || parseFloat(computed.lineHeight) || parseFloat(computed.fontSize) * 1.2;

  return new DOMRect(left, top, CARET_WIDTH, height);
}

// ---------------------------------------------------------------------------
// Unified caret rect
// ---------------------------------------------------------------------------

function getCaretRect(): DOMRect | null {
  if (!activeTarget) return null;
  if (targetKind === "input") return getInputCaretRect();
  return getEditableCaretRect();
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

  const isAndroid = /android/i.test(navigator.userAgent);

  if (!activeTarget || (isComposing && !isAndroid)) {
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

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

function onFocusIn(e: Event) {
  const found = findTarget(e.target as HTMLElement);
  if (found) {
    setActiveTarget(found.el, found.kind);
    markActivity();
    requestRender();
  }
}

function onFocusOut() {
  window.setTimeout(() => {
    syncTargetFromFocus();
    requestRender();
  }, 0);
}

function onSelectionChange() {
  const prev = activeTarget;
  syncTargetFromFocus();

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
  if (/android/i.test(navigator.userAgent)) return;
  const found = findTarget(e.target as HTMLElement);
  if (!found) return;

  setActiveTarget(found.el, found.kind);
  isComposing = true;
  clearIdleTimer();
  if (isIdle) isIdle = false;

  if (activeTarget) {
    activeTarget.style.setProperty("caret-color", "auto", "important");
  }
  requestRender();
}

function onCompositionEnd(e: Event) {
  if (/android/i.test(navigator.userAgent)) return;
  const found = findTarget(e.target as HTMLElement);
  if (!found) return;

  setActiveTarget(found.el, found.kind);
  isComposing = false;
  hideNativeCaret(found.el);
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
  listen(window, "resize", requestRender);
  listen(window, "scroll", requestRender, true);

  syncTargetFromFocus();
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
  if (mirror?.parentElement) {
    mirror.parentElement.removeChild(mirror);
    mirror = null;
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
  background: var(--accent);
  border-radius: 1px;
  will-change: left, top, height;
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

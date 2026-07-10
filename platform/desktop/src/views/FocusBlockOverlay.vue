<script setup lang="ts">
import { Check } from "@lucide/vue";
import { computed, nextTick, onMounted, onUnmounted, shallowRef } from "vue";
import type { FocusOverlayFeedback } from "@shared/ipc-api-shell-services";

const feedback = shallowRef<FocusOverlayFeedback | null>(null);
const active = shallowRef(false); // плашка/градиент в DOM
const edgesOn = shallowRef(false); // градиент по краям (fade 300ms)
const popupOn = shallowRef(false); // плашка (slide 300ms сверху)
const isBlocked = computed(() => feedback.value?.kind === "blocked");
const toneClass = computed(() =>
  feedback.value ? `focus-overlay--${feedback.value.kind}` : undefined,
);
const feedbackLabel = computed(() =>
  feedback.value?.kind === "completed" ? "Задача выполнена" : "Заблокировано во время фокуса",
);

// Hold-to-confirm на кнопке «Открыть»: при наведении отсчёт 3→2→1, после —
// кнопка становится кликабельной.
const holdState = shallowRef<"idle" | "counting" | "ready">("idle");
const countdown = shallowRef(3);

let edgesTimer: ReturnType<typeof setTimeout> | null = null;
let popupTimer: ReturnType<typeof setTimeout> | null = null;
let cleanupTimer: ReturnType<typeof setTimeout> | null = null;
let leaveTimer: ReturnType<typeof setTimeout> | null = null;
let holdTimer: ReturnType<typeof setInterval> | null = null;
let offShow = () => {};

const EDGES_HOLD_MS = 3000;
const POPUP_HOLD_MS = 5000;
const EXIT_MS = 340;

function clearTimers(): void {
  for (const t of [edgesTimer, popupTimer, cleanupTimer, leaveTimer]) if (t) clearTimeout(t);
  edgesTimer = popupTimer = cleanupTimer = leaveTimer = null;
}

function resetHold(): void {
  if (holdTimer) {
    clearInterval(holdTimer);
    holdTimer = null;
  }
  holdState.value = "idle";
  countdown.value = 3;
}

function show(nextFeedback: FocusOverlayFeedback): void {
  clearTimers();
  resetHold();
  feedback.value = nextFeedback;
  active.value = true;
  edgesOn.value = false;
  popupOn.value = false;
  // Элементы монтируются в начальном состоянии, затем включаем — CSS transition
  // отыгрывает fade-in градиента и slide-down плашки.
  requestAnimationFrame(() => {
    edgesOn.value = true;
    popupOn.value = true;
  });
  edgesTimer = setTimeout(() => {
    edgesOn.value = false;
  }, EDGES_HOLD_MS);
  schedulePopupHide(POPUP_HOLD_MS);
}

function schedulePopupHide(delay: number): void {
  if (popupTimer) clearTimeout(popupTimer);
  popupTimer = setTimeout(() => {
    popupOn.value = false;
    cleanupTimer = setTimeout(() => finish(), EXIT_MS);
  }, delay);
}

function finish(): void {
  clearTimers();
  resetHold();
  active.value = false;
  edgesOn.value = false;
  popupOn.value = false;
  void window.kepler.focusOverlay.setInteractive(false);
  // Ждём один тик Vue-рендера (DOM очищен), затем сигналим main скрыть окно.
  void nextTick(() => window.kepler.focusOverlay.done());
}

// --- hover плашки: пауза авто-скрытия + интерактивность окна ---------------
function onPopupEnter(): void {
  if (!isBlocked.value) return;
  if (popupTimer) {
    clearTimeout(popupTimer);
    popupTimer = null;
  }
  if (cleanupTimer) {
    clearTimeout(cleanupTimer);
    cleanupTimer = null;
  }
  if (leaveTimer) {
    clearTimeout(leaveTimer);
    leaveTimer = null;
  }
  void window.kepler.focusOverlay.setInteractive(true);
}

function onPopupLeave(): void {
  if (!isBlocked.value) return;
  cancelHold();
  void window.kepler.focusOverlay.setInteractive(false);
  // После увода курсора — даём плашке уехать через короткую паузу.
  schedulePopupHide(600);
}

// --- hold-to-confirm на кнопке --------------------------------------------
function startHold(): void {
  if (holdState.value === "ready") return;
  holdState.value = "counting";
  countdown.value = 3;
  if (holdTimer) clearInterval(holdTimer);
  holdTimer = setInterval(() => {
    countdown.value -= 1;
    if (countdown.value <= 0) {
      if (holdTimer) {
        clearInterval(holdTimer);
        holdTimer = null;
      }
      holdState.value = "ready";
    }
  }, 1000);
}

function cancelHold(): void {
  if (holdTimer) {
    clearInterval(holdTimer);
    holdTimer = null;
  }
  holdState.value = "idle";
  countdown.value = 3;
}

async function onOpenClick(): Promise<void> {
  const current = feedback.value;
  if (holdState.value !== "ready" || current?.kind !== "blocked") return;
  const id = current.id;
  finish();
  await window.kepler.focusSession.snoozeApp(id);
  await new Promise((r) => setTimeout(r, 200));
  try {
    await window.kepler.ark.request("app_index.launch", { id });
  } catch {
    /* ignore */
  }
}

onMounted(() => {
  offShow = window.kepler.focusOverlay.onShow(show);
  window.kepler.focusOverlay.ready();
});

onUnmounted(() => {
  offShow();
  clearTimers();
  resetHold();
});
</script>

<template>
  <div class="focus-overlay" :class="toneClass">
    <div
      v-if="active"
      class="focus-overlay__edges"
      :class="{ 'focus-overlay__edges--on': edgesOn }"
      aria-hidden="true"
    />
    <div
      v-if="active && feedback"
      class="focus-overlay__popup"
      :class="{
        'focus-overlay__popup--on': popupOn,
        'focus-overlay__popup--completed': feedback.kind === 'completed',
      }"
      @mouseenter="onPopupEnter"
      @mouseleave="onPopupLeave"
    >
      <div class="focus-overlay__popup-inner">
        <img
          v-if="feedback.kind === 'blocked' && feedback.icon"
          :src="feedback.icon"
          class="focus-overlay__icon"
          alt=""
        />
        <div v-else class="focus-overlay__icon-fallback">
          <Check v-if="feedback.kind === 'completed'" :size="20" />
          <template v-else>{{ feedback.title.charAt(0).toUpperCase() }}</template>
        </div>
        <div class="focus-overlay__body">
          <div class="focus-overlay__label">{{ feedbackLabel }}</div>
          <div class="focus-overlay__title">{{ feedback.title }}</div>
        </div>
        <button
          v-if="feedback.kind === 'blocked'"
          type="button"
          class="focus-overlay__open"
          :class="{ 'focus-overlay__open--ready': holdState === 'ready' }"
          @mouseenter="startHold"
          @mouseleave="cancelHold"
          @click="onOpenClick"
        >
          {{ holdState === "counting" ? countdown : "Открыть" }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.focus-overlay {
  position: fixed;
  inset: 0;
  pointer-events: none;
  overflow: hidden;
  --feedback-accent: var(--status-warning);
}

.focus-overlay--completed {
  --feedback-accent: var(--status-success);
}

.focus-overlay__edges {
  position: fixed;
  inset: 0;
  pointer-events: none;
  opacity: 0;
  transition: opacity 300ms ease;
  background:
    linear-gradient(
      to bottom,
      color-mix(in srgb, var(--feedback-accent) 32%, transparent) 0%,
      transparent 16%
    ),
    linear-gradient(
      to top,
      color-mix(in srgb, var(--feedback-accent) 18%, transparent) 0%,
      transparent 10%
    ),
    linear-gradient(
      to right,
      color-mix(in srgb, var(--feedback-accent) 18%, transparent) 0%,
      transparent 7%
    ),
    linear-gradient(
      to left,
      color-mix(in srgb, var(--feedback-accent) 18%, transparent) 0%,
      transparent 7%
    );
}

.focus-overlay__edges--on {
  opacity: 1;
}

.focus-overlay__popup {
  position: fixed;
  top: 16px;
  left: 50%;
  pointer-events: auto;
  opacity: 0;
  transform: translateX(-50%) translateY(-140%);
  transition:
    transform 300ms cubic-bezier(0.2, 0, 0, 1),
    opacity 220ms ease;
}

.focus-overlay__popup--on {
  opacity: 1;
  transform: translateX(-50%) translateY(0);
}

.focus-overlay__popup--completed {
  pointer-events: none;
}

.focus-overlay__popup-inner {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-radius: 12px;
  border: 1px solid color-mix(in srgb, var(--foreground, #fff) 12%, transparent);
  background: var(
    --popover,
    color-mix(in srgb, var(--background, #1e1e20) 96%, var(--foreground, #fff) 4%)
  );
  min-width: 360px;
  max-width: 500px;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  box-shadow:
    0 8px 32px color-mix(in srgb, var(--background) 55%, transparent),
    0 0 0 1px color-mix(in srgb, var(--feedback-accent) 18%, transparent);
}

.focus-overlay__icon {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  flex-shrink: 0;
  object-fit: cover;
}

.focus-overlay__icon-fallback {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--feedback-accent) 16%, transparent);
  color: var(--feedback-accent);
  font-size: 16px;
  font-weight: 700;
}

.focus-overlay__body {
  flex: 1;
  min-width: 0;
}

.focus-overlay__label {
  font-size: 11px;
  font-weight: 600;
  color: color-mix(in srgb, var(--foreground, #fff) 50%, transparent);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 2px;
}

.focus-overlay__title {
  font-size: 14px;
  font-weight: 700;
  color: var(--foreground, #fff);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.focus-overlay__open {
  flex-shrink: 0;
  min-width: 86px;
  height: 32px;
  padding: 0 14px;
  border: 1px solid color-mix(in srgb, var(--foreground, #fff) 16%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground, #fff) 7%, transparent);
  color: color-mix(in srgb, var(--foreground, #fff) 70%, transparent);
  font: inherit;
  font-size: 13px;
  font-weight: 650;
  cursor: default;
  white-space: nowrap;
  transition:
    background-color 140ms,
    color 140ms,
    border-color 140ms;
}

/* Во время отсчёта (3,2,1) — кнопка не кликабельна, центрированная цифра. */
.focus-overlay__open:not(.focus-overlay__open--ready) {
  pointer-events: auto;
}

.focus-overlay__open--ready {
  border-color: color-mix(in srgb, var(--foreground, #fff) 32%, transparent);
  background: color-mix(in srgb, var(--foreground, #fff) 16%, transparent);
  color: var(--foreground, #fff);
}

.focus-overlay__open--ready:hover {
  background: color-mix(in srgb, var(--foreground, #fff) 24%, transparent);
}
</style>

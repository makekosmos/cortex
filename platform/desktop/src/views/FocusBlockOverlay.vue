<script setup lang="ts">
import { ref, nextTick, onMounted, onUnmounted } from "vue";

interface BlockedApp {
  id: string;
  title: string;
  icon?: string | null;
}

const app = ref<BlockedApp | null>(null);
const active = ref(false); // плашка/градиент в DOM
const edgesOn = ref(false); // градиент по краям (fade 300ms)
const popupOn = ref(false); // плашка (slide 300ms сверху)

// Hold-to-confirm на кнопке «Открыть»: при наведении отсчёт 3→2→1, после —
// кнопка становится кликабельной.
const holdState = ref<"idle" | "counting" | "ready">("idle");
const countdown = ref(3);

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

function show(blocked: BlockedApp): void {
  clearTimers();
  resetHold();
  app.value = blocked;
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
  if (holdState.value !== "ready" || !app.value) return;
  const id = app.value.id;
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
  offShow = window.kepler.focusOverlay.onShow((blocked) => show(blocked));
  window.kepler.focusOverlay.ready();
});

onUnmounted(() => {
  offShow();
  clearTimers();
  resetHold();
});
</script>

<template>
  <div class="focus-overlay">
    <div
      v-if="active"
      class="focus-overlay__edges"
      :class="{ 'focus-overlay__edges--on': edgesOn }"
      aria-hidden="true"
    />
    <div
      v-if="active && app"
      class="focus-overlay__popup"
      :class="{ 'focus-overlay__popup--on': popupOn }"
      @mouseenter="onPopupEnter"
      @mouseleave="onPopupLeave"
    >
      <div class="focus-overlay__popup-inner">
        <img v-if="app.icon" :src="app.icon" class="focus-overlay__icon" alt="" />
        <div v-else class="focus-overlay__icon-fallback">
          {{ app.title.charAt(0).toUpperCase() }}
        </div>
        <div class="focus-overlay__body">
          <div class="focus-overlay__label">Заблокировано во время фокуса</div>
          <div class="focus-overlay__title">{{ app.title }}</div>
        </div>
        <button
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
      color-mix(in srgb, var(--timer-work, oklch(0.55 0.22 22)) 32%, transparent) 0%,
      transparent 16%
    ),
    linear-gradient(
      to top,
      color-mix(in srgb, var(--timer-work, oklch(0.55 0.22 22)) 18%, transparent) 0%,
      transparent 10%
    ),
    linear-gradient(
      to right,
      color-mix(in srgb, var(--timer-work, oklch(0.55 0.22 22)) 18%, transparent) 0%,
      transparent 7%
    ),
    linear-gradient(
      to left,
      color-mix(in srgb, var(--timer-work, oklch(0.55 0.22 22)) 18%, transparent) 0%,
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
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.45);
  min-width: 360px;
  max-width: 500px;
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
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
  background: color-mix(in srgb, var(--foreground, #fff) 12%, transparent);
  color: var(--foreground, #fff);
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

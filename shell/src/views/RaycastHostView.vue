<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import { DesktopChrome, DesktopContentSurface, type ToastTone } from "@kosmos/visuals";
import type { RaycastFeedbackEvent, RaycastSnapshot } from "../../shared/raycast-ipc";
import RaycastFormView from "../raycast-host/RaycastFormView.vue";
import RaycastGridView from "../raycast-host/RaycastGridView.vue";
import RaycastListView from "../raycast-host/RaycastListView.vue";
import RaycastDetailView from "../raycast-host/RaycastDetailView.vue";
import RaycastMenuBarExtraView from "../raycast-host/RaycastMenuBarExtraView.vue";

const snapshot = shallowRef<RaycastSnapshot | null>(null);
const error = shallowRef<string | null>(null);
const feedbackItems = shallowRef<
  Array<{
    id: number;
    title: string;
    message: string | null;
    tone: ToastTone;
    loading: boolean;
  }>
>([]);
let nextFeedbackId = 1;
let stopFeedback: (() => void) | null = null;
let stopSnapshotUpdates: (() => void) | null = null;

const sessionId = computed(() => {
  const raw = window.location.hash.split("?", 2)[1] ?? "";
  return new URLSearchParams(raw).get("session");
});

onMounted(async () => {
  const id = sessionId.value;
  if (!id) {
    error.value = "Сессия Raycast не передана";
    return;
  }
  try {
    stopFeedback = window.kepler.raycast.onFeedback(id, showFeedback);
    if (typeof window.kepler.raycast.onSnapshotUpdated === "function") {
      stopSnapshotUpdates = window.kepler.raycast.onSnapshotUpdated(id, (nextSnapshot) => {
        snapshot.value = nextSnapshot;
        error.value = null;
      });
    }
    snapshot.value = await window.kepler.raycast.snapshot(id);
    if (!snapshot.value) error.value = "Сессия Raycast не найдена";
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  }
});

onBeforeUnmount(() => {
  stopFeedback?.();
  stopFeedback = null;
  stopSnapshotUpdates?.();
  stopSnapshotUpdates = null;
});

function feedbackTone(event: RaycastFeedbackEvent): ToastTone {
  if (event.style === "success") return "success";
  if (event.style === "failure") return "error";
  return "info";
}

function showFeedback(event: RaycastFeedbackEvent): void {
  const id = nextFeedbackId++;
  feedbackItems.value = [
    ...feedbackItems.value.slice(-2),
    {
      id,
      title: event.kind === "hud" ? "HUD" : event.title,
      message: event.kind === "hud" ? event.title : (event.message ?? null),
      tone: feedbackTone(event),
      loading: event.style === "animated",
    },
  ];
  if (event.style !== "animated") {
    window.setTimeout(() => {
      feedbackItems.value = feedbackItems.value.filter((item) => item.id !== id);
    }, 2200);
  }
}

function dismissFeedback(id: number): void {
  feedbackItems.value = feedbackItems.value.filter((item) => item.id !== id);
}
</script>

<template>
  <DesktopChrome platform="windows" :title="snapshot?.commandTitle ?? 'Raycast'">
    <DesktopContentSurface class="raycast-host">
      <RaycastListView
        v-if="snapshot?.root.type === 'List'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <RaycastFormView
        v-else-if="snapshot?.root.type === 'Form'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <RaycastGridView
        v-else-if="snapshot?.root.type === 'Grid'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <RaycastDetailView
        v-else-if="snapshot?.root.type === 'Detail'"
        :detail="snapshot.root"
        :session-id="snapshot.sessionId"
        class="raycast-host__detail"
      />
      <RaycastMenuBarExtraView
        v-else-if="snapshot?.root.type === 'MenuBarExtra'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <section v-else class="raycast-host__state">
        <h1 class="raycast-host__title">{{ snapshot?.commandTitle ?? "Raycast" }}</h1>
        <p class="raycast-host__text">
          {{ error ?? "Этот тип Raycast view пока не поддерживается" }}
        </p>
      </section>
    </DesktopContentSurface>

    <div v-if="feedbackItems.length > 0" class="raycast-host__feedback" aria-live="polite">
      <button
        v-for="item in feedbackItems"
        :key="item.id"
        class="raycast-host__feedback-card"
        :class="{
          'raycast-host__feedback-card--success': item.tone === 'success',
          'raycast-host__feedback-card--error': item.tone === 'error',
        }"
        type="button"
        @click="dismissFeedback(item.id)"
      >
        <span v-if="item.loading" class="raycast-host__feedback-spinner" aria-hidden="true" />
        <span class="raycast-host__feedback-text">
          <strong class="raycast-host__feedback-title">{{ item.title }}</strong>
          <span v-if="item.message" class="raycast-host__feedback-message">{{ item.message }}</span>
        </span>
      </button>
    </div>
  </DesktopChrome>
</template>

<style scoped>
.raycast-host {
  display: flex;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  background: color-mix(in srgb, var(--background) 94%, transparent);
}

.raycast-host__state {
  display: grid;
  place-content: center;
  gap: 8px;
  min-height: 0;
  flex: 1;
  padding: 24px;
  text-align: center;
}

.raycast-host__title {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
}

.raycast-host__text {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 13px;
}

.raycast-host__detail {
  max-width: none;
  flex: 1;
  border-left: 0;
}

.raycast-host__feedback {
  position: fixed;
  top: 48px;
  right: 16px;
  z-index: 20;
  display: grid;
  width: min(320px, calc(100vw - 32px));
  gap: 8px;
  pointer-events: none;
}

.raycast-host__feedback-card {
  display: flex;
  min-width: 0;
  align-items: flex-start;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--background) 88%, var(--foreground) 12%);
  color: var(--foreground);
  padding: 10px 12px;
  box-shadow: 0 12px 34px color-mix(in srgb, var(--foreground) 18%, transparent);
  pointer-events: auto;
  text-align: left;
}

.raycast-host__feedback-card--success {
  border-color: color-mix(in srgb, var(--accent) 56%, var(--border));
}

.raycast-host__feedback-card--error {
  border-color: color-mix(in srgb, var(--destructive) 56%, var(--border));
}

.raycast-host__feedback-spinner {
  width: 14px;
  height: 14px;
  flex: 0 0 auto;
  margin-top: 1px;
  border: 2px solid color-mix(in srgb, var(--foreground) 20%, transparent);
  border-top-color: var(--accent);
  border-radius: 999px;
  animation: raycast-feedback-spin 800ms linear infinite;
}

.raycast-host__feedback-text {
  display: grid;
  min-width: 0;
  gap: 3px;
}

.raycast-host__feedback-title,
.raycast-host__feedback-message {
  overflow: hidden;
  text-overflow: ellipsis;
}

.raycast-host__feedback-title {
  font-size: 13px;
  font-weight: 700;
}

.raycast-host__feedback-message {
  color: var(--muted-foreground);
  font-size: 12px;
}

@keyframes raycast-feedback-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>

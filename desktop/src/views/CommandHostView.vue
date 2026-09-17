<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import { DesktopChrome, DesktopContentSurface, usePlatform, type ToastTone } from "@kosmos/visuals";
import type { CommandFeedbackEvent, CommandSnapshot } from "../../shared/command-ipc";
import { isFunction } from "../shared/runtimeGuards";
import CommandFormView from "../command-host/CommandFormView.vue";
import CommandGridView from "../command-host/CommandGridView.vue";
import CommandListView from "../command-host/CommandListView.vue";
import CommandDetailView from "../command-host/CommandDetailView.vue";
import CommandMenuBarExtraView from "../command-host/CommandMenuBarExtraView.vue";

const snapshot = shallowRef<CommandSnapshot | null>(null);
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
const { platform } = usePlatform();

onMounted(async () => {
  const id = sessionId.value;
  if (!id) {
    error.value = "Сессия команды не передана";
    return;
  }
  try {
    stopFeedback = window.kepler.command.onFeedback(id, showFeedback);
    if (isFunction(window.kepler.command.onSnapshotUpdated)) {
      stopSnapshotUpdates = window.kepler.command.onSnapshotUpdated(id, (nextSnapshot) => {
        snapshot.value = nextSnapshot;
        error.value = null;
      });
    }
    snapshot.value = await window.kepler.command.snapshot(id);
    if (!snapshot.value) error.value = "Сессия команды не найдена";
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

function feedbackTone(event: CommandFeedbackEvent): ToastTone {
  if (event.style === "success") return "success";
  if (event.style === "failure") return "error";
  return "info";
}

function showFeedback(event: CommandFeedbackEvent): void {
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
  <DesktopChrome :platform="platform" :title="snapshot?.commandTitle ?? 'Команда'">
    <DesktopContentSurface class="command-host">
      <CommandListView
        v-if="snapshot?.root.type === 'List'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <CommandFormView
        v-else-if="snapshot?.root.type === 'Form'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <CommandGridView
        v-else-if="snapshot?.root.type === 'Grid'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <CommandDetailView
        v-else-if="snapshot?.root.type === 'Detail'"
        :detail="snapshot.root"
        :session-id="snapshot.sessionId"
        class="command-host__detail"
      />
      <CommandMenuBarExtraView
        v-else-if="snapshot?.root.type === 'MenuBarExtra'"
        :root="snapshot.root"
        :session-id="snapshot.sessionId"
      />
      <section v-else class="command-host__state">
        <h1 class="command-host__title">{{ snapshot?.commandTitle ?? "Команда" }}</h1>
        <p class="command-host__text">
          {{ error ?? "Этот тип команды пока не поддерживается" }}
        </p>
      </section>
    </DesktopContentSurface>

    <div v-if="feedbackItems.length > 0" class="command-host__feedback" aria-live="polite">
      <button
        v-for="item in feedbackItems"
        :key="item.id"
        class="command-host__feedback-card"
        :class="{
          'command-host__feedback-card--success': item.tone === 'success',
          'command-host__feedback-card--error': item.tone === 'error',
        }"
        type="button"
        @click="dismissFeedback(item.id)"
      >
        <span v-if="item.loading" class="command-host__feedback-spinner" aria-hidden="true" />
        <span class="command-host__feedback-text">
          <strong class="command-host__feedback-title">{{ item.title }}</strong>
          <span v-if="item.message" class="command-host__feedback-message">{{ item.message }}</span>
        </span>
      </button>
    </div>
  </DesktopChrome>
</template>

<style scoped>
.command-host {
  display: flex;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  background: color-mix(in srgb, var(--background) 94%, transparent);
}

.command-host__state {
  display: grid;
  place-content: center;
  gap: 8px;
  min-height: 0;
  flex: 1;
  padding: 24px;
  text-align: center;
}

.command-host__title {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
}

.command-host__text {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 13px;
}

.command-host__detail {
  max-width: none;
  flex: 1;
  border-left: 0;
}

.command-host__feedback {
  position: fixed;
  top: 48px;
  right: 16px;
  z-index: 20;
  display: grid;
  width: min(320px, calc(100vw - 32px));
  gap: 8px;
  pointer-events: none;
}

.command-host__feedback-card {
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

.command-host__feedback-card--success {
  border-color: color-mix(in srgb, var(--accent) 56%, var(--border));
}

.command-host__feedback-card--error {
  border-color: color-mix(in srgb, var(--destructive) 56%, var(--border));
}

.command-host__feedback-spinner {
  width: 14px;
  height: 14px;
  flex: 0 0 auto;
  margin-top: 1px;
  border: 2px solid color-mix(in srgb, var(--foreground) 20%, transparent);
  border-top-color: var(--accent);
  border-radius: var(--radius-pill, 999px);
  animation: command-feedback-spin 800ms linear infinite;
}

.command-host__feedback-text {
  display: grid;
  min-width: 0;
  gap: 3px;
}

.command-host__feedback-title,
.command-host__feedback-message {
  overflow: hidden;
  text-overflow: ellipsis;
}

.command-host__feedback-title {
  font-size: 13px;
  font-weight: 700;
}

.command-host__feedback-message {
  color: var(--muted-foreground);
  font-size: 12px;
}

@keyframes command-feedback-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>

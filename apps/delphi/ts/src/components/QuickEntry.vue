<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import { useRoute } from "vue-router";
import { QuickEntryPanel } from "@kepler/visuals";
import type { QuickEntrySavePayload } from "@kepler/visuals";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { useQuickEntry } from "@/composables/useQuickEntry";

const { open, hide } = useQuickEntry();
const store = useTodoStore();
const { projects } = storeToRefs(store);
const route = useRoute();

const todayIso = () => new Date().toISOString().slice(0, 10);

const defaultScheduledDate = computed<string | null>(() =>
  route.path === "/today" ? todayIso() : null,
);

const defaultProjectId = computed<string | null>(() => {
  const match = route.path.match(/^\/project\/(.+)$/);
  return match ? match[1] : null;
});

function onSave(payload: QuickEntrySavePayload) {
  const isToday =
    payload.scheduledDate !== null && payload.scheduledDate === todayIso();
  store.addTodo({
    title: payload.title,
    notes: payload.notes,
    scheduledDate: payload.scheduledDate,
    projectId: payload.projectId,
    isToday,
    billable: payload.billable,
    price: payload.price,
  });
}

const handler = (e: KeyboardEvent) => {
  if ((e.metaKey || e.ctrlKey) && e.key === "n" && !e.shiftKey && !e.altKey) {
    e.preventDefault();
    if (open.value) { hide(); } else { open.value = true; }
  }
};

onMounted(() => {
  window.addEventListener("keydown", handler);
});

onUnmounted(() => {
  window.removeEventListener("keydown", handler);
});
</script>

<template>
  <QuickEntryPanel
    v-model:open="open"
    :projects="projects"
    :default-scheduled-date="defaultScheduledDate"
    :default-project-id="defaultProjectId"
    @save="onSave"
  />
</template>

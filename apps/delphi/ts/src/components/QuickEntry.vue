<script setup lang="ts">
import { onMounted, onUnmounted } from "vue";
import { QuickEntryPanel } from "@kepler/visuals";
import type { QuickEntrySavePayload } from "@kepler/visuals";
import { useTodoStore } from "@/store/todos";
import { storeToRefs } from "pinia";
import { useQuickEntry } from "@/composables/useQuickEntry";

const { open, hide } = useQuickEntry();
const store = useTodoStore();
const { projects } = storeToRefs(store);

function onSave(payload: QuickEntrySavePayload) {
  store.addTodo({
    title: payload.title,
    notes: payload.notes,
    isToday: payload.isToday,
    isEvening: payload.isEvening,
    scheduledDate: payload.scheduledDate,
    projectId: payload.projectId,
  });
}

onMounted(() => {
  const handler = (e: KeyboardEvent) => {
    if (e.metaKey && e.key === "n" && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      if (open.value) { hide(); } else { open.value = true; }
    }
  };
  window.addEventListener("keydown", handler);
  onUnmounted(() => window.removeEventListener("keydown", handler));
});
</script>

<template>
  <QuickEntryPanel
    v-model:open="open"
    :projects="projects"
    @save="onSave"
  />
</template>

<script setup lang="ts">
import { ref } from "vue";
import QuickEntryPanel, {
  type QuickEntryProject,
  type QuickEntrySavePayload,
} from "../../components/QuickEntryPanel.vue";

const openBasic = ref(true);
const openWithProjects = ref(true);

const projects: QuickEntryProject[] = [
  { id: "p-eden", title: "Eden — journal" },
  { id: "p-kepler", title: "Kepler shell" },
  { id: "p-client", title: "Acme Co. consulting", billable: true },
  { id: "p-personal", title: "Личное" },
];

const lastSave = ref<QuickEntrySavePayload | null>(null);

function onSave(payload: QuickEntrySavePayload) {
  lastSave.value = payload;
}
</script>

<template>
  <Story title="QuickEntryPanel" group="complex" :layout="{ type: 'single', iframe: true }">
    <Variant title="Минимальный — только title + date">
      <div class="story-canvas">
        <p class="story-label">
          Enter — сохранить. Escape / клик вне — закрыть. Пустой title = отмена.
        </p>
        <QuickEntryPanel v-model:open="openBasic" @save="onSave" />
      </div>
    </Variant>

    <Variant title="С проектами и billable">
      <div class="story-canvas">
        <p class="story-label">
          Folder-иконка раскрывает project picker. Если у проекта `billable=true` —
          подставляется $ при выборе.
        </p>
        <QuickEntryPanel
          v-model:open="openWithProjects"
          :projects="projects"
          default-project-id="p-kepler"
          default-scheduled-date="2026-05-16"
          @save="onSave"
        />
      </div>
    </Variant>

    <Variant title="Эмиссия save">
      <div class="story-canvas">
        <p class="story-label">Сохранённое payload последнего save</p>
        <pre v-if="lastSave" class="payload">{{ JSON.stringify(lastSave, null, 2) }}</pre>
        <p v-else class="muted">Ничего не сохранено. Используй варианты выше — введи title + Enter.</p>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.payload {
  font-family: var(--font-mono);
  font-size: 0.75rem;
  background: var(--secondary);
  padding: 0.75rem;
  border-radius: var(--radius);
  white-space: pre-wrap;
  max-width: 480px;
}
.muted {
  color: var(--muted-foreground);
  font-size: 0.85rem;
}
</style>

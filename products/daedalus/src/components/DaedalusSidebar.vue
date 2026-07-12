<script setup lang="ts">
import { Archive, Bot, FolderGit2, FolderPlus, Plus, Settings2 } from "@lucide/vue";
import type { AgentsProject, AgentsSession } from "@kosmos/ark/agents";

defineProps<{
  projects: AgentsProject[];
  sessions: AgentsSession[];
  archivedSessions: AgentsSession[];
  selectedId: string | null;
}>();
const emit = defineEmits<{ select: [id: string]; newSession: []; addProject: [] }>();

const statusLabel: Record<string, string> = {
  starting: "Запуск",
  running: "Работает",
  waiting_approval: "Нужно решение",
  completed: "Готово",
  interrupted: "Остановлено",
  failed: "Ошибка",
  archived: "Архив",
};
</script>

<template>
  <aside class="sidebar">
    <header class="brand">
      <span class="brand__mark"><Bot :size="18" /></span><strong>Daedalus</strong>
    </header>
    <button class="primary sidebar__new" type="button" @click="emit('newSession')">
      <Plus :size="16" />Новая задача
    </button>
    <button class="ghost sidebar__add-project" type="button" @click="emit('addProject')">
      <FolderPlus :size="15" />Добавить проект
    </button>
    <div class="sidebar__scroll">
      <section v-for="project in projects" :key="project.id" class="project">
        <div class="project__title">
          <FolderGit2 :size="15" /><span>{{ project.name }}</span
          ><span v-if="project.dirty" class="dirty" title="Есть незакоммиченные изменения">●</span>
        </div>
        <button
          v-for="session in sessions.filter((item) => item.projectId === project.id)"
          :key="session.id"
          class="session-row"
          :class="{ 'session-row--active': session.id === selectedId }"
          type="button"
          @click="emit('select', session.id)"
        >
          <span class="session-row__title">{{ session.title }}</span>
          <span class="session-row__status" :data-status="session.status">{{
            statusLabel[session.status] ?? session.status
          }}</span>
        </button>
      </section>
      <section v-if="archivedSessions.length" class="project archive-section">
        <div class="project__title"><Archive :size="15" /><span>Архив</span></div>
        <button
          v-for="session in archivedSessions"
          :key="session.id"
          class="session-row"
          :class="{ 'session-row--active': session.id === selectedId }"
          type="button"
          @click="emit('select', session.id)"
        >
          <span class="session-row__title">{{ session.title }}</span>
          <span class="session-row__status">{{
            session.worktreeExists ? "Рабочая копия сохранена" : "Рабочая копия удалена"
          }}</span>
        </button>
      </section>
    </div>
    <footer class="sidebar__footer"><Settings2 :size="14" />Codex CLI</footer>
  </aside>
</template>

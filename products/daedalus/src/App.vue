<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef, watch } from "vue";
import { Archive, GitCompareArrows, PanelRightOpen, Trash2 } from "@lucide/vue";
import { DesktopChrome } from "@kosmos/visuals";
import DaedalusSidebar from "@/components/DaedalusSidebar.vue";
import StartScreen from "@/components/StartScreen.vue";
import SessionTimeline from "@/components/SessionTimeline.vue";
import SessionComposer from "@/components/SessionComposer.vue";
import ChangesPanel from "@/components/ChangesPanel.vue";
import { useAgentsStore } from "@/stores/agents";

const store = useAgentsStore();
const changesOpen = shallowRef(true);
const confirmRemoveWorktree = shallowRef(false);
let unsubscribe: () => void = () => undefined;
const running = computed(() =>
  ["starting", "running", "waiting_approval"].includes(store.selectedSession?.status ?? ""),
);
const statusLabels: Record<string, string> = {
  starting: "Запуск",
  running: "Работает",
  waiting_approval: "Нужно решение",
  completed: "Готово",
  interrupted: "Остановлено",
  failed: "Ошибка",
  archived: "Архив",
};
onMounted(async () => {
  unsubscribe = await store.initialize();
});
onBeforeUnmount(() => unsubscribe());
watch(
  () => store.selectedSessionId,
  () => (confirmRemoveWorktree.value = false),
);
async function removeWorktree(): Promise<void> {
  if (await store.removeWorktree()) confirmRemoveWorktree.value = false;
}
</script>

<template>
  <DesktopChrome appearance="settings" platform="windows" class="app-shell">
    <template #sidebar
      ><DaedalusSidebar
        :projects="store.projects"
        :sessions="store.sessions"
        :archived-sessions="store.archivedSessions"
        :selected-id="store.selectedSessionId"
        @new-session="store.newSession"
        @add-project="store.addProject"
        @select="store.selectSession"
    /></template>
    <template #titlebar-center><span class="window-title">Daedalus · Codex</span></template>
    <StartScreen
      v-if="!store.selectedSession"
      :projects="store.projects"
      :models="store.models"
      :loading="store.loading"
      @add-project="store.addProject"
      @create="store.createSession"
    />
    <main v-else class="session-view">
      <header class="session-header">
        <div>
          <span class="eyebrow">{{ store.selectedSession.branch }}</span>
          <h1>{{ store.selectedSession.title }}</h1>
        </div>
        <div class="session-header__actions">
          <span class="status-pill" :data-status="store.selectedSession.status">{{
            statusLabels[store.selectedSession.status] ?? store.selectedSession.status
          }}</span>
          <template
            v-if="
              store.selectedSession.status === 'archived' && store.selectedSession.worktreeExists
            "
          >
            <template v-if="confirmRemoveWorktree">
              <span class="remove-confirmation">Удалить чистую рабочую копию?</span>
              <button class="ghost" type="button" @click="confirmRemoveWorktree = false">
                Отмена
              </button>
              <button class="destructive-button" type="button" @click="removeWorktree">
                Удалить
              </button>
            </template>
            <button v-else class="secondary" type="button" @click="confirmRemoveWorktree = true">
              <Trash2 :size="16" />Удалить рабочую копию
            </button>
          </template>
          <button
            v-if="store.selectedSession.status !== 'archived'"
            class="ghost icon-button"
            type="button"
            title="Архивировать"
            @click="store.archive"
          >
            <Archive :size="16" /></button
          ><button v-if="!changesOpen" class="secondary" type="button" @click="changesOpen = true">
            <PanelRightOpen :size="16" />Изменения
          </button>
        </div>
      </header>
      <div class="session-body">
        <section class="conversation">
          <SessionTimeline
            :events="store.timeline"
            :approval="store.pendingApproval"
            :has-older="store.timelineCursor !== null"
            :loading-older="store.timelineLoadingOlder"
            @load-older="store.loadOlder"
            @respond="store.respond"
          /><SessionComposer
            v-if="store.selectedSession.status !== 'archived'"
            :running="running"
            @send="store.send"
            @interrupt="store.interrupt"
          />
        </section>
        <ChangesPanel
          v-if="changesOpen"
          :diff="store.diff"
          :editors="store.editors"
          :worktree-exists="store.selectedSession.worktreeExists"
          @close="changesOpen = false"
          @refresh="store.loadDiff"
          @open="store.openEditor"
        />
      </div>
    </main>
    <div v-if="store.error" class="error-toast">
      <GitCompareArrows :size="16" />{{ store.error }}
    </div>
  </DesktopChrome>
</template>

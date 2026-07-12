<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { FolderPlus, Play } from "@lucide/vue";
import type { AgentsMode, AgentsModel, AgentsProject } from "@kosmos/ark/agents";

const props = defineProps<{ projects: AgentsProject[]; models: AgentsModel[]; loading: boolean }>();
const emit = defineEmits<{
  addProject: [];
  create: [
    input: {
      projectId: string;
      prompt: string;
      mode: AgentsMode;
      model?: string;
      fullAccessConfirmed?: boolean;
    },
  ];
}>();
const projectId = shallowRef("");
const prompt = shallowRef("");
const mode = shallowRef<AgentsMode>("default");
const model = shallowRef("");
const confirmed = shallowRef(false);
const canCreate = computed(
  () =>
    Boolean(projectId.value && prompt.value.trim()) &&
    (mode.value !== "full-access" || confirmed.value),
);
function submit(): void {
  if (canCreate.value)
    emit("create", {
      projectId: projectId.value,
      prompt: prompt.value.trim(),
      mode: mode.value,
      model: model.value || undefined,
      fullAccessConfirmed: confirmed.value,
    });
}
</script>

<template>
  <main class="start-screen">
    <div class="start-screen__content">
      <span class="eyebrow">ЦЕНТР УПРАВЛЕНИЯ CODEX</span>
      <h1>Запусти задачу.<br />Вернись, когда будет готово.</h1>
      <p>
        Каждая сессия работает в отдельной Git-ветке и worktree. Закрытие окна не остановит агента.
      </p>
      <button
        v-if="projects.length === 0"
        class="primary"
        type="button"
        @click="emit('addProject')"
      >
        <FolderPlus :size="18" />Выбрать Git-репозиторий
      </button>
      <form v-else class="task-form" @submit.prevent="submit">
        <label
          >Проект<select v-model="projectId">
            <option disabled value="">Выберите проект</option>
            <option v-for="project in projects" :key="project.id" :value="project.id">
              {{ project.name }}{{ project.dirty ? " · есть локальные изменения" : "" }}
            </option>
          </select></label
        >
        <label
          >Задача<textarea
            v-model="prompt"
            rows="5"
            placeholder="Опиши, что должен сделать Codex…"
          />
        </label>
        <label v-if="models.length"
          >Модель<select v-model="model">
            <option value="">По умолчанию Codex</option>
            <option v-for="item in models" :key="item.id" :value="item.id">
              {{ item.displayName ?? item.id }}
            </option>
          </select></label
        >
        <div class="mode-picker">
          <button
            v-for="item in [
              { id: 'default', label: 'Обычный' },
              { id: 'auto-review', label: 'Автопроверка' },
              { id: 'full-access', label: 'Полный доступ' },
            ]"
            :key="item.id"
            type="button"
            :class="{ active: mode === item.id }"
            @click="mode = item.id as AgentsMode"
          >
            {{ item.label }}
          </button>
        </div>
        <label v-if="mode === 'full-access'" class="warning"
          ><input v-model="confirmed" type="checkbox" />Я понимаю: Codex получит полный доступ к
          системе и сети.</label
        >
        <p v-if="projects.find((project) => project.id === projectId)?.dirty" class="notice">
          Незакоммиченные изменения основного checkout не попадут в новую рабочую копию.
        </p>
        <button class="primary" type="submit" :disabled="!canCreate || loading">
          <Play :size="16" />{{ loading ? "Создаём worktree…" : "Запустить задачу" }}
        </button>
      </form>
    </div>
  </main>
</template>

<script setup lang="ts">
import { onMounted, ref, shallowRef } from "vue";
import { Globe2 } from "lucide-vue-next";
import {
  type Space,
  deriveSpaceId,
  formatSpaceCode,
  getActiveSpace,
  getSpaces,
  removeSpace,
  renameSpace,
} from "@/services/space/space-manager";

const spaces = ref<Space[]>([]);
const activeSpaceCode = shallowRef<string | null>(null);
const renamingCode = shallowRef<string | null>(null);
const renameInput = shallowRef("");
const deletingCode = shallowRef<string | null>(null);

async function loadSpacesState() {
  spaces.value = await getSpaces();
  activeSpaceCode.value = await getActiveSpace();
}

function startRename(space: Space) {
  renamingCode.value = space.code;
  renameInput.value = space.name;
}

async function confirmRename() {
  if (!renamingCode.value) return;

  await renameSpace(renamingCode.value, renameInput.value);
  await loadSpacesState();
  renamingCode.value = null;
}

function cancelRename() {
  renamingCode.value = null;
  renameInput.value = "";
}

async function confirmDeleteSpace() {
  if (!deletingCode.value) return;

  const code = deletingCode.value;
  if (window.electronAPI?.invoke) {
    const spaceId = await deriveSpaceId(code);
    await window.electronAPI.invoke("db:deleteSpace", spaceId).catch(() => {});
  }

  await removeSpace(code);
  await loadSpacesState();
  deletingCode.value = null;
}

onMounted(() => {
  void loadSpacesState();
});
</script>

<template>
  <div class="settings-tab" data-testid="delphi-settings-spaces-tab">
    <header class="settings-tab-header">
      <p class="settings-tab-kicker">Настройки Delphi</p>
      <h1 class="settings-tab-title">Пространства</h1>
      <p class="settings-tab-subtitle">
        Управление пространствами синхронизации и их локальными данными.
      </p>
    </header>

    <section class="settings-card">
      <div class="settings-card-header">
        <div class="settings-card-icon">
          <Globe2 :size="16" />
        </div>
        <div>
          <h2>Список пространств</h2>
          <p>Переименовывайте и очищайте неактивные пространства.</p>
        </div>
      </div>

      <div v-if="spaces.length === 0" class="settings-empty">
        Сохранённых пространств пока нет.
      </div>

      <div v-else class="settings-space-list">
        <article
          v-for="space in spaces"
          :key="space.code"
          :class="[
            'settings-space-card',
            activeSpaceCode === space.code ? 'settings-space-card--active' : '',
          ]"
        >
          <template v-if="deletingCode === space.code">
            <div class="settings-space-delete">
              <div>
                <h3>Удалить пространство?</h3>
                <p>Будут удалены локальные данные этого пространства.</p>
              </div>

              <div class="settings-space-actions">
                <button
                  type="button"
                  class="settings-action-button settings-action-button--danger"
                  @click="confirmDeleteSpace"
                >
                  Да, удалить
                </button>
                <button
                  type="button"
                  class="settings-action-button"
                  @click="deletingCode = null"
                >
                  Отмена
                </button>
              </div>
            </div>
          </template>

          <template v-else-if="renamingCode === space.code">
            <div class="settings-space-rename">
              <input
                v-model="renameInput"
                class="settings-text-input"
                autofocus
                @keyup.enter="confirmRename"
                @keyup.escape="cancelRename"
              />

              <div class="settings-space-actions">
                <button
                  type="button"
                  class="settings-action-button settings-action-button--primary"
                  @click="confirmRename"
                >
                  Сохранить
                </button>
                <button
                  type="button"
                  class="settings-action-button"
                  @click="cancelRename"
                >
                  Отмена
                </button>
              </div>
            </div>
          </template>

          <template v-else>
            <div class="settings-space-main">
              <div>
                <div class="settings-space-title-row">
                  <h3 class="settings-space-title">{{ space.name }}</h3>
                  <span
                    v-if="activeSpaceCode === space.code"
                    class="settings-space-badge"
                  >
                    Активно
                  </span>
                </div>

                <p class="settings-space-code">
                  {{ formatSpaceCode(space.code) }}
                </p>
              </div>

              <div class="settings-space-actions">
                <button
                  type="button"
                  class="settings-action-button"
                  @click="startRename(space)"
                >
                  Переименовать
                </button>
                <button
                  v-if="activeSpaceCode !== space.code"
                  type="button"
                  class="settings-action-button settings-action-button--danger"
                  @click="deletingCode = space.code"
                >
                  Удалить
                </button>
              </div>
            </div>
          </template>
        </article>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-tab {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.settings-tab-header {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.settings-tab-kicker {
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted-foreground);
}

.settings-tab-title {
  font-size: clamp(1.8rem, 2.6vw, 2.4rem);
  line-height: 1;
  font-weight: 700;
  color: var(--foreground);
}

.settings-tab-subtitle {
  max-width: 42rem;
  font-size: 0.95rem;
  color: var(--muted-foreground);
}

.settings-card {
  border: 1px solid var(--border);
  border-radius: 1.25rem;
  background: color-mix(in srgb, var(--background) 86%, var(--secondary));
  overflow: hidden;
}

.settings-card-header {
  display: flex;
  align-items: center;
  gap: 0.9rem;
  padding: 1.2rem 1.25rem 1rem;
  border-bottom: 1px solid color-mix(in srgb, var(--border) 75%, transparent);
}

.settings-card-header h2 {
  font-size: 1rem;
  font-weight: 600;
  color: var(--foreground);
}

.settings-card-header p {
  margin-top: 0.15rem;
  font-size: 0.85rem;
  color: var(--muted-foreground);
}

.settings-card-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 2rem;
  height: 2rem;
  border-radius: 0.75rem;
  background: color-mix(in srgb, var(--secondary) 82%, transparent);
  color: var(--foreground);
  flex-shrink: 0;
}

.settings-empty {
  padding: 1rem 1.25rem 1.25rem;
  color: var(--muted-foreground);
  font-size: 0.9rem;
}

.settings-space-list {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
  padding: 1rem 1.25rem 1.25rem;
}

.settings-space-card {
  border: 1px solid color-mix(in srgb, var(--border) 78%, transparent);
  border-radius: 1rem;
  background: color-mix(in srgb, var(--background) 92%, transparent);
}

.settings-space-card--active {
  border-color: color-mix(in srgb, rgb(16 185 129) 40%, var(--border));
  background: color-mix(in srgb, rgb(16 185 129) 8%, var(--background));
}

.settings-space-main,
.settings-space-rename,
.settings-space-delete {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem;
}

.settings-space-rename,
.settings-space-delete {
  align-items: stretch;
}

.settings-space-title-row {
  display: flex;
  align-items: center;
  gap: 0.55rem;
}

.settings-space-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--foreground);
}

.settings-space-code {
  margin-top: 0.25rem;
  font-family: var(--font-mono, monospace);
  font-size: 0.78rem;
  letter-spacing: 0.12em;
  color: var(--muted-foreground);
}

.settings-space-badge {
  display: inline-flex;
  align-items: center;
  border-radius: 999px;
  padding: 0.2rem 0.5rem;
  background: color-mix(in srgb, rgb(16 185 129) 16%, transparent);
  color: rgb(16 185 129);
  font-size: 0.72rem;
  font-weight: 600;
}

.settings-space-actions {
  display: inline-flex;
  align-items: center;
  gap: 0.55rem;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.settings-action-button {
  padding: 0.6rem 0.85rem;
  border: 1px solid color-mix(in srgb, var(--border) 82%, transparent);
  border-radius: 0.85rem;
  background: color-mix(in srgb, var(--background) 96%, transparent);
  color: var(--foreground);
  font-size: 0.82rem;
  font-weight: 500;
  transition:
    border-color 120ms ease,
    background-color 120ms ease;
}

.settings-action-button:hover {
  border-color: color-mix(in srgb, var(--foreground) 20%, var(--border));
  background: color-mix(in srgb, var(--secondary) 55%, var(--background));
}

.settings-action-button--primary {
  background: color-mix(in srgb, var(--foreground) 92%, transparent);
  border-color: color-mix(in srgb, var(--foreground) 85%, transparent);
  color: var(--background);
}

.settings-action-button--primary:hover {
  background: var(--foreground);
}

.settings-action-button--danger {
  color: rgb(248 113 113);
  border-color: color-mix(in srgb, rgb(248 113 113) 26%, var(--border));
}

.settings-action-button--danger:hover {
  background: color-mix(in srgb, rgb(248 113 113) 10%, transparent);
}

.settings-text-input {
  flex: 1;
  min-width: 0;
  padding: 0.7rem 0.85rem;
  border: 1px solid color-mix(in srgb, var(--border) 82%, transparent);
  border-radius: 0.85rem;
  background: color-mix(in srgb, var(--background) 96%, transparent);
  color: var(--foreground);
  outline: none;
}

.settings-text-input:focus {
  border-color: color-mix(in srgb, var(--foreground) 22%, var(--border));
}

.settings-space-delete h3 {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--foreground);
}

.settings-space-delete p {
  margin-top: 0.25rem;
  font-size: 0.82rem;
  color: var(--muted-foreground);
}

@media (max-width: 720px) {
  .settings-space-main,
  .settings-space-rename,
  .settings-space-delete {
    flex-direction: column;
    align-items: stretch;
  }

  .settings-space-actions {
    justify-content: stretch;
  }

  .settings-action-button {
    width: 100%;
  }
}
</style>

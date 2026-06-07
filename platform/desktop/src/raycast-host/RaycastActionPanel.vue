<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import type { RaycastSnapshotNode } from "../../shared/raycast-ipc";
import {
  actionNodes,
  actionSections,
  actionShortcut,
  actionSubmenuActions,
  matchesActionShortcut,
} from "./model";

const props = defineProps<{
  panel: RaycastSnapshotNode | null;
}>();

const emit = defineEmits<{
  execute: [action: RaycastSnapshotNode];
}>();

const sections = computed(() => actionSections(props.panel));
const actions = computed(() => actionNodes(props.panel));
const openSubmenuId = shallowRef<string | null>(null);

onMounted(() => {
  window.addEventListener("keydown", handleKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleKeydown);
});

function actionTitle(action: RaycastSnapshotNode): string {
  const title = action.props.title;
  if (typeof title === "string" && title.trim()) return title;
  if (action.type === "Action.CopyToClipboard") return "Скопировать";
  if (action.type === "Action.Paste") return "Вставить";
  if (action.type === "Action.Push") return "Открыть";
  if (action.type === "Action.Pop") return "Назад";
  if (action.type === "Action.PopToRoot") return "К началу";
  if (action.type === "Action.OpenInBrowser") return "Открыть в браузере";
  if (action.type === "Action.Open") return "Открыть";
  if (action.type === "Action.ShowInFinder") return "Показать в папке";
  if (action.type === "Action.Trash") return "Удалить";
  if (action.type === "Action.LaunchCommand") return "Запустить";
  if (action.type === "Action.SubmitForm") return "Отправить";
  return "Действие";
}

function actionKey(action: RaycastSnapshotNode, prefix: string): string {
  const title = action.props.title;
  const id = typeof action.props.id === "string" ? action.props.id : null;
  return `${prefix}:${id ?? action.type}:${typeof title === "string" ? title : ""}`;
}

function toggleSubmenu(id: string): void {
  openSubmenuId.value = openSubmenuId.value === id ? null : id;
}

function executeSubmenuAction(action: RaycastSnapshotNode): void {
  openSubmenuId.value = null;
  emit("execute", action);
}

function handleKeydown(event: KeyboardEvent): void {
  if (isTextInputTarget(event.target) && !event.ctrlKey && !event.metaKey && !event.altKey) return;
  const action = actions.value.find((item) => matchesActionShortcut(actionShortcut(item), event));
  if (!action) return;
  event.preventDefault();
  emit("execute", action);
}

function isTextInputTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}
</script>

<template>
  <aside v-if="sections.length > 0" class="raycast-action-panel" aria-label="Действия">
    <section v-for="section in sections" :key="section.id" class="raycast-action-panel__section">
      <h2 v-if="section.title" class="raycast-action-panel__section-title">
        {{ section.title }}
      </h2>
      <template v-for="action in section.actions" :key="actionKey(action, section.id)">
        <button
          v-if="action.type !== 'ActionPanel.Submenu'"
          class="raycast-action-panel__button"
          type="button"
          @click="emit('execute', action)"
        >
          <span class="raycast-action-panel__title">{{ actionTitle(action) }}</span>
          <span v-if="actionShortcut(action)" class="raycast-action-panel__shortcut">
            {{ actionShortcut(action)?.label }}
          </span>
        </button>
        <div v-else class="raycast-action-panel__submenu">
          <button
            class="raycast-action-panel__button"
            type="button"
            :aria-expanded="openSubmenuId === actionKey(action, section.id)"
            @click="toggleSubmenu(actionKey(action, section.id))"
          >
            <span class="raycast-action-panel__title">{{ actionTitle(action) }}</span>
            <span class="raycast-action-panel__chevron" aria-hidden="true">^</span>
          </button>
          <div
            v-if="openSubmenuId === actionKey(action, section.id)"
            class="raycast-action-panel__submenu-menu"
          >
            <button
              v-for="child in actionSubmenuActions(action)"
              :key="actionKey(child, actionKey(action, section.id))"
              class="raycast-action-panel__submenu-item"
              type="button"
              @click="executeSubmenuAction(child)"
            >
              <span class="raycast-action-panel__title">{{ actionTitle(child) }}</span>
              <span v-if="actionShortcut(child)" class="raycast-action-panel__shortcut">
                {{ actionShortcut(child)?.label }}
              </span>
            </button>
          </div>
        </div>
      </template>
    </section>
  </aside>
</template>

<style scoped>
.raycast-action-panel {
  position: relative;
  z-index: 5;
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  overflow: visible;
}

.raycast-action-panel__section {
  display: flex;
  min-width: 0;
  align-items: center;
  gap: 6px;
}

.raycast-action-panel__section-title {
  margin: 0;
  color: var(--muted-foreground);
  font-size: 11px;
  font-weight: 700;
  white-space: nowrap;
}

.raycast-action-panel__button {
  display: inline-flex;
  min-width: 0;
  height: 28px;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
  padding: 0 10px;
  cursor: default;
}

.raycast-action-panel__submenu {
  position: relative;
}

.raycast-action-panel__chevron {
  flex: 0 0 auto;
  color: var(--muted-foreground);
  font-size: 11px;
}

.raycast-action-panel__submenu-menu {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  z-index: 10;
  display: grid;
  min-width: 190px;
  gap: 4px;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--background) 92%, var(--foreground) 8%);
  padding: 6px;
  box-shadow: 0 12px 34px color-mix(in srgb, var(--foreground) 18%, transparent);
}

.raycast-action-panel__submenu-item {
  display: flex;
  min-width: 0;
  height: 28px;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  border: 0;
  border-radius: var(--radius-input);
  background: transparent;
  color: var(--foreground);
  padding: 0 8px;
  cursor: default;
  text-align: left;
}

.raycast-action-panel__submenu-item:hover,
.raycast-action-panel__submenu-item:focus {
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  outline: none;
}

.raycast-action-panel__title {
  display: block;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
}

.raycast-action-panel__shortcut {
  flex: 0 0 auto;
  color: var(--muted-foreground);
  font-size: 11px;
  white-space: nowrap;
}
</style>

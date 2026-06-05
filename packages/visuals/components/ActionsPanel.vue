<script setup lang="ts">
import KbdKey from "./KbdKey.vue";

defineProps<{
  isFavorite: boolean;
  isHidden: boolean;
  canHide: boolean;
}>();

defineEmits<{
  open: [];
  toggleFavorite: [];
  toggleHide: [];
}>();
</script>

<template>
  <div class="actions-panel">
    <ul class="actions-panel__list">
      <li class="actions-panel__item" @click="$emit('open')">
        <span class="actions-panel__label">Открыть</span>
        <span class="actions-panel__keys"><KbdKey>↵</KbdKey></span>
      </li>
      <li class="actions-panel__item" @click="$emit('toggleFavorite')">
        <span class="actions-panel__label">{{
          isFavorite ? "Убрать из избранного" : "Добавить в избранное"
        }}</span>
        <span class="actions-panel__keys">
          <KbdKey>Ctrl</KbdKey><KbdKey>Shift</KbdKey><KbdKey>F</KbdKey>
        </span>
      </li>
      <li v-if="canHide" class="actions-panel__item" @click="$emit('toggleHide')">
        <span class="actions-panel__label">{{
          isHidden ? "Показать в списке" : "Скрыть из списка"
        }}</span>
        <span class="actions-panel__keys"> <KbdKey>Ctrl</KbdKey><KbdKey>H</KbdKey> </span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.actions-panel {
  position: absolute;
  bottom: calc(36px + 8px);
  right: 8px;
  z-index: 20;
  width: 300px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 8px;
  background: var(--surface, #3a3a3e);
  box-shadow: none;
  padding: 4px;
  animation: panel-in 120ms cubic-bezier(0.2, 0, 0, 1);
}

.actions-panel__list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.actions-panel__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 10px;
  border-radius: 5px;
  cursor: default;
}

.actions-panel__item:hover {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.actions-panel__label {
  font-size: 13px;
  color: var(--foreground);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.actions-panel__keys {
  display: flex;
  align-items: center;
  gap: 3px;
  flex-shrink: 0;
}

@keyframes panel-in {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>

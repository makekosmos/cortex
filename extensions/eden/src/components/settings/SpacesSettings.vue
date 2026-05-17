<template>
  <div class="settings-tab" data-testid="spaces-settings-tab">
    <h1 class="settings-tab-title">Пространства</h1>
    <p class="settings-tab-subtitle">Выберите активное пространство для рабочего экрана.</p>

    <div class="settings-sections">
      <section class="settings-section">
        <div class="settings-section-header">
          <h2>Активное пространство</h2>
        </div>
        <div class="settings-section-body">
          <button
            v-for="option in spaceOptions"
            :key="option.id"
            class="settings-row settings-space-row"
            :class="{ active: props.activeSpace === option.id }"
            :data-testid="`settings-space-${option.id}`"
            type="button"
            @click="emit('selectSpace', option.id)"
          >
            <div class="settings-row-left">
              <div class="settings-row-title">{{ option.label }}</div>
              <div class="settings-row-desc settings-row-desc-plain">{{ option.description }}</div>
            </div>
            <div class="settings-row-right">
              <span v-if="props.activeSpace === option.id" class="settings-space-badge">Активно</span>
            </div>
          </button>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { SpaceId } from "@/components/sidebar/types";

const props = defineProps<{
  activeSpace: SpaceId;
}>();

const emit = defineEmits<{
  selectSpace: [spaceId: SpaceId];
}>();

const spaceOptions: Array<{ id: SpaceId; label: string; description: string }> = [
  {
    id: "my-space",
    label: "Моё пространство",
    description: "Открывает главную заметку пространства для фокусного письма.",
  },
  {
    id: "all-objects",
    label: "Все объекты",
    description: "Таблица всех объектов и заметок с их типами.",
  },
  {
    id: "all-notes",
    label: "Все заметки",
    description: "Список всех заметок с сортировкой по ключевым полям.",
  },
  {
    id: "all-properties",
    label: "Все свойства",
    description: "Обзор типов объектов и их свойств.",
  },
  {
    id: "diary",
    label: "Дневник",
    description: "Экран дневника и истории тренировок.",
  },
];
</script>

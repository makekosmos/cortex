<template>
  <aside class="spaces-rail" data-testid="spaces-rail">
    <div class="spaces-rail-list">
      <button
        v-for="space in spaces"
        :key="space.id"
        class="space-rail-item"
        :class="{ 'is-active': activeSpace === space.id }"
        :data-testid="`space-tab-${space.id}`"
        :title="space.label"
        type="button"
        @click="emit('selectSpace', space.id)"
      >
        <span aria-hidden="true" class="space-rail-badge">{{ space.icon }}</span>
      </button>
    </div>
  </aside>
</template>

<script setup vapor lang="ts">
import type { SpaceId } from "./types";

const spaces: Array<{ id: SpaceId; label: string; icon: string }> = [
  { id: "my-space", label: "Мое пространство", icon: "🏡" },
  { id: "all-objects", label: "Все объекты", icon: "📚" },
  { id: "all-notes", label: "Все заметки", icon: "📝" },
];

defineProps<{
  activeSpace: SpaceId;
}>();

const emit = defineEmits<{
  selectSpace: [spaceId: SpaceId];
}>();
</script>

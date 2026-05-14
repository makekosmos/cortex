<script setup lang="ts">
interface Props {
  /** Опасное действие — рендерится в --destructive тон. */
  destructive?: boolean;
  disabled?: boolean;
}

withDefaults(defineProps<Props>(), {
  destructive: false,
  disabled: false,
});

defineEmits<{
  click: [event: MouseEvent];
}>();
</script>

<template>
  <button
    type="button"
    role="menuitem"
    class="kosmos-context-menu-item"
    :class="{ 'kosmos-context-menu-item--destructive': destructive }"
    :disabled="disabled"
    @click="(e) => $emit('click', e)"
  >
    <slot />
  </button>
</template>

<style scoped>
.kosmos-context-menu-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.4rem 0.625rem;
  background: transparent;
  border: none;
  border-radius: calc(var(--radius) * 0.6);
  cursor: pointer;
  text-align: left;
  color: var(--foreground);
  font-size: 0.8125rem;
  font-family: inherit;
  transition: background-color 90ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-context-menu-item:hover:not(:disabled),
.kosmos-context-menu-item:focus-visible:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.kosmos-context-menu-item:disabled {
  color: color-mix(in srgb, var(--foreground) 40%, transparent);
  cursor: not-allowed;
}

.kosmos-context-menu-item--destructive {
  color: var(--destructive);
}

.kosmos-context-menu-item--destructive:hover:not(:disabled) {
  background: color-mix(in srgb, var(--destructive) 14%, transparent);
}

.kosmos-context-menu-item :deep(svg) {
  flex-shrink: 0;
}
</style>

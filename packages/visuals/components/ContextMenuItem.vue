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
    class="flex min-h-7 w-full items-center gap-2 rounded-md border-0 bg-transparent px-2 py-1.5 text-left font-[inherit] text-[0.8125rem] text-[var(--foreground)] transition-colors duration-[90ms] ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] focus-visible:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] disabled:cursor-not-allowed disabled:text-[color-mix(in_srgb,var(--foreground)_40%,transparent)] [&_svg]:shrink-0"
    :class="{
      'text-[var(--destructive)] hover:bg-[color-mix(in_srgb,var(--destructive)_14%,transparent)]':
        destructive,
    }"
    :disabled="disabled"
    @click="(e) => $emit('click', e)"
  >
    <slot />
  </button>
</template>

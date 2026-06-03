<script setup lang="ts">
interface Props {
  title?: string;
}

defineProps<Props>();
</script>

<template>
  <aside
    class="box-border flex h-full w-[228px] min-w-[228px] flex-col gap-4 border-r border-[var(--border-color-strong)] bg-[var(--main-background-color)] text-white"
  >
    <div
      v-if="title"
      class="px-3 pt-3 pb-0 font-[var(--font-sans)] text-[13px] leading-[1.4] font-medium [-webkit-app-region:drag]"
    >
      {{ title }}
    </div>
    <div class="flex min-h-0 flex-1 flex-col gap-6 [-webkit-app-region:no-drag]">
      <slot />
    </div>
  </aside>
</template>

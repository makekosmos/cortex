<script setup lang="ts">
import { computed, useSlots } from "vue";

interface Props {
  tone?: "default" | "strong";
  title?: string;
}

const props = withDefaults(defineProps<Props>(), {
  tone: "default",
  title: undefined,
});

const slots = useSlots();
const hasTitleBar = computed(
  () => Boolean(props.title) || Boolean(slots["title-leading"]) || Boolean(slots["title-trailing"]),
);

const sidebarStyle = computed(() => ({
  "--kosmos-settings-sidebar-bg": "var(--bg-app, #1d1d1f)",
}));
</script>

<template>
  <aside
    class="kosmos-settings-sidebar box-border flex h-full w-[228px] min-w-[228px] flex-col gap-4 border-r border-[var(--border-color-strong)] bg-[var(--kosmos-settings-sidebar-bg)] text-white"
    :style="sidebarStyle"
  >
    <div
      v-if="hasTitleBar"
      class="kosmos-settings-sidebar__title flex min-h-9 items-center gap-2 px-2 pb-0 pt-2 font-[var(--font-sans)] text-[13px] leading-[1.4] font-medium [-webkit-app-region:drag]"
    >
      <div
        v-if="$slots['title-leading']"
        class="inline-flex shrink-0 items-center [-webkit-app-region:no-drag]"
      >
        <slot name="title-leading" />
      </div>
      <span v-if="title" class="min-w-0 truncate px-1">{{ title }}</span>
      <div
        v-if="$slots['title-trailing']"
        class="ml-auto inline-flex shrink-0 items-center [-webkit-app-region:no-drag]"
      >
        <slot name="title-trailing" />
      </div>
    </div>
    <div
      class="kosmos-settings-sidebar__content flex min-h-0 flex-1 flex-col gap-6 [-webkit-app-region:no-drag]"
    >
      <slot />
    </div>
  </aside>
</template>

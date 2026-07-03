<script setup lang="ts">
import { computed, useSlots } from "vue";

interface Props {
  tone?: "default" | "strong";
  title?: string;
  background?: string;
}

const props = withDefaults(defineProps<Props>(), {
  tone: "default",
  title: undefined,
  background: undefined,
});

const slots = useSlots();
const hasTitleBar = computed(
  () => Boolean(props.title) || Boolean(slots["title-leading"]) || Boolean(slots["title-trailing"]),
);

const sidebarStyle = computed(() => ({
  "--kosmos-settings-sidebar-bg": props.background ?? "var(--sidebar-bg, var(--bg-app, #1d1d1f))",
}));
</script>

<template>
  <aside
    class="kosmos-settings-sidebar box-border flex h-full w-[228px] min-w-[228px] flex-col gap-4 border-r border-[var(--border-color-strong)] bg-[var(--kosmos-settings-sidebar-bg)] text-white"
    :style="sidebarStyle"
  >
    <div
      v-if="hasTitleBar"
      class="kosmos-settings-sidebar__title flex items-center font-[var(--font-sans)] text-[length:var(--kosmos-text-caption-size)] leading-[1.4] font-medium [-webkit-app-region:drag]"
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

<style scoped>
.kosmos-settings-sidebar__title {
  box-sizing: border-box;
  height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-settings-titlebar-height))
  );
  min-height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-settings-titlebar-height))
  );
  gap: var(--kosmos-titlebar-control-gap);
  border-bottom: 1px solid var(--border-color-strong);
  padding: env(titlebar-area-y, 0px) 10px 0;
}

.kosmos-settings-sidebar__title :deep([data-testid="sidebar-header"]) {
  height: 100%;
  align-items: center;
  gap: var(--kosmos-titlebar-control-gap);
}
</style>

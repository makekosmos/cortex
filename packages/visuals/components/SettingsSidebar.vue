<script setup lang="ts">
import { computed } from "vue";

interface Props {
  tone?: "default" | "strong";
  title?: string;
}

const props = withDefaults(defineProps<Props>(), {
  tone: "default",
  title: undefined,
});

const sidebarStyle = computed(() => ({
  "--kosmos-settings-sidebar-bg":
    props.tone === "strong"
      ? "color-mix(in srgb, var(--main-background-color) 64%, var(--color-bg-primary) 36%)"
      : "var(--main-background-color)",
}));
</script>

<template>
  <aside
    class="kosmos-settings-sidebar box-border flex h-full w-[228px] min-w-[228px] flex-col gap-4 border-r border-[var(--border-color-strong)] bg-[var(--kosmos-settings-sidebar-bg)] text-white"
    :style="sidebarStyle"
  >
    <div
      v-if="title"
      class="kosmos-settings-sidebar__title px-3 pb-0 pt-[calc(0.75rem+var(--kosmos-mac-traffic-light-top-safe-area,0px))] font-[var(--font-sans)] text-[13px] leading-[1.4] font-medium [-webkit-app-region:drag]"
    >
      {{ title }}
    </div>
    <div
      class="kosmos-settings-sidebar__content flex min-h-0 flex-1 flex-col gap-6 [-webkit-app-region:no-drag]"
    >
      <slot />
    </div>
  </aside>
</template>

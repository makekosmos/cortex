<script setup lang="ts">
import { computed, provide, useSlots } from "vue";
import Titlebar from "./Titlebar.vue";
import type { TitlebarPlatform } from "./types";

interface Props {
  platform?: TitlebarPlatform;
  title?: string;
}

const props = withDefaults(defineProps<Props>(), {
  platform: "windows",
  title: undefined,
});

// Провайдим наличие сайдбара вниз по дереву, чтобы DesktopContentSurface
// мог автоматически убрать скругление верхнего-левого угла + левую границу,
// когда сайдбар не используется.
const slots = useSlots();
const hasSidebar = computed(() => Boolean(slots.sidebar));
provide("kosmosHasSidebar", hasSidebar);
</script>

<template>
  <div class="flex h-full min-h-0 w-full flex-col overflow-hidden bg-(--sidebar-bg)">
    <Titlebar :platform="props.platform" :title="props.title">
      <template #leading>
        <slot name="titlebar-leading" />
      </template>
      <template #center>
        <slot name="titlebar-center" />
      </template>
      <template #trailing>
        <slot name="titlebar-trailing" />
      </template>
    </Titlebar>

    <div class="flex min-h-0 min-w-0 flex-1 overflow-hidden">
      <aside v-if="hasSidebar" class="relative z-10 flex h-full min-h-0 shrink-0 overflow-visible">
        <slot name="sidebar" />
      </aside>

      <div class="relative z-[1] flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        <slot />
      </div>
    </div>
  </div>
</template>

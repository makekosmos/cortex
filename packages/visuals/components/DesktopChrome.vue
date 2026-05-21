<script setup lang="ts">
import { computed, provide, useSlots } from "vue";
import Titlebar, { type TitlebarPlatform } from "./Titlebar.vue";

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
  <div class="kosmos-desktop-chrome">
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

    <div class="kosmos-desktop-chrome__body">
      <aside v-if="hasSidebar" class="kosmos-desktop-chrome__sidebar">
        <slot name="sidebar" />
      </aside>

      <div class="kosmos-desktop-chrome__content">
        <slot />
      </div>
    </div>
  </div>
</template>

<style scoped>
.kosmos-desktop-chrome {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--sidebar-bg);
}

.kosmos-desktop-chrome__body {
  display: flex;
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
}

.kosmos-desktop-chrome__sidebar {
  position: relative;
  display: flex;
  flex-shrink: 0;
  min-height: 0;
  height: 100%;
  overflow: visible;
  z-index: 10;
}

.kosmos-desktop-chrome__content {
  position: relative;
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  z-index: 1;
}
</style>

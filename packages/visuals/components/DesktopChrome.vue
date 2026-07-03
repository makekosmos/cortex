<script setup lang="ts">
import { computed, provide, useSlots } from "vue";
import Titlebar from "./Titlebar.vue";
import type { TitlebarPlatform } from "./types";

interface Props {
  appearance?: "default" | "settings";
  platform?: TitlebarPlatform;
  title?: string;
  titlebarTransparent?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  appearance: "default",
  platform: "windows",
  title: undefined,
  titlebarTransparent: false,
});

// Провайдим наличие сайдбара вниз по дереву, чтобы DesktopContentSurface
// мог автоматически убрать скругление верхнего-левого угла + левую границу,
// когда сайдбар не используется.
const slots = useSlots();
const hasSidebar = computed(() => Boolean(slots.sidebar));
provide("kosmosHasSidebar", hasSidebar);
</script>

<template>
  <div
    v-if="props.appearance === 'settings'"
    class="kosmos-desktop-chrome-settings flex h-full min-h-0 w-full overflow-hidden bg-transparent"
  >
    <aside v-if="hasSidebar" class="relative z-10 flex h-full min-h-0 shrink-0 overflow-visible">
      <slot name="sidebar" />
    </aside>

    <div
      class="relative z-[1] flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-[var(--bg-app,var(--background))]"
    >
      <header
        :class="[
          'kosmos-desktop-chrome-settings__header flex items-center justify-between gap-4 [-webkit-app-region:drag]',
          // Native window controls: на macOS traffic lights слева → отступ
          // слева под них; на Windows min/max/close справа → отступ справа.
          props.platform === 'mac'
            ? 'pr-[10px] pl-[calc(10px+var(--kosmos-mac-traffic-light-left-safe-area,0px))]'
            : 'pl-[max(10px,calc(env(titlebar-area-x,0px)+10px))] pr-[max(16px,calc(100vw-env(titlebar-area-x,0px)-env(titlebar-area-width,100vw)+16px))]',
        ]"
      >
        <div
          class="kosmos-desktop-chrome-settings__header-left inline-flex min-w-0 items-center gap-2"
        >
          <slot name="titlebar-leading" />
        </div>

        <div
          class="kosmos-desktop-chrome-settings__header-center inline-flex min-w-0 flex-1 items-center justify-center gap-2"
        >
          <slot name="titlebar-center" />
        </div>

        <div
          class="kosmos-desktop-chrome-settings__header-right inline-flex min-w-0 items-center justify-end gap-2"
        >
          <slot name="titlebar-trailing" />
        </div>
      </header>

      <div
        class="kosmos-desktop-chrome-settings__body flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden bg-transparent"
      >
        <slot />
      </div>
    </div>
  </div>

  <div v-else class="flex h-full min-h-0 w-full flex-col overflow-hidden bg-(--sidebar-bg)">
    <Titlebar
      :platform="props.platform"
      :title="props.title"
      :transparent="props.titlebarTransparent"
    >
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

<style scoped>
.kosmos-desktop-chrome-settings__header {
  position: relative;
  box-sizing: border-box;
  height: var(--kosmos-settings-titlebar-height);
  min-height: var(--kosmos-settings-titlebar-height);
  border-bottom: 1px solid var(--border-color-strong);
  padding-top: 0;
  padding-bottom: 0;
}

.kosmos-desktop-chrome-settings__header-left,
.kosmos-desktop-chrome-settings__header-center,
.kosmos-desktop-chrome-settings__header-right {
  height: 100%;
  align-items: center;
  gap: var(--kosmos-titlebar-control-gap);
}

.kosmos-desktop-chrome-settings__header-left :deep(> div),
.kosmos-desktop-chrome-settings__header-right :deep(> div),
.kosmos-desktop-chrome-settings__header :deep(.kosmos-titlebar-history-controls) {
  height: 100%;
  align-items: center;
  gap: var(--kosmos-titlebar-control-gap);
}

.kosmos-desktop-chrome-settings__header :deep(button),
.kosmos-desktop-chrome-settings__header :deep(a),
.kosmos-desktop-chrome-settings__header :deep(input),
.kosmos-desktop-chrome-settings__header :deep(select),
.kosmos-desktop-chrome-settings__header :deep(textarea),
.kosmos-desktop-chrome-settings__header :deep([role="button"]) {
  -webkit-app-region: no-drag;
}
</style>

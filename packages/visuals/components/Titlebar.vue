<script setup lang="ts">
import { computed } from "vue";
import type { TitlebarPlatform } from "./types";

interface Props {
  platform?: TitlebarPlatform;
  title?: string;
  transparent?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  platform: "windows",
  title: undefined,
  transparent: false,
});

const titlebarClasses = computed(() => [
  "kosmos-titlebar relative z-[10000] grid box-border select-none grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-4 px-(--kosmos-titlebar-inline-padding) py-(--kosmos-titlebar-vertical-padding) text-(--sidebar-foreground)",
  `kosmos-titlebar--${props.platform}`,
  props.transparent ? "bg-transparent" : "bg-(--sidebar-bg)",
]);
</script>

<template>
  <header :class="titlebarClasses">
    <div class="flex min-h-full min-w-0 items-center justify-start gap-2">
      <slot name="leading" />
    </div>

    <div class="flex min-h-full min-w-0 items-center justify-center gap-2">
      <slot name="center">
        <span
          v-if="title"
          class="whitespace-nowrap text-[0.8125rem] font-semibold tracking-[0.02em] text-[color-mix(in_srgb,var(--sidebar-foreground)_82%,transparent)]"
          >{{ title }}</span
        >
      </slot>
    </div>

    <div class="flex min-h-full min-w-0 items-center justify-end gap-2">
      <slot name="trailing" />
    </div>
  </header>
</template>

<style scoped>
.kosmos-titlebar {
  --kosmos-titlebar-height: 40px;
  --kosmos-titlebar-control-size: 32px;
  --kosmos-titlebar-control-radius: 8px;
  --kosmos-titlebar-inline-padding: 16px;
  --kosmos-titlebar-vertical-padding: 0px;
  /* Тайтлбар всегда поверх Modal backdrop/panel (Modal = 9000) и любых
       app-overlay'ов: пользователь должен видеть/нажимать наши leading
       (sidebar toggle, app name) и trailing (status, settings) даже при
       открытом модальном окне. Native window controls (titleBarOverlay)
       и так выше любого CSS-слоя. */
  height: calc(var(--kosmos-titlebar-height) + (var(--kosmos-titlebar-vertical-padding) * 2));
  min-height: calc(var(--kosmos-titlebar-height) + (var(--kosmos-titlebar-vertical-padding) * 2));
  -webkit-app-region: drag;
}

.kosmos-titlebar--mac {
  padding-left: var(--kosmos-mac-traffic-light-left-safe-area, 96px);
}

.kosmos-titlebar--windows {
  height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-titlebar-height))
  );
  min-height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-titlebar-height))
  );
  padding-top: env(titlebar-area-y, 0px);
  padding-bottom: 0;
  padding-left: max(
    var(--kosmos-titlebar-inline-padding),
    calc(env(titlebar-area-x, 0px) + var(--kosmos-titlebar-inline-padding))
  );
  padding-right: max(
    var(--kosmos-titlebar-inline-padding),
    calc(
      100vw - env(titlebar-area-x, 0px) - env(titlebar-area-width, 100vw) +
        var(--kosmos-titlebar-inline-padding)
    )
  );
}

.kosmos-titlebar :deep(button),
.kosmos-titlebar :deep(a),
.kosmos-titlebar :deep(input),
.kosmos-titlebar :deep(select),
.kosmos-titlebar :deep(textarea),
.kosmos-titlebar :deep([role="button"]) {
  -webkit-app-region: no-drag;
}
</style>

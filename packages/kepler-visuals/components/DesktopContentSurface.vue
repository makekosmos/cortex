<script setup lang="ts">
import { computed } from "vue";

interface Props {
  paddingTop?: string;
  paddingInline?: string;
  paddingBottom?: string;
  radiusTopLeft?: string;
  radiusBottomLeft?: string;
  showLeftBorder?: boolean;
  scrollable?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  paddingTop: "1rem",
  paddingInline: "1rem",
  paddingBottom: "0",
  radiusTopLeft: "16px",
  radiusBottomLeft: "0",
  showLeftBorder: true,
  scrollable: false,
});

const surfaceStyle = computed(() => ({
  "--kepler-content-padding-top": props.paddingTop,
  "--kepler-content-padding-inline": props.paddingInline,
  "--kepler-content-padding-bottom": props.paddingBottom,
  "--kepler-content-radius-top-left": props.radiusTopLeft,
  "--kepler-content-radius-bottom-left": props.radiusBottomLeft,
  "--kepler-content-border-color": "var(--dashboard-border-subtle, var(--border))",
  "--kepler-content-border-left-color": props.showLeftBorder
    ? "var(--kepler-content-border-color)"
    : "transparent",
}));
</script>

<template>
  <section
    class="kepler-desktop-content-surface"
    :class="{ 'kepler-desktop-content-surface--scrollable': props.scrollable }"
    :style="surfaceStyle"
  >
    <slot />
  </section>
</template>

<style scoped>
.kepler-desktop-content-surface {
  display: flex;
  flex: 1;
  min-width: 0;
  min-height: 0;
  flex-direction: column;
  padding-top: var(--kepler-content-padding-top);
  padding-right: var(--kepler-content-padding-inline);
  padding-bottom: var(--kepler-content-padding-bottom);
  padding-left: var(--kepler-content-padding-inline);
  border-top: 1px solid var(--kepler-content-border-color);
  border-left: 1px solid var(--kepler-content-border-left-color);
  border-top-left-radius: var(--kepler-content-radius-top-left);
  border-bottom-left-radius: var(--kepler-content-radius-bottom-left);
  background: var(--background);
  overflow: hidden;
  transition:
    border-left-color 280ms cubic-bezier(0.2, 0, 0, 1),
    border-top-left-radius 280ms cubic-bezier(0.2, 0, 0, 1),
    border-bottom-left-radius 280ms cubic-bezier(0.2, 0, 0, 1);
}

.kepler-desktop-content-surface--scrollable {
  overflow-x: hidden;
  overflow-y: auto;
}
</style>

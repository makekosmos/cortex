<script setup lang="ts">
import { computed, inject, ref, type Ref } from "vue";

interface Props {
  paddingTop?: string;
  paddingInline?: string;
  paddingBottom?: string;
  /**
   * Скругление верхнего-левого угла content surface.
   *
   * Если не передано — берётся из контекста `DesktopChrome` через provide/inject:
   * с сайдбаром → `16px`, без сайдбара → `0` (плоский край, без «лестницы»).
   * Передавай явно, чтобы переопределить.
   */
  radiusTopLeft?: string;
  radiusBottomLeft?: string;
  /**
   * Левая граница content surface.
   *
   * Если не передано — берётся из контекста: с сайдбаром → `true`, без → `false`.
   */
  showLeftBorder?: boolean;
  scrollable?: boolean;
}

const props = defineProps<Props>();

// Контекст из DesktopChrome: есть ли сайдбар. По умолчанию (без обёртки) считаем
// что сайдбар есть — это сохраняет старое поведение для standalone-использования.
const hasSidebar = inject<Ref<boolean>>("keplerHasSidebar", ref(true));

const effectiveRadiusTopLeft = computed(() => {
  if (props.radiusTopLeft !== undefined) return props.radiusTopLeft;
  return hasSidebar.value ? "16px" : "0";
});
const effectiveRadiusBottomLeft = computed(() => props.radiusBottomLeft ?? "0");
const effectiveShowLeftBorder = computed(() => {
  if (props.showLeftBorder !== undefined) return props.showLeftBorder;
  return hasSidebar.value;
});
const effectivePaddingTop = computed(() => props.paddingTop ?? "1rem");
const effectivePaddingInline = computed(() => props.paddingInline ?? "1rem");
const effectivePaddingBottom = computed(() => props.paddingBottom ?? "0");

const surfaceStyle = computed(() => ({
  "--kepler-content-padding-top": effectivePaddingTop.value,
  "--kepler-content-padding-inline": effectivePaddingInline.value,
  "--kepler-content-padding-bottom": effectivePaddingBottom.value,
  "--kepler-content-radius-top-left": effectiveRadiusTopLeft.value,
  "--kepler-content-radius-bottom-left": effectiveRadiusBottomLeft.value,
  "--kepler-content-border-color": "var(--dashboard-border-subtle, var(--border))",
  "--kepler-content-border-left-color": effectiveShowLeftBorder.value
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

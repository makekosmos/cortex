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
  showTopBorder?: boolean;
  scrollable?: boolean;
}

const props = defineProps<Props>();

// Контекст из DesktopChrome: есть ли сайдбар. По умолчанию (без обёртки) считаем
// что сайдбар есть — это сохраняет старое поведение для standalone-использования.
const hasSidebar = inject<Ref<boolean>>("kosmosHasSidebar", ref(true));

const effectiveRadiusTopLeft = computed(() => {
  if (props.radiusTopLeft !== undefined) return props.radiusTopLeft;
  return hasSidebar.value ? "16px" : "0";
});
const effectiveRadiusBottomLeft = computed(() => props.radiusBottomLeft ?? "0");
const effectiveShowLeftBorder = computed(() => {
  if (props.showLeftBorder !== undefined) return props.showLeftBorder;
  return hasSidebar.value;
});
const effectiveShowTopBorder = computed(() => props.showTopBorder ?? true);
const effectivePaddingTop = computed(() => props.paddingTop ?? "1rem");
const effectivePaddingInline = computed(() => props.paddingInline ?? "1rem");
const effectivePaddingBottom = computed(() => props.paddingBottom ?? "0");

const surfaceStyle = computed(() => ({
  "--kosmos-content-padding-top": effectivePaddingTop.value,
  "--kosmos-content-padding-inline": effectivePaddingInline.value,
  "--kosmos-content-padding-bottom": effectivePaddingBottom.value,
  "--kosmos-content-radius-top-left": effectiveRadiusTopLeft.value,
  "--kosmos-content-radius-bottom-left": effectiveRadiusBottomLeft.value,
  "--kosmos-content-border-color": "var(--dashboard-border-subtle, var(--border))",
  "--kosmos-content-border-left-color": effectiveShowLeftBorder.value
    ? "var(--kosmos-content-border-color)"
    : "transparent",
  "--kosmos-content-border-top-color": effectiveShowTopBorder.value
    ? "var(--kosmos-content-border-color)"
    : "transparent",
}));
</script>

<template>
  <section
    class="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden border-t border-l border-t-[var(--kosmos-content-border-top-color)] border-l-[var(--kosmos-content-border-left-color)] bg-(--background) px-[var(--kosmos-content-padding-inline)] pt-[var(--kosmos-content-padding-top)] pb-[var(--kosmos-content-padding-bottom)] rounded-tl-[var(--kosmos-content-radius-top-left)] rounded-bl-[var(--kosmos-content-radius-bottom-left)] transition-[border-left-color,border-top-color,border-top-left-radius,border-bottom-left-radius] duration-[280ms] ease-[cubic-bezier(0.2,0,0,1)]"
    :class="{ 'overflow-x-hidden overflow-y-auto': props.scrollable }"
    :style="surfaceStyle"
  >
    <slot />
  </section>
</template>

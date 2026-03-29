<script setup lang="ts">
import { shallowRef, watch, onMounted, onUnmounted, computed } from "vue";
import "./sidebar.css";

export interface SidebarConfig {
  width: number;
  collapsed: boolean;
}

interface Props {
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  collapseThreshold?: number;
  toggleShortcut?: string;
  className?: string;
  initialConfig?: Partial<SidebarConfig>;
  dragRegion?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  defaultWidth: 200,
  minWidth: 160,
  maxWidth: 320,
  collapseThreshold: 60,
  toggleShortcut: undefined,
  className: undefined,
  initialConfig: undefined,
  dragRegion: false,
});

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
}>();

const width = shallowRef(props.initialConfig?.width ?? props.defaultWidth);
const collapsed = shallowRef(props.initialConfig?.collapsed ?? false);
const isResizing = shallowRef(false);
const animating = shallowRef(false);

const resizeRaf = shallowRef<number | null>(null);
const animTimer = shallowRef<number | null>(null);
const isAnimatingRef = shallowRef(false);

// Keep a config ref in sync for use inside rAF callbacks
const configRef = shallowRef<SidebarConfig>({
  width: width.value,
  collapsed: collapsed.value,
});

watch([width, collapsed], () => {
  configRef.value = { width: width.value, collapsed: collapsed.value };
});

function notifyConfigChange(config: SidebarConfig) {
  emit("configChange", config);
}

function startAnimation() {
  if (animTimer.value) {
    window.clearTimeout(animTimer.value);
  }
  isAnimatingRef.value = true;
  animating.value = true;
  animTimer.value = window.setTimeout(() => {
    isAnimatingRef.value = false;
    animating.value = false;
    animTimer.value = null;
  }, 220);
}

function toggle() {
  startAnimation();
  const next = !collapsed.value;
  collapsed.value = next;
  notifyConfigChange({ width: width.value, collapsed: next });
}

// Keyboard shortcut
function handleKeydown(e: KeyboardEvent) {
  if (!props.toggleShortcut) return;

  const parts = props.toggleShortcut.toLowerCase().split("+");
  const key = parts[parts.length - 1];
  const needsMeta = parts.includes("meta");
  const needsCtrl = parts.includes("ctrl");
  const needsShift = parts.includes("shift");
  const needsAlt = parts.includes("alt");

  if (needsMeta && !e.metaKey) return;
  if (needsCtrl && !e.ctrlKey) return;
  if (needsShift && !e.shiftKey) return;
  if (needsAlt && !e.altKey) return;
  if (e.key !== key && e.key.toLowerCase() !== key) return;

  e.preventDefault();
  toggle();
}

// Resize handlers
function handleResizeStart(e: MouseEvent) {
  e.preventDefault();
  isResizing.value = true;
  document.body.classList.add("sidebar-resizing");
}

function handleResizeMove(e: MouseEvent) {
  if (isAnimatingRef.value) return;

  if (resizeRaf.value) {
    cancelAnimationFrame(resizeRaf.value);
  }

  resizeRaf.value = requestAnimationFrame(() => {
    if (isAnimatingRef.value) return;

    const newWidth = e.clientX;

    if (newWidth <= props.collapseThreshold) {
      if (!configRef.value.collapsed) {
        startAnimation();
        collapsed.value = true;
        configRef.value = { ...configRef.value, collapsed: true };
      }
      return;
    }

    if (configRef.value.collapsed) {
      startAnimation();
      collapsed.value = false;
      width.value = props.minWidth;
      configRef.value = { width: props.minWidth, collapsed: false };
      return;
    }

    const clamped = Math.max(
      props.minWidth,
      Math.min(props.maxWidth, newWidth),
    );
    width.value = clamped;
    configRef.value = { ...configRef.value, width: clamped };
  });
}

function handleResizeEnd() {
  if (resizeRaf.value) {
    cancelAnimationFrame(resizeRaf.value);
    resizeRaf.value = null;
  }
  isResizing.value = false;
  document.body.classList.remove("sidebar-resizing");
  notifyConfigChange(configRef.value);
}

// Attach/detach global mouse listeners during resize
watch(isResizing, (resizing) => {
  if (resizing) {
    window.addEventListener("mousemove", handleResizeMove);
    window.addEventListener("mouseup", handleResizeEnd);
  } else {
    window.removeEventListener("mousemove", handleResizeMove);
    window.removeEventListener("mouseup", handleResizeEnd);
  }
});

onMounted(() => {
  window.addEventListener("keydown", handleKeydown);
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeydown);
  window.removeEventListener("mousemove", handleResizeMove);
  window.removeEventListener("mouseup", handleResizeEnd);
  if (animTimer.value) window.clearTimeout(animTimer.value);
  if (resizeRaf.value) cancelAnimationFrame(resizeRaf.value);
  document.body.classList.remove("sidebar-resizing");
});

const wrapperClasses = computed(() =>
  [
    "kosmos-sidebar-wrapper",
    collapsed.value ? "collapsed" : "",
    animating.value ? "animating" : "",
    isResizing.value ? "is-resizing" : "",
    props.className ?? "",
  ]
    .filter(Boolean)
    .join(" "),
);
</script>

<template>
  <!-- Collapsed with collapsed slot -->
  <template v-if="collapsed && $slots.collapsed">
    <slot name="collapsed" :toggle="toggle" />
  </template>

  <!-- Collapsed without collapsed slot -->
  <div
    v-else-if="collapsed"
    :class="wrapperClasses"
    :style="{ width: '0px' }"
  />

  <!-- Expanded -->
  <div v-else :class="wrapperClasses" :style="{ width: `${width}px` }">
    <div
      v-if="dragRegion"
      :style="{ WebkitAppRegion: 'drag' } as any"
      class="kosmos-sidebar-drag-region"
    />
    <div
      class="kosmos-sidebar-content"
      :style="{ width: `${width}px`, minWidth: `${width}px` }"
    >
      <slot :toggle="toggle" />
    </div>
    <div class="kosmos-sidebar-resize-handle" @mousedown="handleResizeStart">
      <div class="kosmos-resize-handle-line" />
    </div>
  </div>
</template>

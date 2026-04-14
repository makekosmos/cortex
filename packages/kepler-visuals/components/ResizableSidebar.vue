<script setup lang="ts">
import { shallowRef, watch, onMounted, onUnmounted, computed } from "vue";
// eslint-disable-next-line import/no-unassigned-import
import "./sidebar.css";

export interface SidebarConfig {
  width: number;
  hidden: boolean;
}

interface Props {
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  hiddenWidth?: number;
  toggleShortcut?: string;
  className?: string;
  initialConfig?: Partial<SidebarConfig>;
  dragRegion?: boolean;
  offsetX?: number;
  hidden?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  defaultWidth: 200,
  minWidth: 160,
  maxWidth: 320,
  hiddenWidth: 80,
  toggleShortcut: undefined,
  className: undefined,
  initialConfig: undefined,
  dragRegion: false,
  offsetX: 0,
  hidden: undefined,
});

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
  "update:hidden": [hidden: boolean];
}>();

const width = shallowRef(props.initialConfig?.width ?? props.defaultWidth);
const _hidden = shallowRef(
  props.hidden !== undefined
    ? props.hidden
    : (props.initialConfig?.hidden ?? false),
);
const isResizing = shallowRef(false);
const animating = shallowRef(false);
const lineExpanded = shallowRef(false);

let mouseDownX = 0;
let mouseDownY = 0;

const resizeRaf = shallowRef<number | null>(null);
const animTimer = shallowRef<number | null>(null);
const isAnimatingRef = shallowRef(false);

const configRef = shallowRef<SidebarConfig>({
  width: width.value,
  hidden: _hidden.value,
});

watch([width, _hidden], () => {
  configRef.value = { width: width.value, hidden: _hidden.value };
});

watch(
  () => props.hidden,
  (val) => {
    if (val !== undefined && val !== _hidden.value) {
      startAnimation();
      _hidden.value = val;
    }
  },
);

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
  _hidden.value = !_hidden.value;
  emit("update:hidden", _hidden.value);
  notifyConfigChange({ width: width.value, hidden: _hidden.value });
}

function matchesShortcut(e: KeyboardEvent, shortcut: string): boolean {
  const parts = shortcut.toLowerCase().split("+");
  const key = parts[parts.length - 1];
  const needsMeta = parts.includes("meta");
  const needsCtrl = parts.includes("ctrl");
  const needsShift = parts.includes("shift");
  const needsAlt = parts.includes("alt");

  if (needsMeta && !e.metaKey) return false;
  if (needsCtrl && !e.ctrlKey) return false;
  if (needsShift && !e.shiftKey) return false;
  if (needsAlt && !e.altKey) return false;
  const codeKey = e.code.startsWith("Key") ? e.code.slice(3).toLowerCase() : null;
  if (e.key !== key && e.key.toLowerCase() !== key && codeKey !== key) return false;
  return true;
}

function handleKeydown(e: KeyboardEvent) {
  if (!props.toggleShortcut) return;

  const shortcuts = props.toggleShortcut.split("|");
  if (!shortcuts.some((s) => matchesShortcut(e, s))) return;

  e.preventDefault();
  toggle();
}

function handleResizeStart(e: MouseEvent) {
  e.preventDefault();
  mouseDownX = e.clientX;
  mouseDownY = e.clientY;
  isResizing.value = true;
  document.body.classList.add("sidebar-resizing");
}

function handleResizeMove(e: MouseEvent) {
  if (isAnimatingRef.value || _hidden.value) return;

  if (resizeRaf.value) {
    cancelAnimationFrame(resizeRaf.value);
  }

  resizeRaf.value = requestAnimationFrame(() => {
    if (isAnimatingRef.value) return;

    const newWidth = e.clientX - (props.offsetX ?? 0);
    const clamped = Math.max(
      props.minWidth,
      Math.min(props.maxWidth, newWidth),
    );
    width.value = clamped;
    configRef.value = { ...configRef.value, width: clamped };
  });
}

function handleResizeEnd(e: MouseEvent) {
  if (resizeRaf.value) {
    cancelAnimationFrame(resizeRaf.value);
    resizeRaf.value = null;
  }
  isResizing.value = false;
  document.body.classList.remove("sidebar-resizing");

  const dx = Math.abs(e.clientX - mouseDownX);
  const dy = Math.abs(e.clientY - mouseDownY);
  if (dx < 4 && dy < 4) {
    lineExpanded.value = true;
    window.setTimeout(() => {
      lineExpanded.value = false;
      toggle();
    }, 180);
    return;
  }

  notifyConfigChange(configRef.value);
}

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

const wrapperStyle = computed(() => {
  if (_hidden.value) return { width: `${props.hiddenWidth}px` };
  return { width: `${width.value}px` };
});

const wrapperClasses = computed(() =>
  [
    "kepler-sidebar-wrapper",
    _hidden.value ? "hidden collapsed" : "",
    animating.value ? "animating" : "",
    isResizing.value ? "is-resizing" : "",
    props.className ?? "",
  ]
    .filter(Boolean)
    .join(" "),
);
</script>

<template>
  <div :class="wrapperClasses" :style="wrapperStyle">
    <div
      class="kepler-sidebar-content"
      :style="dragRegion ? { WebkitAppRegion: 'drag' } as any : undefined"
    >
      <slot v-if="!_hidden" :toggle="toggle" />
    </div>
    <div v-if="!_hidden" class="kepler-sidebar-resize-handle" @mousedown="handleResizeStart">
      <div :class="['kepler-resize-handle-line', lineExpanded ? 'expanded' : '']" />
    </div>
  </div>
</template>

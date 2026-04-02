<script setup lang="ts">
import { shallowRef, watch, onMounted, onUnmounted, computed } from "vue";
// eslint-disable-next-line import/no-unassigned-import
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
  offsetX?: number;
  collapsed?: boolean;
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
  offsetX: 0,
  collapsed: undefined,
});

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
  "update:collapsed": [collapsed: boolean];
}>();

const width = shallowRef(props.initialConfig?.width ?? props.defaultWidth);
const _collapsed = shallowRef(
  props.collapsed !== undefined
    ? props.collapsed
    : (props.initialConfig?.collapsed ?? false),
);
const _fullyHidden = shallowRef(false);
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
  collapsed: _collapsed.value,
});

watch([width, _collapsed], () => {
  configRef.value = { width: width.value, collapsed: _collapsed.value };
});

watch(
  () => props.collapsed,
  (val) => {
    if (val !== undefined && val !== _collapsed.value) {
      startAnimation();
      _collapsed.value = val;
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
  const next = !_fullyHidden.value;
  _fullyHidden.value = next;
  if (!next && _collapsed.value) {
    _collapsed.value = false;
    emit("update:collapsed", false);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (!props.toggleShortcut) return;

  const parts = props.toggleShortcut.toLowerCase().split("+");
  const key = parts[parts.length - 1];
  const needsMeta = parts.includes("meta");
  const needsCtrl = parts.includes("ctrl");
  const needsShift = parts.includes("shift");
  const needsAlt = props.toggleShortcut.toLowerCase().includes("alt");

  if (needsMeta && !e.metaKey) return;
  if (needsCtrl && !e.ctrlKey) return;
  if (needsShift && !e.shiftKey) return;
  if (needsAlt && !e.altKey) return;
  if (e.key !== key && e.key.toLowerCase() !== key) return;

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
  if (isAnimatingRef.value || _fullyHidden.value) return;

  if (resizeRaf.value) {
    cancelAnimationFrame(resizeRaf.value);
  }

  resizeRaf.value = requestAnimationFrame(() => {
    if (isAnimatingRef.value) return;

    const newWidth = e.clientX - (props.offsetX ?? 0);

    if (newWidth <= props.collapseThreshold) {
      if (!_collapsed.value) {
        startAnimation();
        _collapsed.value = true;
        configRef.value = { ...configRef.value, collapsed: true };
        emit("update:collapsed", true);
      }
      return;
    }

    if (_collapsed.value) {
      startAnimation();
      _collapsed.value = false;
      width.value = props.minWidth;
      configRef.value = { width: props.minWidth, collapsed: false };
      emit("update:collapsed", false);
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
  if (_fullyHidden.value) return { width: "6px", padding: "0" };
  if (_collapsed.value) return {};
  return { width: `${width.value}px` };
});

const wrapperClasses = computed(() =>
  [
    "kepler-sidebar-wrapper",
    _fullyHidden.value ? "fully-hidden" : "",
    _collapsed.value && !_fullyHidden.value ? "collapsed" : "",
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
      v-if="dragRegion && !_fullyHidden"
      :style="{ WebkitAppRegion: 'drag' } as any"
      class="kepler-sidebar-drag-region"
    />
    <div class="kepler-sidebar-content">
      <slot v-if="!_collapsed" :toggle="toggle" />
      <slot v-else-if="$slots.collapsed" name="collapsed" :toggle="toggle" />
    </div>
    <div class="kepler-sidebar-resize-handle" @mousedown="handleResizeStart">
      <div :class="['kepler-resize-handle-line', lineExpanded ? 'expanded' : '']" />
    </div>
  </div>
</template>

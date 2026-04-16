<script setup lang="ts">
import type { Component } from "vue";
import {
  computed,
  shallowRef,
  onMounted,
  onUnmounted,
  watch,
} from "vue";
import { RouterLink } from "vue-router";
import { PanelLeftClose } from "lucide-vue-next";
import SidebarButton from "./SidebarButton.vue";

export interface SidebarConfig {
    width: number;
    hidden: boolean;
}

export interface SidebarNavItem {
  id: string;
  icon: Component;
  to?: string;
  label?: string;
  active?: boolean;
  testId?: string;
  onClick?: () => void;
}

export interface SidebarProjectItem {
  id: string;
  label: string;
  to?: string;
  active?: boolean;
  colorClass?: string;
  color?: string;
  testId?: string;
  onClick?: () => void;
}

interface Props {
  primaryItems: SidebarNavItem[];
  projectItems?: SidebarProjectItem[];
  projectSectionLabel?: string;
  footerItems?: SidebarNavItem[];
  topItem?: SidebarNavItem;
  isMac?: boolean;
  className?: string;
  hidden?: boolean;
  offsetX?: number;
  toggleTitle?: string;
  defaultWidth?: number;
  minWidth?: number;
  maxWidth?: number;
  hiddenWidth?: number;
  toggleShortcut?: string;
  initialConfig?: Partial<SidebarConfig>;
  showToggle?: boolean;
  reserveTopInset?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  projectItems: () => [],
  projectSectionLabel: "Проекты",
  footerItems: () => [],
  topItem: undefined,
  isMac: false,
  className: undefined,
  hidden: undefined,
  offsetX: 0,
  toggleTitle: "Скрыть сайдбар (⌘B)",
  defaultWidth: 200,
  minWidth: 160,
  maxWidth: 320,
  hiddenWidth: 80,
  toggleShortcut: undefined,
  initialConfig: undefined,
  showToggle: true,
  reserveTopInset: true,
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

const hasProjects = computed(() => props.projectItems.length > 0);
const hasTopBar = computed(() => props.showToggle || Boolean(props.topItem));
const shellClasses = computed(() => [
    "kepler-sidebar-shell",
    { "kepler-sidebar-shell--mac-safe-top": props.isMac && props.reserveTopInset },
    { "kepler-sidebar-shell--with-top-bar": hasTopBar.value },
]);

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
    }, 330);
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
  <div :class="wrapperClasses" :style="wrapperStyle" data-testid="kepler-sidebar">
    <div class="kepler-sidebar-content">
      <aside v-if="!_hidden" :class="shellClasses">
        <div v-if="hasTopBar" class="kepler-sidebar-top">
          <button
            v-if="props.topItem"
            type="button"
            class="kepler-sidebar-top-toggle"
            :data-testid="props.topItem.testId"
            :title="props.topItem.label"
            @click="'onClick' in props.topItem && typeof props.topItem.onClick === 'function' ? props.topItem.onClick() : undefined"
          >
            <component :is="props.topItem.icon" :size="16" />
          </button>
          <button
            v-if="props.showToggle"
            type="button"
            class="kepler-sidebar-top-toggle"
            data-testid="sidebar-toggle"
            :title="toggleTitle"
            @click="toggle"
          >
            <PanelLeftClose :size="16" />
          </button>
        </div>

        <div class="kepler-sidebar-body">
          <SidebarButton
            v-for="item in primaryItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            @click="'onClick' in item && typeof item.onClick === 'function' ? item.onClick() : undefined"
          />

          <template v-if="hasProjects">
            <div class="kepler-sidebar-divider" />
            <div class="kepler-sidebar-section-label">
              {{ projectSectionLabel }}
            </div>
            <template v-for="project in projectItems" :key="project.id">
              <RouterLink
                v-if="project.to"
                :to="project.to"
                :data-testid="project.testId"
                :class="[
                  'kepler-sidebar-project-link',
                  'widget-nav-item',
                  project.active
                    ? 'kepler-sidebar-project-link--active'
                    : '',
                ]"
              >
                <span
                  :class="[
                    'kepler-sidebar-project-dot',
                    project.colorClass ?? 'bg-(--muted-foreground)',
                  ]"
                  :style="project.color ? { backgroundColor: project.color } : undefined"
                />
                <span class="kepler-sidebar-project-label">{{ project.label }}</span>
              </RouterLink>
              <button
                v-else
                type="button"
                :data-testid="project.testId"
                :class="[
                  'kepler-sidebar-project-link',
                  'widget-nav-item',
                  project.active
                    ? 'kepler-sidebar-project-link--active'
                    : '',
                ]"
                @click="'onClick' in project && typeof project.onClick === 'function' ? project.onClick() : undefined"
              >
                <span
                  :class="[
                    'kepler-sidebar-project-dot',
                    project.colorClass ?? 'bg-(--muted-foreground)',
                  ]"
                  :style="project.color ? { backgroundColor: project.color } : undefined"
                />
                <span class="kepler-sidebar-project-label">{{ project.label }}</span>
              </button>
            </template>
          </template>
        </div>

        <div v-if="footerItems.length > 0" class="kepler-sidebar-footer">
          <SidebarButton
            v-for="item in footerItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            @click="'onClick' in item && typeof item.onClick === 'function' ? item.onClick() : undefined"
          />
        </div>
      </aside>
    </div>

    <div
      v-if="!_hidden"
      class="kepler-sidebar-resize-handle"
      data-testid="kepler-sidebar-resize-handle"
      @mousedown="handleResizeStart"
    >
      <div :class="['kepler-resize-handle-line', lineExpanded ? 'expanded' : '']" />
    </div>
  </div>
</template>

<style scoped>
.kepler-sidebar-shell {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  min-height: 100%;
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  min-height: 0;
  padding: 0.5rem;
  background: var(--sidebar-bg);
}

.kepler-sidebar-shell::before {
  content: "";
  position: absolute;
  inset: 0;
  z-index: 0;
  /* -webkit-app-region: drag; */
}

.kepler-sidebar-top,
.kepler-sidebar-body,
.kepler-sidebar-footer {
  position: relative;
  z-index: 1;
}

.kepler-sidebar-shell--mac-safe-top {
  padding-top: var(--kepler-mac-sidebar-top-safe-area, 52px);
}

.kepler-sidebar-shell--mac-safe-top.kepler-sidebar-shell--with-top-bar {
  padding-top: 0.5rem;
}

.kepler-sidebar-top {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding-bottom: 0.75rem;
}

.kepler-sidebar-shell--mac-safe-top .kepler-sidebar-top {
  position: absolute;
  top: 12px;
  left: auto;
  right: 0.5rem;
  min-height: 28px;
  justify-content: flex-end;
  padding-bottom: 0;
  z-index: 1;
}

.kepler-sidebar-top-toggle {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0.375rem;
  border-radius: 0.5rem;
  color: color-mix(in srgb, var(--sidebar-foreground) 40%, transparent);
  -webkit-app-region: no-drag;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kepler-sidebar-top-toggle:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 6%, transparent);
  color: var(--sidebar-foreground);
}

.kepler-sidebar-body {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  gap: 0.125rem;
  overflow-y: auto;
}

.kepler-sidebar-shell--mac-safe-top.kepler-sidebar-shell--with-top-bar .kepler-sidebar-body {
  padding-top: 2.5rem;
}

.kepler-sidebar-divider {
  margin: 0.5rem 0;
  border-top: 1px solid var(--border);
}

.kepler-sidebar-section-label {
  padding: 0.5rem 0.75rem 0.25rem;
  color: color-mix(in srgb, var(--muted-foreground) 60%, transparent);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  user-select: none;
}

.kepler-sidebar-project-link {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  width: 100%;
  min-height: 2.5rem;
  padding: 0.5rem 0.75rem;
  border-radius: calc(var(--radius) * 1.4);
  corner-shape: var(--corner-shape);
  color: var(--muted-foreground);
  cursor: pointer;
  text-decoration: none;
  text-align: left;
  -webkit-app-region: no-drag;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kepler-sidebar-project-link:hover {
  background: var(--secondary);
  color: var(--foreground);
}

.kepler-sidebar-project-link--active {
  background: var(--surface);
  color: var(--foreground);
  font-weight: 500;
}

.kepler-sidebar-project-dot {
  display: block;
  width: 0.625rem;
  height: 0.625rem;
  flex-shrink: 0;
  border-radius: 999px;
}

.kepler-sidebar-project-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: none;
}

.kepler-sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
</style>

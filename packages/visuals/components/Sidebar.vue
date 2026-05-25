<script setup lang="ts">
import { computed, shallowRef, onMounted, onUnmounted, watch } from "vue";
import { RouterLink } from "vue-router";
import { ChevronRight, PanelLeftClose } from "@lucide/vue";
// eslint-disable-next-line import/no-unassigned-import
import "./sidebar.css";
import SidebarButton from "./SidebarButton.vue";
import type {
  SidebarConfig,
  SidebarNavItem,
  SidebarProjectItem,
  SidebarProjectGroup,
} from "./types";

interface Props {
  primaryItems: SidebarNavItem[];
  projectItems?: SidebarProjectItem[];
  projectSectionLabel?: string;
  secondaryProjectItems?: SidebarProjectItem[];
  secondaryProjectSectionLabel?: string;
  projectGroups?: SidebarProjectGroup[];
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
  toggleShortcut?: string;
  dragRegion?: boolean;
  initialConfig?: Partial<SidebarConfig>;
  showToggle?: boolean;
  reserveTopInset?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  projectItems: () => [],
  projectSectionLabel: "Проекты",
  secondaryProjectItems: () => [],
  secondaryProjectSectionLabel: "Ещё",
  projectGroups: () => [],
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
  toggleShortcut: "meta+b|ctrl+b",
  dragRegion: true,
  initialConfig: undefined,
  showToggle: true,
  reserveTopInset: true,
});

const emit = defineEmits<{
  configChange: [config: SidebarConfig];
  "update:hidden": [hidden: boolean];
}>();

const keyLayoutAliases: Record<string, string> = {
  q: "й",
  w: "ц",
  e: "у",
  r: "к",
  t: "е",
  y: "н",
  u: "г",
  i: "ш",
  o: "щ",
  p: "з",
  "[": "х",
  "]": "ъ",
  a: "ф",
  s: "ы",
  d: "в",
  f: "а",
  g: "п",
  h: "р",
  j: "о",
  k: "л",
  l: "д",
  ";": "ж",
  "'": "э",
  z: "я",
  x: "ч",
  c: "с",
  v: "м",
  b: "и",
  n: "т",
  m: "ь",
  ",": "б",
  ".": "ю",
};
const reverseKeyLayoutAliases = Object.fromEntries(
  Object.entries(keyLayoutAliases).map(([latinKey, localizedKey]) => [localizedKey, latinKey]),
) as Record<string, string>;

const width = shallowRef(props.initialConfig?.width ?? props.defaultWidth);
const _hidden = shallowRef(
  props.hidden !== undefined ? props.hidden : (props.initialConfig?.hidden ?? false),
);
const isResizing = shallowRef(false);
const animating = shallowRef(false);
const lineExpanded = shallowRef(false);
const collapsedGroups = shallowRef<Record<string, boolean>>({});

let mouseDownX = 0;
let mouseDownY = 0;

const resizeRaf = shallowRef<number | null>(null);
const animTimer = shallowRef<number | null>(null);
const isAnimatingRef = shallowRef(false);

const configRef = shallowRef<SidebarConfig>({
  width: width.value,
  hidden: _hidden.value,
});

const groupedProjectSections = computed<SidebarProjectGroup[]>(() => {
  if (props.projectGroups.length > 0) {
    return props.projectGroups.filter(
      (group) => group.items.length > 0 || typeof group.onAction === "function",
    );
  }

  const groups: SidebarProjectGroup[] = [];

  if (props.projectItems.length > 0) {
    groups.push({
      id: "legacy-primary-project-group",
      label: props.projectSectionLabel,
      items: props.projectItems,
    });
  }

  if (props.secondaryProjectItems.length > 0) {
    groups.push({
      id: "legacy-secondary-project-group",
      label: props.secondaryProjectSectionLabel,
      items: props.secondaryProjectItems,
    });
  }

  return groups;
});

const hasProjectGroups = computed(() => groupedProjectSections.value.length > 0);
const hasTopBar = computed(() => props.showToggle || Boolean(props.topItem));
const shellClasses = computed(() => [
  "kosmos-sidebar-shell",
  { "kosmos-sidebar-shell--mac-safe-top": props.isMac && props.reserveTopInset },
  { "kosmos-sidebar-shell--with-top-bar": hasTopBar.value },
  { "kosmos-sidebar-shell--drag-region": props.dragRegion },
]);

watch([width, _hidden], () => {
  configRef.value = { width: width.value, hidden: _hidden.value };
});

watch(
  groupedProjectSections,
  (groups) => {
    const nextState: Record<string, boolean> = {};
    for (const group of groups) {
      nextState[group.id] = collapsedGroups.value[group.id] ?? !!group.defaultCollapsed;
    }
    collapsedGroups.value = nextState;
  },
  { immediate: true, deep: true },
);

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

function expandKeyVariants(key: string | null | undefined): Set<string> {
  const variants = new Set<string>();
  if (!key) return variants;

  const normalized = key.toLowerCase();
  variants.add(normalized);

  const alias = keyLayoutAliases[normalized];
  if (alias) {
    variants.add(alias);
  }

  const reverseAlias = reverseKeyLayoutAliases[normalized];
  if (reverseAlias) {
    variants.add(reverseAlias);
  }

  return variants;
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

  const shortcutKeys = expandKeyVariants(key);
  const eventKeys = expandKeyVariants(e.key);
  const codeKey = e.code.startsWith("Key") ? e.code.slice(3).toLowerCase() : null;
  for (const codeVariant of expandKeyVariants(codeKey)) {
    eventKeys.add(codeVariant);
  }

  return [...shortcutKeys].some((shortcutKey) => eventKeys.has(shortcutKey));
}

function handleKeydown(e: KeyboardEvent) {
  if (!props.toggleShortcut) return;

  const shortcuts = props.toggleShortcut.split("|");
  if (!shortcuts.some((shortcut) => matchesShortcut(e, shortcut))) return;

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
    const clamped = Math.max(props.minWidth, Math.min(props.maxWidth, newWidth));
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

function isGroupCollapsed(groupId: string): boolean {
  return collapsedGroups.value[groupId] ?? false;
}

function toggleGroup(groupId: string) {
  collapsedGroups.value = {
    ...collapsedGroups.value,
    [groupId]: !isGroupCollapsed(groupId),
  };
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
  if (_hidden.value) return { width: "0px" };
  return { width: `${width.value}px` };
});

const wrapperClasses = computed(() =>
  [
    "kosmos-sidebar-wrapper",
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
  <div :class="wrapperClasses" :style="wrapperStyle" data-testid="kosmos-sidebar">
    <div class="kosmos-sidebar-content">
      <aside v-if="!_hidden" :class="shellClasses">
        <div v-if="hasTopBar" class="kosmos-sidebar-top">
          <button
            v-if="props.topItem"
            type="button"
            class="kosmos-sidebar-top-toggle"
            :data-testid="props.topItem.testId"
            :title="props.topItem.label"
            @click="
              'onClick' in props.topItem && typeof props.topItem.onClick === 'function'
                ? props.topItem.onClick()
                : undefined
            "
          >
            <component :is="props.topItem.icon" :size="16" />
          </button>
          <button
            v-if="props.showToggle"
            type="button"
            class="kosmos-sidebar-top-toggle"
            data-testid="sidebar-toggle"
            :title="toggleTitle"
            @click="toggle"
          >
            <PanelLeftClose :size="16" />
          </button>
        </div>

        <div class="kosmos-sidebar-body">
          <SidebarButton
            v-for="item in primaryItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            @click="
              'onClick' in item && typeof item.onClick === 'function' ? item.onClick() : undefined
            "
          />

          <template v-if="hasProjectGroups">
            <div class="kosmos-sidebar-groups">
              <section
                v-for="group in groupedProjectSections"
                :key="group.id"
                class="kosmos-sidebar-group"
              >
                <div class="kosmos-sidebar-group-header-row">
                  <button
                    type="button"
                    class="kosmos-sidebar-group-header"
                    :data-testid="`sidebar-group-${group.id}`"
                    @click="toggleGroup(group.id)"
                  >
                    <ChevronRight
                      :size="14"
                      :class="[
                        'kosmos-sidebar-group-chevron',
                        isGroupCollapsed(group.id) ? '' : 'kosmos-sidebar-group-chevron--expanded',
                      ]"
                    />
                    <span>{{ group.label }}</span>
                  </button>

                  <button
                    v-if="group.actionIcon && group.onAction"
                    type="button"
                    class="kosmos-sidebar-group-action"
                    :title="group.actionLabel"
                    :data-testid="group.actionTestId"
                    @click="group.onAction()"
                  >
                    <component :is="group.actionIcon" :size="14" />
                  </button>
                </div>

                <div
                  v-if="!isGroupCollapsed(group.id) && group.items.length > 0"
                  class="kosmos-sidebar-group-surface"
                >
                  <template v-for="project in group.items" :key="project.id">
                    <RouterLink
                      v-if="project.to"
                      :to="project.to"
                      :data-testid="project.testId"
                      :class="[
                        'kosmos-sidebar-project-link',
                        'widget-nav-item',
                        project.active ? 'kosmos-sidebar-project-link--active' : '',
                      ]"
                      @contextmenu="project.onContextMenu?.($event)"
                    >
                      <span
                        v-if="project.iconSrc"
                        class="kosmos-sidebar-project-icon-wrap"
                        :style="{
                          '--kosmos-project-icon-color':
                            project.iconColor ?? project.color ?? 'var(--muted-foreground)',
                        }"
                      >
                        <span
                          class="kosmos-sidebar-project-icon"
                          :style="{ '--kosmos-project-icon-src': `url(${project.iconSrc})` }"
                          aria-hidden="true"
                        />
                      </span>
                      <span
                        v-else
                        :class="[
                          'kosmos-sidebar-project-dot',
                          project.colorClass ?? 'bg-(--muted-foreground)',
                        ]"
                        :style="project.color ? { backgroundColor: project.color } : undefined"
                      />
                      <span class="kosmos-sidebar-project-label">{{ project.label }}</span>
                    </RouterLink>

                    <button
                      v-else
                      type="button"
                      :data-testid="project.testId"
                      :class="[
                        'kosmos-sidebar-project-link',
                        'widget-nav-item',
                        project.active ? 'kosmos-sidebar-project-link--active' : '',
                      ]"
                      @click="
                        'onClick' in project && typeof project.onClick === 'function'
                          ? project.onClick()
                          : undefined
                      "
                      @contextmenu="project.onContextMenu?.($event)"
                    >
                      <span
                        v-if="project.iconSrc"
                        class="kosmos-sidebar-project-icon-wrap"
                        :style="{
                          '--kosmos-project-icon-color':
                            project.iconColor ?? project.color ?? 'var(--muted-foreground)',
                        }"
                      >
                        <span
                          class="kosmos-sidebar-project-icon"
                          :style="{ '--kosmos-project-icon-src': `url(${project.iconSrc})` }"
                          aria-hidden="true"
                        />
                      </span>
                      <span
                        v-else
                        :class="[
                          'kosmos-sidebar-project-dot',
                          project.colorClass ?? 'bg-(--muted-foreground)',
                        ]"
                        :style="project.color ? { backgroundColor: project.color } : undefined"
                      />
                      <span class="kosmos-sidebar-project-label">{{ project.label }}</span>
                    </button>
                  </template>
                </div>
              </section>
            </div>
          </template>
        </div>

        <div v-if="footerItems.length > 0" class="kosmos-sidebar-footer">
          <SidebarButton
            v-for="item in footerItems"
            :key="item.id"
            :icon="item.icon"
            :to="item.to"
            :label="item.label"
            :active="item.active"
            :test-id="item.testId"
            @click="
              'onClick' in item && typeof item.onClick === 'function' ? item.onClick() : undefined
            "
          />
        </div>
      </aside>
    </div>

    <div
      v-if="!_hidden"
      class="kosmos-sidebar-resize-handle"
      data-testid="kosmos-sidebar-resize-handle"
      @mousedown="handleResizeStart"
    >
      <div :class="['kosmos-resize-handle-line', lineExpanded ? 'expanded' : '']" />
    </div>
  </div>
</template>

<style scoped>
.kosmos-sidebar-shell {
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

.kosmos-sidebar-shell::before {
  content: "";
  position: absolute;
  inset: 0;
  z-index: 0;
}

.kosmos-sidebar-shell--drag-region::before {
  -webkit-app-region: drag;
}

.kosmos-sidebar-top,
.kosmos-sidebar-body,
.kosmos-sidebar-footer {
  position: relative;
  z-index: 1;
}

.kosmos-sidebar-shell--mac-safe-top {
  padding-top: var(--kosmos-mac-sidebar-top-safe-area, 52px);
}

.kosmos-sidebar-shell--mac-safe-top.kosmos-sidebar-shell--with-top-bar {
  padding-top: 0.5rem;
}

.kosmos-sidebar-top {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding-bottom: 0.75rem;
}

.kosmos-sidebar-shell--mac-safe-top .kosmos-sidebar-top {
  position: absolute;
  top: 12px;
  right: 0.5rem;
  min-height: 28px;
  justify-content: flex-end;
  padding-bottom: 0;
  z-index: 1;
}

.kosmos-sidebar-top-toggle {
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

.kosmos-sidebar-top-toggle:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 6%, transparent);
  color: var(--sidebar-foreground);
}

.kosmos-sidebar-body {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  gap: 0.125rem;
  overflow-y: auto;
}

.kosmos-sidebar-shell--mac-safe-top.kosmos-sidebar-shell--with-top-bar .kosmos-sidebar-body {
  padding-top: 2.5rem;
}

.kosmos-sidebar-divider {
  margin: 0.5rem 0;
  border-top: 1px solid var(--border);
}

.kosmos-sidebar-groups {
  display: grid;
  margin-top: 0.875rem;
  gap: 1.25rem;
}

.kosmos-sidebar-group {
  display: grid;
  gap: 0.375rem;
}

.kosmos-sidebar-group-header-row {
  display: flex;
  align-items: center;
  gap: 0.375rem;
}

.kosmos-sidebar-group-header {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  flex: 1;
  min-width: 0;
  padding: 0 0.375rem;
  color: color-mix(in srgb, var(--muted-foreground) 88%, transparent);
  font-size: 0.8125rem;
  font-weight: 600;
  text-align: left;
}

.kosmos-sidebar-group-header:hover {
  color: var(--foreground);
}

.kosmos-sidebar-group-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 1.75rem;
  height: 1.75rem;
  flex-shrink: 0;
  border-radius: 999px;
  color: color-mix(in srgb, var(--muted-foreground) 88%, transparent);
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-sidebar-group-action:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
  color: var(--foreground);
}

.kosmos-sidebar-group-chevron {
  flex-shrink: 0;
  transition: transform 140ms ease;
}

.kosmos-sidebar-group-chevron--expanded {
  transform: rotate(90deg);
}

.kosmos-sidebar-group-surface {
  display: grid;
  gap: 0.125rem;
  padding: 0.375rem;
  border-radius: 1.125rem;
  background: color-mix(in srgb, var(--sidebar-foreground) 4%, transparent);
}

.kosmos-sidebar-project-link {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 0.625rem;
  width: 100%;
  min-height: 1.75rem;
  padding: 0.25rem;
  border-radius: calc(var(--radius) * 1.4);
  corner-shape: var(--corner-shape);
  color: var(--muted-foreground);
  font-size: 0.875rem;
  line-height: 1.25rem;
  font-weight: 500;
  text-decoration: none;
  text-align: left;
  -webkit-app-region: no-drag;
  transition:
    background-color 120ms cubic-bezier(0.2, 0, 0, 1),
    color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.kosmos-sidebar-project-link:hover {
  background: color-mix(in srgb, var(--sidebar-foreground) 6%, transparent);
  color: var(--foreground);
}

.kosmos-sidebar-project-link--active {
  background: color-mix(in srgb, var(--sidebar-foreground) 10%, transparent);
  color: var(--foreground);
  font-weight: 500;
}

.kosmos-sidebar-project-dot {
  display: block;
  width: 0.625rem;
  height: 0.625rem;
  flex-shrink: 0;
  border-radius: 999px;
}

.kosmos-sidebar-project-icon-wrap {
  width: 1rem;
  height: 1rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.kosmos-sidebar-project-icon {
  width: 1rem;
  height: 1rem;
  display: block;
  background-color: var(--kosmos-project-icon-color);
  -webkit-mask-image: var(--kosmos-project-icon-src);
  mask-image: var(--kosmos-project-icon-src);
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
  -webkit-mask-position: center;
  mask-position: center;
  -webkit-mask-size: contain;
  mask-size: contain;
  opacity: 0.92;
}

.kosmos-sidebar-project-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: none;
}

.kosmos-sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
</style>

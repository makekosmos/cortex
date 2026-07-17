<script setup lang="ts">
import { computed, shallowRef, onMounted, onUnmounted, useSlots, watch } from "vue";
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
  mode?: "navigation" | "panel";
  primaryItems?: SidebarNavItem[];
  tone?: "default" | "strong";
  title?: string;
  background?: string;
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
  mode: "navigation",
  primaryItems: () => [],
  tone: "default",
  title: undefined,
  background: undefined,
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
  defaultWidth: 240,
  minWidth: 160,
  maxWidth: 320,
  toggleShortcut: "meta+b|ctrl+b",
  dragRegion: true,
  initialConfig: undefined,
  showToggle: true,
  reserveTopInset: true,
});

const slots = useSlots();
const hasPanelTitleBar = computed(
  () => Boolean(props.title) || Boolean(slots["title-leading"]) || Boolean(slots["title-trailing"]),
);
const panelStyle = computed(() => ({
  "--kosmos-settings-sidebar-bg": props.background ?? "var(--sidebar-bg, var(--bg-app, #1d1d1f))",
}));

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

const SIDEBAR_WIDTH_STEP = 8;

function snapSidebarWidth(value: number): number {
  return Math.round(value / SIDEBAR_WIDTH_STEP) * SIDEBAR_WIDTH_STEP;
}

function clampSidebarWidth(value: number): number {
  const snappedMin = snapSidebarWidth(props.minWidth);
  const snappedMax = snapSidebarWidth(props.maxWidth);
  return Math.max(snappedMin, Math.min(snappedMax, snapSidebarWidth(value)));
}

const width = shallowRef(clampSidebarWidth(props.initialConfig?.width ?? props.defaultWidth));
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
const hasTopBar = computed(
  () => props.showToggle || Boolean(props.topItem) || Boolean(props.title),
);
const shellClasses = computed(() => [
  "kosmos-sidebar-shell",
  "absolute inset-0 box-border flex h-full min-h-full w-full min-h-0 flex-col justify-between bg-(--sidebar-bg) p-2",
  { "kosmos-sidebar-shell--mac-safe-top": props.isMac && props.reserveTopInset },
  { "kosmos-sidebar-shell--with-top-bar": hasTopBar.value },
  { "kosmos-sidebar-shell--drag-region": props.dragRegion },
]);

const topToggleClass =
  "kosmos-sidebar-top-toggle inline-flex size-8 items-center justify-center rounded-lg p-2 text-[color-mix(in_srgb,var(--sidebar-foreground)_40%,transparent)] transition-[background-color,color] duration-[120ms] ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--sidebar-foreground)_6%,transparent)] hover:text-(--sidebar-foreground)";
const groupHeaderClass =
  "inline-flex min-w-0 flex-1 items-center gap-2 px-2 text-left text-[0.8125rem] font-semibold text-[color-mix(in_srgb,var(--muted-foreground)_88%,transparent)] hover:text-(--foreground)";
const groupActionClass =
  "inline-flex size-8 shrink-0 items-center justify-center rounded-full text-[color-mix(in_srgb,var(--muted-foreground)_88%,transparent)] transition-[background-color,color] duration-[120ms] ease-[cubic-bezier(0.2,0,0,1)] hover:bg-[color-mix(in_srgb,var(--sidebar-foreground)_8%,transparent)] hover:text-(--foreground)";
const projectLinkBaseClass =
  "kosmos-sidebar-project-link widget-nav-item flex min-h-8 w-full items-center justify-start gap-2 rounded-lg p-2 text-left text-sm leading-5 font-medium text-(--muted-foreground) no-underline transition-[background-color,color] duration-[120ms] ease-[cubic-bezier(0.2,0,0,1)] [corner-shape:var(--corner-shape)] hover:bg-[color-mix(in_srgb,var(--sidebar-foreground)_6%,transparent)] hover:text-(--foreground)";
const projectLinkActiveClass =
  "bg-[color-mix(in_srgb,var(--sidebar-foreground)_10%,transparent)] text-(--foreground)";

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
  if (props.mode !== "navigation" || !props.toggleShortcut) return;

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
    const clamped = clampSidebarWidth(newWidth);
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
  if (props.mode === "navigation") window.addEventListener("keydown", handleKeydown);
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
  const background = props.background ? { "--sidebar-bg": props.background } : {};
  if (_hidden.value) return { width: "0px", ...background };
  return { width: `${width.value}px`, ...background };
});

const wrapperClasses = computed(() =>
  [
    "kosmos-sidebar-wrapper",
    "relative z-[2] block h-full min-h-full shrink-0 self-stretch overflow-visible box-border transition-[width] duration-[375ms] ease-[cubic-bezier(0.22,1,0.36,1)]",
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
  <aside
    v-if="props.mode === 'panel'"
    class="kosmos-settings-sidebar box-border flex h-full w-[228px] min-w-[228px] flex-col gap-4 border-r border-[var(--border-color-strong)] bg-[var(--kosmos-settings-sidebar-bg)] text-white"
    :data-tone="props.tone"
    :style="panelStyle"
  >
    <div
      v-if="hasPanelTitleBar"
      class="kosmos-settings-sidebar__title flex items-center font-[var(--font-sans)] text-[length:var(--kosmos-text-caption-size)] leading-[1.4] font-medium [-webkit-app-region:drag]"
    >
      <div
        v-if="$slots['title-leading']"
        class="inline-flex shrink-0 items-center [-webkit-app-region:no-drag]"
      >
        <slot name="title-leading" />
      </div>
      <span v-if="props.title" class="min-w-0 truncate px-1">{{ props.title }}</span>
      <div
        v-if="$slots['title-trailing']"
        class="ml-auto inline-flex shrink-0 items-center [-webkit-app-region:no-drag]"
      >
        <slot name="title-trailing" />
      </div>
    </div>
    <div
      class="kosmos-settings-sidebar__content flex min-h-0 flex-1 flex-col gap-6 [-webkit-app-region:no-drag]"
    >
      <slot />
    </div>
  </aside>

  <div v-else :class="wrapperClasses" :style="wrapperStyle" data-testid="kosmos-sidebar">
    <div
      class="kosmos-sidebar-content absolute inset-0 flex min-h-0 min-w-0 overflow-hidden bg-(--sidebar-bg) opacity-100 transition-[opacity,background,border-color] duration-300 ease-[cubic-bezier(0.22,1,0.36,1)]"
    >
      <aside v-if="!_hidden" :class="shellClasses">
        <div
          v-if="hasTopBar"
          :class="[
            'kosmos-sidebar-top relative z-[1] flex items-center justify-end pb-4',
            { 'kosmos-sidebar-top--titled': props.title },
          ]"
        >
          <span v-if="props.title" class="kosmos-sidebar-title mr-auto min-w-0 truncate px-1">
            {{ props.title }}
          </span>
          <button
            v-if="props.topItem"
            type="button"
            :class="topToggleClass"
            :data-testid="props.topItem.testId"
            :title="props.topItem.label"
            @click="
              'onClick' in props.topItem && typeof props.topItem.onClick === 'function'
                ? props.topItem.onClick()
                : undefined
            "
          >
            <component :is="props.topItem.icon" :size="24" />
          </button>
          <button
            v-if="props.showToggle"
            type="button"
            :class="topToggleClass"
            data-testid="sidebar-toggle"
            :title="toggleTitle"
            @click="toggle"
          >
            <PanelLeftClose :size="24" />
          </button>
        </div>

        <div
          class="kosmos-sidebar-body relative z-[1] flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto"
        >
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

          <slot />

          <template v-if="hasProjectGroups">
            <div class="mt-4 grid gap-4">
              <section v-for="group in groupedProjectSections" :key="group.id" class="grid gap-2">
                <div class="flex items-center gap-2">
                  <button
                    type="button"
                    :class="groupHeaderClass"
                    :data-testid="`sidebar-group-${group.id}`"
                    @click="toggleGroup(group.id)"
                  >
                    <ChevronRight
                      :size="16"
                      :class="[
                        'shrink-0 transition-transform duration-[140ms] ease-in',
                        isGroupCollapsed(group.id) ? '' : 'rotate-90',
                      ]"
                    />
                    <span>{{ group.label }}</span>
                  </button>

                  <button
                    v-if="group.actionIcon && group.onAction"
                    type="button"
                    :class="groupActionClass"
                    :title="group.actionLabel"
                    :data-testid="group.actionTestId"
                    @click="group.onAction()"
                  >
                    <component :is="group.actionIcon" :size="16" />
                  </button>
                </div>

                <div
                  v-if="!isGroupCollapsed(group.id) && group.items.length > 0"
                  class="grid gap-2 rounded-2xl bg-[color-mix(in_srgb,var(--sidebar-foreground)_4%,transparent)] p-2"
                >
                  <template v-for="project in group.items" :key="project.id">
                    <RouterLink
                      v-if="project.to"
                      :to="project.to"
                      :data-testid="project.testId"
                      :class="[projectLinkBaseClass, project.active ? projectLinkActiveClass : '']"
                      @contextmenu="project.onContextMenu?.($event)"
                    >
                      <span
                        v-if="project.iconSrc"
                        class="kosmos-sidebar-project-icon-wrap inline-flex size-4 shrink-0 items-center justify-center"
                        :style="{
                          '--kosmos-project-icon-color':
                            project.iconColor ?? project.color ?? 'var(--muted-foreground)',
                        }"
                      >
                        <span
                          class="kosmos-sidebar-project-icon block size-4 bg-(--kosmos-project-icon-color) opacity-[0.92]"
                          :style="{ '--kosmos-project-icon-src': `url(${project.iconSrc})` }"
                          aria-hidden="true"
                        />
                      </span>
                      <span
                        v-else
                        :class="[
                          'kosmos-sidebar-project-dot',
                          'block size-2 shrink-0 rounded-full',
                          project.colorClass ?? 'bg-(--muted-foreground)',
                        ]"
                        :style="project.color ? { backgroundColor: project.color } : undefined"
                      />
                      <span class="min-w-0 flex-1 truncate select-none">{{ project.label }}</span>
                    </RouterLink>

                    <button
                      v-else
                      type="button"
                      :data-testid="project.testId"
                      :class="[projectLinkBaseClass, project.active ? projectLinkActiveClass : '']"
                      @click="
                        'onClick' in project && typeof project.onClick === 'function'
                          ? project.onClick()
                          : undefined
                      "
                      @contextmenu="project.onContextMenu?.($event)"
                    >
                      <span
                        v-if="project.iconSrc"
                        class="kosmos-sidebar-project-icon-wrap inline-flex size-4 shrink-0 items-center justify-center"
                        :style="{
                          '--kosmos-project-icon-color':
                            project.iconColor ?? project.color ?? 'var(--muted-foreground)',
                        }"
                      >
                        <span
                          class="kosmos-sidebar-project-icon block size-4 bg-(--kosmos-project-icon-color) opacity-[0.92]"
                          :style="{ '--kosmos-project-icon-src': `url(${project.iconSrc})` }"
                          aria-hidden="true"
                        />
                      </span>
                      <span
                        v-else
                        :class="[
                          'kosmos-sidebar-project-dot',
                          'block size-2 shrink-0 rounded-full',
                          project.colorClass ?? 'bg-(--muted-foreground)',
                        ]"
                        :style="project.color ? { backgroundColor: project.color } : undefined"
                      />
                      <span class="min-w-0 flex-1 truncate select-none">{{ project.label }}</span>
                    </button>
                  </template>
                </div>
              </section>
            </div>
          </template>
        </div>

        <div v-if="footerItems.length > 0" class="relative z-[1] flex flex-col gap-2">
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
      class="kosmos-sidebar-resize-handle absolute top-0 right-0 z-[80] block h-full w-2 shrink-0 cursor-ew-resize bg-transparent p-0 [touch-action:none] [-webkit-app-region:no-drag]"
      data-testid="kosmos-sidebar-resize-handle"
      @mousedown="handleResizeStart"
    >
      <div
        :class="[
          'kosmos-resize-handle-line pointer-events-none absolute top-1/2 left-1/2 h-[var(--kosmos-resize-handle-height,32px)] w-[var(--kosmos-resize-handle-thickness,2px)] -translate-x-1/2 -translate-y-1/2 rounded-[1px] bg-[var(--kosmos-resize-handle-color,color-mix(in_oklab,var(--sidebar-foreground,var(--foreground,currentColor))_60%,transparent))] opacity-0 transition-opacity duration-[150ms] ease-in',
          lineExpanded ? 'expanded' : '',
        ]"
      />
    </div>
  </div>
</template>

<style scoped>
.kosmos-settings-sidebar__title {
  box-sizing: border-box;
  height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-settings-titlebar-height))
  );
  min-height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-settings-titlebar-height))
  );
  gap: var(--kosmos-titlebar-control-gap);
  border-bottom: 1px solid var(--border-color-strong);
  padding: env(titlebar-area-y, 0px) 10px 0;
}

.kosmos-settings-sidebar__title :deep([data-testid="sidebar-header"]) {
  height: 100%;
  align-items: center;
  gap: var(--kosmos-titlebar-control-gap);
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

.kosmos-sidebar-shell--mac-safe-top {
  padding-top: var(--kosmos-mac-sidebar-top-safe-area, 56px);
}

.kosmos-sidebar-shell--mac-safe-top.kosmos-sidebar-shell--with-top-bar {
  padding-top: 0.5rem;
}

.kosmos-sidebar-shell--mac-safe-top .kosmos-sidebar-top {
  position: absolute;
  top: 16px;
  right: 0.5rem;
  min-height: 32px;
  justify-content: flex-end;
  padding-bottom: 0;
  z-index: 1;
}

.kosmos-sidebar-top-toggle {
  -webkit-app-region: no-drag;
}

.kosmos-sidebar-top--titled {
  box-sizing: border-box;
  height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-settings-titlebar-height))
  );
  min-height: calc(
    env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-settings-titlebar-height))
  );
  margin: -0.5rem -0.5rem 0.5rem;
  border-bottom: 1px solid var(--border-color-strong);
  padding: env(titlebar-area-y, 0px) 10px 0;
}

.kosmos-sidebar-title {
  color: var(--sidebar-foreground);
  font-family: var(--font-sans);
  font-size: var(--kosmos-text-caption-size);
  font-weight: 600;
  line-height: 1.4;
}

.kosmos-sidebar-shell--mac-safe-top.kosmos-sidebar-shell--with-top-bar .kosmos-sidebar-body {
  padding-top: 2.5rem;
}

.kosmos-sidebar-project-link {
  -webkit-app-region: no-drag;
}

.kosmos-sidebar-project-icon {
  -webkit-mask-image: var(--kosmos-project-icon-src);
  mask-image: var(--kosmos-project-icon-src);
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
  -webkit-mask-position: center;
  mask-position: center;
  -webkit-mask-size: contain;
  mask-size: contain;
}
</style>

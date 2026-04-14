<template>
  <aside class="main-sidebar-shell widget-sidebar" data-testid="main-sidebar">
    <div class="main-sidebar-topbar">
      <div class="main-sidebar-topbar-left">
        <button
          v-if="isVaultSidebarHidden"
          class="main-sidebar-chrome-btn"
          title="Показать хранилища"
          type="button"
          @click="emit('toggleVaultSidebar')"
        >
          <ChromeIcon name="panel-left" />
        </button>
      </div>

      <div class="main-sidebar-topbar-right">
        <button
          class="main-sidebar-chrome-btn"
          data-testid="sidebar-toggle"
          title="Скрыть сайдбар"
          type="button"
          @click="emit('toggleCollapse')"
        >
          <ChromeIcon name="panel-left-close" />
        </button>
      </div>
    </div>

    <div class="main-sidebar-scroll">
      <section class="main-sidebar-actions">
        <button
          class="main-sidebar-action-btn"
          title="Новая заметка"
          type="button"
          @click="emit('createRootEntry')"
        >
          <span class="main-sidebar-action-icon">
            <ChromeIcon name="plus" />
          </span>
          <span>Создать</span>
        </button>
        <button
          class="main-sidebar-action-btn"
          :class="{ active: isSearchOpen || searchQuery }"
          data-testid="widget-link-search"
          type="button"
          @click="emit('searchToggle')"
        >
          <span class="main-sidebar-action-icon">
            <ChromeIcon name="search" />
          </span>
          <span>Поиск</span>
        </button>
      </section>

      <section class="main-sidebar-section">
        <div class="main-sidebar-section-title">Основное</div>
        <button
          v-for="item in primaryItems"
          :key="item.id"
          class="main-sidebar-nav-btn"
          :class="{ active: item.active }"
          :data-testid="item.testId"
          type="button"
          @click="onPrimaryClick(item.id)"
        >
          <span class="main-sidebar-nav-icon" :class="item.iconClass">
            <template v-if="item.iconClass === 'badge-icon'">
              <span class="itemBadge" :class="item.badgeClass">{{ item.badgeText }}</span>
            </template>
            <template v-else>
              <ChromeIcon :name="item.icon" />
            </template>
          </span>
          <span class="main-sidebar-nav-label">{{ item.label }}</span>
        </button>
      </section>

      <section v-if="recentEntries.length > 0" class="main-sidebar-section">
        <div class="main-sidebar-section-title">Недавние</div>
        <button
          v-for="entry in recentEntries"
          :key="entry.id"
          class="main-sidebar-nav-btn recent widget-nav-item"
          :class="{ active: currentEntry?.id === entry.id }"
          type="button"
          @click="emit('openEntry', entry.id)"
        >
          <span class="main-sidebar-nav-icon badge-icon">
            <span v-if="entry.type_id && getNoteType(entry)" class="itemBadge type-badge">
              {{ getNoteType(entry)?.icon ?? "T" }}
            </span>
            <span v-else class="itemBadge document-badge" />
          </span>
          <span class="main-sidebar-nav-label">{{ getEntryLabel(entry) }}</span>
        </button>
      </section>
    </div>

    <div class="main-sidebar-footer">
      <button
        class="main-sidebar-nav-btn footer"
        :class="{ active: activeScreen === 'settings' }"
        data-testid="open-settings-btn"
        type="button"
        @click="emit('openSettings')"
      >
        <span class="main-sidebar-nav-icon chrome-icon-wrap">
          <ChromeIcon name="settings" />
        </span>
        <span class="main-sidebar-nav-label">Настройки</span>
      </button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, h } from "vue";
import type { SpaceId } from "./types";

type ChromeIconName =
  | "panel-left"
  | "panel-left-close"
  | "plus"
  | "search"
  | "settings"
  | "grid"
  | "book";

const ChromeIcon = (props: { name: ChromeIconName }) => {
  const common = {
    fill: "none",
    stroke: "currentColor",
    "stroke-width": "1.8",
    "stroke-linecap": "round",
    "stroke-linejoin": "round",
    viewBox: "0 0 20 20",
    width: "16",
    height: "16",
    "aria-hidden": "true",
  };

  const paths: Record<ChromeIconName, ReturnType<typeof h>[]> = {
    "panel-left": [
      h("rect", { x: "3.5", y: "4", width: "13", height: "12", rx: "2.5" }),
      h("path", { d: "M8 4v12" }),
    ],
    "panel-left-close": [
      h("rect", { x: "3.5", y: "4", width: "13", height: "12", rx: "2.5" }),
      h("path", { d: "M8 4v12" }),
      h("path", { d: "m11 8 3 2-3 2" }),
    ],
    plus: [h("path", { d: "M10 5.5v9M5.5 10h9" })],
    search: [
      h("circle", { cx: "9", cy: "9", r: "4.5" }),
      h("path", { d: "m12.5 12.5 3 3" }),
    ],
    settings: [
      h("circle", { cx: "10", cy: "10", r: "2.5" }),
      h("path", {
        d: "M10 4.5v1.2M10 14.3v1.2M15.5 10h-1.2M5.7 10H4.5M13.89 6.11l-.85.85M6.96 13.04l-.85.85M13.89 13.89l-.85-.85M6.96 6.96l-.85-.85",
      }),
    ],
    grid: [
      h("rect", { x: "4.5", y: "4.5", width: "4", height: "4", rx: "1" }),
      h("rect", { x: "11.5", y: "4.5", width: "4", height: "4", rx: "1" }),
      h("rect", { x: "4.5", y: "11.5", width: "4", height: "4", rx: "1" }),
      h("rect", { x: "11.5", y: "11.5", width: "4", height: "4", rx: "1" }),
    ],
    book: [
      h("path", { d: "M6 4.5h7a1.5 1.5 0 0 1 1.5 1.5v8.5H7.5A1.5 1.5 0 0 0 6 16" }),
      h("path", { d: "M6 4.5v11.5" }),
      h("path", { d: "M6 6h7" }),
    ],
  };

  return h("svg", common, paths[props.name]);
};

type PrimarySidebarItem = {
  id: SpaceId;
  label: string;
  testId: string;
  active: boolean;
  icon: ChromeIconName;
  iconClass: string;
  badgeClass?: string;
  badgeText?: string;
};

const props = defineProps<{
  isSearchOpen?: boolean;
  isVaultSidebarHidden: boolean;
  activeScreen: "notes" | "settings";
  activeSpace: SpaceId;
  entries: Entry[];
  noteTypes: NoteType[];
  currentEntry: Entry | null;
  searchQuery: string;
}>();

const emit = defineEmits<{
  toggleCollapse: [];
  toggleVaultSidebar: [];
  selectSpace: [spaceId: SpaceId];
  searchToggle: [];
  createRootEntry: [];
  openEntry: [entryId: string];
  openSettings: [];
}>();

const primaryItems = computed<PrimarySidebarItem[]>(() => [
  {
    id: "my-space",
    label: "Моё пространство",
    testId: "widget-link-my-space",
    active: props.activeScreen === "notes" && props.activeSpace === "my-space",
    icon: "book",
    iconClass: "badge-icon",
    badgeClass: "person-badge",
    badgeText: "М",
  },
  {
    id: "all-objects",
    label: "Все объекты",
    testId: "widget-link-all-objects",
    active: props.activeScreen === "notes" && props.activeSpace === "all-objects",
    icon: "grid",
    iconClass: "badge-icon",
    badgeClass: "collection-badge",
  },
  {
    id: "all-notes",
    label: "Все заметки",
    testId: "widget-link-all-notes",
    active: props.activeScreen === "notes" && props.activeSpace === "all-notes",
    icon: "book",
    iconClass: "badge-icon",
    badgeClass: "document-badge",
  },
  {
    id: "all-properties",
    label: "Все свойства",
    testId: "widget-link-all-properties",
    active: props.activeScreen === "notes" && props.activeSpace === "all-properties",
    icon: "settings",
    iconClass: "badge-icon",
    badgeClass: "property-badge",
  },
  {
    id: "diary",
    label: "Дневник",
    testId: "widget-link-diary",
    active: props.activeScreen === "notes" && props.activeSpace === "diary",
    icon: "book",
    iconClass: "chrome-icon-wrap",
  },
]);

const recentEntries = computed(() =>
  [...props.entries]
    .filter((entry) => entry.id !== props.currentEntry?.id)
    .sort((entryA, entryB) => entryB.updated_at - entryA.updated_at)
    .slice(0, 8),
);

function getEntryLabel(entry: Entry) {
  const title = entry.title.trim();
  return title || "Без названия";
}

function getNoteType(entry: Entry) {
  if (!entry.type_id) return null;
  return props.noteTypes.find((noteType) => noteType.id === entry.type_id) ?? null;
}

function onPrimaryClick(spaceId: SpaceId) {
  emit("selectSpace", spaceId);
}
</script>

<style scoped>
.main-sidebar-shell {
  display: flex;
  min-height: 0;
  height: 100%;
  flex-direction: column;
  justify-content: space-between;
  padding: 10px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.035), transparent 26%),
    var(--bg-sidebar);
}

.main-sidebar-topbar {
  display: flex;
  min-height: 38px;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 2px 2px 10px;
}

.main-sidebar-topbar-left,
.main-sidebar-topbar-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.main-sidebar-scroll {
  display: flex;
  min-height: 0;
  flex: 1;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
}

.main-sidebar-actions {
  display: grid;
  gap: 8px;
}

.main-sidebar-action-btn,
.main-sidebar-nav-btn,
.main-sidebar-chrome-btn {
  border: 0;
  font: inherit;
}

.main-sidebar-action-btn,
.main-sidebar-nav-btn {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 10px;
  border-radius: 12px;
  padding: 10px 12px;
  color: var(--text-secondary);
  text-align: left;
  transition:
    background var(--transition-fast),
    color var(--transition-fast),
    transform var(--transition-fast);
}

.main-sidebar-action-btn:hover,
.main-sidebar-action-btn.active,
.main-sidebar-nav-btn:hover,
.main-sidebar-nav-btn.active {
  background: var(--bg-sidebar-active);
  color: var(--text-primary);
}

.main-sidebar-action-btn:active,
.main-sidebar-nav-btn:active,
.main-sidebar-chrome-btn:active {
  transform: translateY(1px);
}

.main-sidebar-action-icon,
.main-sidebar-nav-icon {
  display: inline-flex;
  width: 20px;
  height: 20px;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.main-sidebar-nav-icon.badge-icon :deep(.itemBadge) {
  width: 20px;
  height: 20px;
  border-radius: 5px;
}

.main-sidebar-nav-icon.badge-icon :deep(.itemBadge)::before {
  width: 12px;
  height: 12px;
}

.main-sidebar-nav-icon.badge-icon :deep(.property-badge)::before {
  width: 11px;
  height: 11px;
}

.main-sidebar-nav-icon.badge-icon :deep(.person-badge) {
  font-size: 10px;
  font-weight: 700;
}

.main-sidebar-nav-icon.chrome-icon-wrap {
  color: var(--text-secondary);
}

.main-sidebar-nav-btn.active .main-sidebar-nav-icon.chrome-icon-wrap,
.main-sidebar-nav-btn:hover .main-sidebar-nav-icon.chrome-icon-wrap,
.main-sidebar-action-btn.active .main-sidebar-action-icon,
.main-sidebar-action-btn:hover .main-sidebar-action-icon {
  color: var(--text-primary);
}

.main-sidebar-nav-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  font-weight: 500;
}

.main-sidebar-section {
  display: grid;
  gap: 4px;
}

.main-sidebar-section-title {
  padding: 0 12px 6px;
  color: var(--text-tertiary);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.main-sidebar-footer {
  padding-top: 10px;
  border-top: 1px solid var(--border-divider);
}

.main-sidebar-chrome-btn {
  display: inline-flex;
  width: 28px;
  height: 28px;
  align-items: center;
  justify-content: center;
  border-radius: 8px;
  color: var(--text-tertiary);
  transition:
    background var(--transition-fast),
    color var(--transition-fast);
}

.main-sidebar-chrome-btn:hover {
  background: var(--bg-sidebar-hover);
  color: var(--text-primary);
}

.main-sidebar-nav-btn.recent {
  padding-block: 8px;
}

.main-sidebar-nav-btn.footer {
  margin-top: 2px;
}

.platform-mac .main-sidebar-shell {
  padding-top: 14px;
}

.platform-mac .main-sidebar-topbar {
  min-height: 44px;
  padding-top: 2px;
}
</style>

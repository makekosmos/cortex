<template>
  <aside class="widget-sidebar sidebarPage pageWidget">
    <div class="head">
      <div class="side left">
        <button
          v-if="isVaultSidebarHidden"
          class="sidebar-head-icon withBackground"
          title="Показать хранилища"
          type="button"
          @click="emit('toggleVaultSidebar')"
        >
          <span aria-hidden="true" class="anytype-icon toggleVault" />
        </button>
        <button
          class="sidebar-head-icon withBackground"
          data-testid="sidebar-toggle"
          title="Скрыть виджеты"
          type="button"
          @click="emit('toggleCollapse')"
        >
          <span aria-hidden="true" class="anytype-icon toggleWidget" />
        </button>
      </div>

      <div class="side right" style="position: relative">
        <button
          ref="recentlyOpenedBtnRef"
          class="sidebar-head-icon withBackground"
          :class="{ active: isRecentlyOpenedOpen }"
          title="Недавно открытые"
          type="button"
          @click="isRecentlyOpenedOpen = !isRecentlyOpenedOpen"
        >
          <span aria-hidden="true" class="anytype-icon clock" />
        </button>

        <div v-if="isRecentlyOpenedOpen" ref="recentlyOpenedRef" class="recently-opened-menu">
          <div class="recently-opened-section-name"><span>Недавно открытые</span></div>
          <div class="recently-opened-items">
            <button
              v-for="entry in recentlyOpenedEntries"
              :key="entry.id"
              class="recently-opened-item"
              :class="{ active: currentEntry?.id === entry.id }"
              type="button"
              @click="openRecentEntry(entry.id)"
            >
              <span class="recently-opened-icon"
                ><ObjectIcon :entry="entry" :note-types="noteTypes" :size="18"
              /></span>
              <span class="recently-opened-name">{{ getEntryLabel(entry) }}</span>
              <span class="recently-opened-caption">{{ getEntryCaption(entry) }}</span>
            </button>
            <div v-if="recentlyOpenedEntries.length === 0" class="recently-opened-empty">
              Нет недавних записей
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="body widget-sidebar-body">
      <div class="content">
        <section class="section widget-space-card">
          <div class="items">
            <button class="item widget-primary-item" type="button" @click="emit('createRootEntry')">
              <span class="itemIcon"><span aria-hidden="true" class="anytype-icon create" /></span>
              <span class="value">Создать</span>
            </button>
            <button
              class="item widget-primary-item"
              :class="{ active: isSearchOpen || searchQuery }"
              data-testid="widget-link-search"
              type="button"
              @click="emit('searchToggle')"
            >
              <span class="itemIcon"><span aria-hidden="true" class="anytype-icon search" /></span>
              <span class="value">Поиск</span>
            </button>
          </div>
        </section>

        <!-- Закреплённые -->
        <div class="widgetSection" :class="{ isOpen: !collapsedSections.pinned }">
          <div class="nameWrap">
            <div class="name" @click="toggleSection('pinned')">
              <span aria-hidden="true" class="anytype-icon collapseArrow" />Закреплённые
            </div>
            <div class="buttons">
              <button
                class="sidebar-section-icon"
                title="Новая заметка"
                type="button"
                @click="emit('createRootEntry')"
              >
                <span aria-hidden="true" class="anytype-icon plus" />
              </button>
            </div>
          </div>
          <div class="itemsWrap">
            <div class="items">
              <button
                class="item widget-nav-item"
                :class="{ active: activeSpace === 'my-space' }"
                data-testid="widget-link-my-space"
                type="button"
                @click="emit('selectSpace', 'my-space')"
              >
                <span class="itemIcon"><span class="itemBadge person-badge" /></span>
                <span class="value">Моё пространство</span>
              </button>
            </div>
          </div>
        </div>

        <!-- Недавно изменённые -->
        <div class="widgetSection" :class="{ isOpen: !collapsedSections.recent }">
          <div class="nameWrap">
            <div class="name" @click="toggleSection('recent')">
              <span aria-hidden="true" class="anytype-icon collapseArrow" />Недавно изменённые
            </div>
          </div>
          <div class="itemsWrap">
            <div class="items">
              <button
                v-for="entry in recentEntries"
                :key="entry.id"
                class="item widget-nav-item"
                :class="{ active: currentEntry?.id === entry.id }"
                type="button"
                @click="emit('openEntry', entry.id)"
              >
                <span class="itemIcon"
                  ><ObjectIcon :entry="entry" :note-types="noteTypes" :size="18"
                /></span>
                <span class="value">{{ getEntryLabel(entry) }}</span>
              </button>
            </div>
          </div>
        </div>

        <!-- Объекты -->
        <div class="widgetSection isOpen">
          <div class="nameWrap">
            <div class="name" @click="emit('selectSpace', 'all-objects')">
              <span aria-hidden="true" class="anytype-icon collapseArrow" />Объекты
            </div>
          </div>
          <div class="itemsWrap">
            <div class="items">
              <button
                v-for="item in objectItems"
                :key="item.id"
                class="item widget-nav-item"
                :class="{ active: item.active }"
                :data-testid="item.testId"
                type="button"
                @click="emit('selectSpace', item.id as SpaceId)"
              >
                <span class="itemIcon"><span class="itemBadge" :class="item.iconClassName" /></span>
                <span class="value">{{ item.label }}</span>
              </button>
            </div>
          </div>
        </div>

        <!-- Типизированные разделы -->
        <div
          v-for="{ id, noteType, entries: typedEntries } in typedSections"
          :key="noteType.id"
          class="widgetSection"
          :class="{ isOpen: !collapsedSections[id] }"
        >
          <div class="nameWrap">
            <div class="name" @click="toggleSection(id)">
              <span aria-hidden="true" class="anytype-icon collapseArrow" />{{ noteType.name }}
            </div>
          </div>
          <div class="itemsWrap">
            <div class="items">
              <button
                v-for="entry in typedEntries"
                :key="entry.id"
                class="item widget-nav-item"
                :class="{ active: currentEntry?.id === entry.id }"
                type="button"
                @click="emit('openEntry', entry.id)"
              >
                <span class="itemIcon"
                  ><ObjectIcon :entry="entry" :note-types="noteTypes" :size="18"
                /></span>
                <span class="value">{{ getEntryLabel(entry) }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div class="bottom widget-sidebar-bottom">
      <div class="grad" />
      <div class="sides">
        <div class="side left">
          <button
            class="widgetSettings"
            data-testid="open-settings-btn"
            title="Настройки"
            type="button"
            @click="emit('openSettings')"
          >
            <span aria-hidden="true" class="anytype-icon settings" />
          </button>
        </div>
      </div>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, h } from "vue";
import type { SpaceId } from "./types";

const ObjectIcon = (props: { entry: Entry; noteTypes: NoteType[]; size: number }) => {
  const noteType = props.entry.type_id
    ? (props.noteTypes.find((nt) => nt.id === props.entry.type_id) ?? null)
    : null;
  if (props.entry.type_id && noteType) {
    return h(
      "span",
      {
        class: "objectIcon objectIcon-type",
        style: {
          width: props.size + "px",
          height: props.size + "px",
          color: noteType.color ?? undefined,
        },
      },
      noteType.icon ?? "T",
    );
  }
  return h("img", {
    class: "objectIcon objectIcon-page",
    src: "/anytype/icon/object/page.svg",
    alt: "",
    width: props.size,
    height: props.size,
    draggable: false,
  });
};

const props = defineProps<{
  isSearchOpen?: boolean;
  isVaultSidebarHidden: boolean;
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

type SectionId = "pinned" | "recent" | string;

const collapsedSections = ref<Record<string, boolean>>({ pinned: false, recent: false });
const isRecentlyOpenedOpen = ref(false);
const recentlyOpenedRef = ref<HTMLDivElement | null>(null);
const recentlyOpenedBtnRef = ref<HTMLButtonElement | null>(null);

const recentEntries = computed(() =>
  [...props.entries].sort((a, b) => b.updated_at - a.updated_at).slice(0, 6),
);
const recentlyOpenedEntries = computed(() =>
  [...props.entries].sort((a, b) => b.updated_at - a.updated_at).slice(0, 10),
);
const typedSections = computed(() =>
  props.noteTypes
    .map((nt) => ({
      id: `type-${nt.id}`,
      noteType: nt,
      entries: props.entries
        .filter((e) => e.type_id === nt.id)
        .sort((a, b) => b.updated_at - a.updated_at)
        .slice(0, 4),
    }))
    .filter((s) => s.entries.length > 0)
    .slice(0, 4),
);
const objectItems = computed(() => [
  {
    id: "all-objects",
    iconClassName: "collection-badge",
    label: "Все объекты",
    active: props.activeSpace === "all-objects",
    testId: "widget-link-all-objects",
  },
  {
    id: "all-properties",
    iconClassName: "collection-badge",
    label: "Все свойства",
    active: props.activeSpace === "all-properties",
    testId: "widget-link-all-properties",
  },
  {
    id: "all-notes",
    iconClassName: "document-badge",
    label: "Заметки",
    active: props.activeSpace === "all-notes",
    testId: "widget-link-all-notes",
  },
  {
    id: "diary",
    iconClassName: "document-badge",
    label: "Дневник",
    active: props.activeSpace === "diary",
    testId: "widget-link-diary",
  },
]);

function getEntryLabel(entry: Entry) {
  return entry.title.trim() || "Без названия";
}

function getEntryCaption(entry: Entry) {
  if (entry.type_id) {
    const nt = props.noteTypes.find((n) => n.id === entry.type_id);
    if (nt) return nt.name;
  }
  return "Страница";
}

function toggleSection(id: SectionId) {
  collapsedSections.value = { ...collapsedSections.value, [id]: !collapsedSections.value[id] };
}

function openRecentEntry(entryId: string) {
  emit("openEntry", entryId);
  isRecentlyOpenedOpen.value = false;
}

function handleClickOutside(event: MouseEvent) {
  if (
    recentlyOpenedRef.value?.contains(event.target as Node) ||
    recentlyOpenedBtnRef.value?.contains(event.target as Node)
  )
    return;
  isRecentlyOpenedOpen.value = false;
}

function handleEscape(event: KeyboardEvent) {
  if (event.key === "Escape") isRecentlyOpenedOpen.value = false;
}

onMounted(() => {
  window.addEventListener("mousedown", handleClickOutside);
  window.addEventListener("keydown", handleEscape);
});
onUnmounted(() => {
  window.removeEventListener("mousedown", handleClickOutside);
  window.removeEventListener("keydown", handleEscape);
});
</script>

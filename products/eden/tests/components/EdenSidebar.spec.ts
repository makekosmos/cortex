// Component tests: Eden sidebar contract around notes/settings navigation.
//
// This spec intentionally keys off the stable `data-testid` contract that the
// Eden sidebar exposes today: `sidebar-create-entry`, `widget-link-search`,
// `sidebar-open-objects`, `sidebar-header-settings`, and `settings-nav-*`.
//
// The goal is to keep the tests resilient now that EdenSidebar uses the
// SettingsSidebar / SettingsSidebarButton API from @kosmos/visuals.

import { describe, expect, test } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import { defineComponent, shallowRef } from "vue";
import EdenSidebar from "../../src/components/sidebar/EdenSidebar.vue";
import { SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE } from "../../src/lib/systemTypes";

type EdenScreen = "notes" | "settings" | "object-types" | "type-collection";
type SettingsTab = "general" | "trash" | "storage" | "vim" | "spaces";

function makeCustomType(): NoteType {
  return {
    id: "custom-project",
    name: "Проект",
    slug: "project",
    icon: "folder",
    color: "var(--accent)",
    schema_json: JSON.stringify({ fields: [] }),
    header_template_json: JSON.stringify({ kind: "default" }),
    ui_schema_json: JSON.stringify({ collection_name: "Проекты" }),
    created_at: 0,
    updated_at: 0,
  };
}

function makeEntry(
  id: string,
  title: string,
  typeId: string | null = SYSTEM_TYPE_NOTE.id,
  updatedAt = Date.now(),
  body = "Текст заметки для превью",
): Entry {
  return {
    id,
    title,
    content_json: JSON.stringify({ type: "markdown", version: 1, text: body }),
    created_at: updatedAt,
    updated_at: updatedAt,
    folder_id: null,
    type_id: typeId,
    header_layout: "default",
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
  };
}

const EdenSidebarFixture = defineComponent({
  components: { EdenSidebar },
  setup() {
    const hidden = shallowRef(false);
    const searchQuery = shallowRef("");
    const activeScreen = shallowRef<EdenScreen>("notes");
    const activeSettingsTab = shallowRef<SettingsTab>("general");
    const selectedObjectTypeId = shallowRef<string | null>(null);
    const sidebarWidth = shallowRef(280);
    const now = Date.now();
    const recentEntries = Array.from({ length: 120 }, (_, index) =>
      makeEntry(
        `entry-${index + 1}`,
        `Недавняя заметка ${index + 1}`,
        SYSTEM_TYPE_NOTE.id,
        now - index * 24 * 60 * 60 * 1000,
        `Начало текста заметки ${index + 1}. Второе предложение для превью.`,
      ),
    );
    const journalEntry = makeEntry(
      "entry-diary",
      "Дневник",
      SYSTEM_TYPE_JOURNAL_ID,
      now + 1,
      "Запись дневника",
    );
    const currentEntry = shallowRef<Entry | null>(recentEntries[0]);
    const noteTypes = [SYSTEM_TYPE_NOTE, makeCustomType()];

    const createEntryCount = shallowRef(0);
    const toggleSearchCount = shallowRef(0);
    const backCount = shallowRef(0);
    const openObjectTypesCount = shallowRef(0);
    const createObjectTypeCount = shallowRef(0);
    const openEntryId = shallowRef("");
    const lastSettingsTab = shallowRef<SettingsTab>("general");

    function setNotesScreen(): void {
      activeScreen.value = "notes";
      currentEntry.value = recentEntries[0];
      selectedObjectTypeId.value = null;
    }

    return {
      hidden,
      searchQuery,
      activeScreen,
      activeSettingsTab,
      selectedObjectTypeId,
      sidebarWidth,
      currentEntry,
      recentEntries,
      noteTypes,
      createEntryCount,
      toggleSearchCount,
      backCount,
      openObjectTypesCount,
      createObjectTypeCount,
      openEntryId,
      lastSettingsTab,
      setNotesScreen,
      onEntryContextMenu() {
        return undefined;
      },
      onCreateEntry() {
        createEntryCount.value += 1;
      },
      onOpenDiary() {
        currentEntry.value = journalEntry;
        activeScreen.value = "notes";
      },
      onToggleSearch() {
        toggleSearchCount.value += 1;
        searchQuery.value = searchQuery.value ? "" : "eden";
      },
      onOpenEntry(entryId: string) {
        openEntryId.value = entryId;
        setNotesScreen();
      },
      onOpenSettingsTab(tab: SettingsTab) {
        activeScreen.value = "settings";
        activeSettingsTab.value = tab;
        lastSettingsTab.value = tab;
      },
      onOpenObjectTypes() {
        openObjectTypesCount.value += 1;
        activeScreen.value = "object-types";
      },
      onOpenObjectType(noteTypeId: string) {
        selectedObjectTypeId.value = noteTypeId;
        activeScreen.value = "object-types";
      },
      onCreateObjectType() {
        createObjectTypeCount.value += 1;
      },
      onSidebarWidthChange(width: number) {
        sidebarWidth.value = width;
      },
      onBack() {
        backCount.value += 1;
        setNotesScreen();
      },
    };
  },
  template: `
    <div>
      <EdenSidebar
        :hidden="hidden"
        :search-query="searchQuery"
        :recent-entries="recentEntries"
        :note-types="noteTypes"
        :current-entry="currentEntry"
        :active-screen="activeScreen"
        :active-settings-tab="activeSettingsTab"
        :selected-object-type-id="selectedObjectTypeId"
        :sidebar-width="sidebarWidth"
        @create-entry="onCreateEntry"
        @toggle-search="onToggleSearch"
        @open-diary="onOpenDiary"
        @open-entry="onOpenEntry"
        @entry-context-menu="onEntryContextMenu"
        @open-settings-tab="onOpenSettingsTab"
        @open-object-types="onOpenObjectTypes"
        @open-object-type="onOpenObjectType"
        @create-object-type="onCreateObjectType"
        @sidebar-width-change="onSidebarWidthChange"
        @back="onBack"
      />

      <output data-testid="screen">{{ activeScreen }}</output>
      <output data-testid="settings-tab">{{ activeSettingsTab }}</output>
      <output data-testid="create-entry-count">{{ createEntryCount }}</output>
      <output data-testid="toggle-search-count">{{ toggleSearchCount }}</output>
      <output data-testid="back-count">{{ backCount }}</output>
      <output data-testid="open-object-types-count">{{ openObjectTypesCount }}</output>
      <output data-testid="create-object-type-count">{{ createObjectTypeCount }}</output>
      <output data-testid="selected-object-type-id">{{ selectedObjectTypeId ?? "" }}</output>
      <output data-testid="sidebar-width">{{ sidebarWidth }}</output>
      <output data-testid="open-entry-id">{{ openEntryId }}</output>
      <output data-testid="last-settings-tab">{{ lastSettingsTab }}</output>
    </div>
  `,
});

describe("EdenSidebar contract", () => {
  test("notes screen exposes create/search/settings actions and opens settings", async () => {
    const screen = render(EdenSidebarFixture);

    await expect.element(screen.getByTestId("sidebar-header")).toBeInTheDocument();
    expect(document.querySelector('[data-testid="sidebar-main-scroll"]')).toBeNull();
    await expect.element(screen.getByTestId("sidebar-recent-virtual-list")).toBeInTheDocument();
    await expect.element(screen.getByText("В этом месяце")).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent("Недавние");
    await expect.element(screen.getByTestId("eden-sidebar-resize-handle")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-create-entry")).toBeInTheDocument();
    await expect.element(screen.getByTestId("widget-link-search")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-open-objects")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-header-settings")).toBeInTheDocument();
    await expect.element(screen.getByTestId("recent-entry-entry-1")).toBeInTheDocument();
    await expect
      .element(screen.getByTestId("recent-entry-entry-1"))
      .toHaveTextContent("Недавняя заметка 1");
    await expect.element(screen.getByTestId("recent-entry-entry-1")).toHaveTextContent("Заметки");
    await expect.element(screen.getByTestId("note-type-custom-project")).not.toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-header")).not.toHaveTextContent("Eden");

    await userEvent.click(screen.getByTestId("sidebar-create-entry"));
    await expect.element(screen.getByTestId("create-entry-count")).toHaveTextContent("1");

    await userEvent.click(screen.getByTestId("sidebar-open-diary"));
    await expect.element(screen.getByTestId("screen")).toHaveTextContent("notes");
    await expect
      .element(screen.getByTestId("sidebar-open-diary"))
      .not.toHaveAttribute("data-active", "true");

    await userEvent.click(screen.getByTestId("widget-link-search"));
    await expect.element(screen.getByTestId("toggle-search-count")).toHaveTextContent("1");

    await userEvent.click(screen.getByTestId("sidebar-header-settings"));
    await expect.element(screen.getByTestId("screen")).toHaveTextContent("settings");
    await expect.element(screen.getByTestId("settings-nav-back")).toBeInTheDocument();
    await expect.element(screen.getByTestId("settings-nav-general")).toBeInTheDocument();
    await expect.element(screen.getByTestId("settings-nav-trash")).toBeInTheDocument();
    await expect.element(screen.getByTestId("settings-nav-vim")).toBeInTheDocument();
    await expect.element(screen.getByTestId("settings-nav-object-types")).toBeInTheDocument();

    await userEvent.click(screen.getByTestId("settings-nav-vim"));
    await expect.element(screen.getByTestId("settings-tab")).toHaveTextContent("vim");
    await expect.element(screen.getByTestId("last-settings-tab")).toHaveTextContent("vim");
  });

  test("objects button opens all object types in a modal", async () => {
    const screen = render(EdenSidebarFixture);

    await userEvent.click(screen.getByTestId("sidebar-open-objects"));
    await expect.element(screen.getByTestId("sidebar-objects-modal")).toBeInTheDocument();
    await expect
      .element(screen.getByTestId(`objects-modal-note-type-${SYSTEM_TYPE_NOTE.id}`))
      .toBeInTheDocument();
    await expect
      .element(screen.getByTestId("objects-modal-note-type-custom-project"))
      .toBeInTheDocument();

    await userEvent.click(screen.getByTestId("objects-modal-note-type-custom-project"));
    await expect
      .element(screen.getByTestId("selected-object-type-id"))
      .toHaveTextContent("custom-project");
    await expect.element(screen.getByTestId("screen")).toHaveTextContent("object-types");
    await expect
      .element(screen.getByTestId("custom-type-custom-project"))
      .toHaveAttribute("data-active", "true");
    await expect.element(screen.getByTestId("sidebar-objects-modal")).not.toBeInTheDocument();
  });

  test("recent entries are virtualized and keep updated order while scrolling", async () => {
    const screen = render(EdenSidebarFixture);

    await expect.element(screen.getByTestId("recent-entry-entry-1")).toBeInTheDocument();
    await expect.element(screen.getByTestId("recent-entry-entry-120")).not.toBeInTheDocument();

    const recentList = document.querySelector<HTMLElement>(
      '[data-testid="sidebar-recent-virtual-list"]',
    );
    if (!recentList) throw new Error("sidebar-recent-virtual-list not found");
    recentList.scrollTop = recentList.scrollHeight;
    recentList.dispatchEvent(new Event("scroll"));
    await new Promise((resolve) => requestAnimationFrame(resolve));

    await expect.element(screen.getByTestId("recent-entry-entry-120")).toBeInTheDocument();
    await expect.element(screen.getByTestId("recent-entry-entry-1")).not.toBeInTheDocument();
  });

  test("settings navigation returns to notes and object types keeps its own back action", async () => {
    const screen = render(EdenSidebarFixture);

    await userEvent.click(screen.getByTestId("sidebar-header-settings"));
    await userEvent.click(screen.getByTestId("settings-nav-object-types"));

    await expect.element(screen.getByTestId("screen")).toHaveTextContent("object-types");
    await expect.element(screen.getByTestId("settings-nav-back")).not.toBeInTheDocument();
    await expect.element(screen.getByTestId("object-types-nav-back")).toBeInTheDocument();
    await expect.element(screen.getByTestId("open-object-types-count")).toHaveTextContent("1");

    await userEvent.click(screen.getByTestId("object-types-nav-back"));
    await expect.element(screen.getByTestId("screen")).toHaveTextContent("notes");
    await expect.element(screen.getByTestId("sidebar-header-settings")).toBeInTheDocument();
    await expect.element(screen.getByTestId("back-count")).toHaveTextContent("1");
  });
});

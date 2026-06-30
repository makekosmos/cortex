// Component tests: Eden sidebar contract around notes navigation and settings open action.
//
// The sidebar keeps the main note window in note mode and asks the shell to open
// Eden settings in a separate window.

import { describe, expect, test } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import { defineComponent, shallowRef } from "vue";
import EdenSidebar from "../../src/components/sidebar/EdenSidebar.vue";
import { SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE } from "../../src/lib/systemTypes";

type EdenScreen = "notes" | "settings" | "type-collection";

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
    const noteTypes = [SYSTEM_TYPE_NOTE];

    const createEntryCount = shallowRef(0);
    const toggleSearchCount = shallowRef(0);
    const openEntryId = shallowRef("");
    const openSettingsCount = shallowRef(0);
    const openObjectTypeId = shallowRef("");

    function setNotesScreen(): void {
      activeScreen.value = "notes";
      currentEntry.value = recentEntries[0];
    }

    return {
      hidden,
      searchQuery,
      activeScreen,
      sidebarWidth,
      currentEntry,
      recentEntries,
      noteTypes,
      createEntryCount,
      toggleSearchCount,
      openEntryId,
      openSettingsCount,
      openObjectTypeId,
      setNotesScreen,
      onEntryContextMenu() {
        return;
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
      onOpenSettings() {
        openSettingsCount.value += 1;
      },
      onOpenObjectType(noteTypeId: string) {
        openObjectTypeId.value = noteTypeId;
        activeScreen.value = "type-collection";
      },
      onSidebarWidthChange(width: number) {
        sidebarWidth.value = width;
      },
    };
  },
  template: `
    <div style="height: 760px">
      <EdenSidebar
        :hidden="hidden"
        :search-query="searchQuery"
        :recent-entries="recentEntries"
        :note-types="noteTypes"
        :current-entry="currentEntry"
        :active-screen="activeScreen"
        :selected-object-type-id="openObjectTypeId"
        :sidebar-width="sidebarWidth"
        :resizable="true"
        @create-entry="onCreateEntry"
        @toggle-search="onToggleSearch"
        @open-diary="onOpenDiary"
        @open-entry="onOpenEntry"
        @entry-context-menu="onEntryContextMenu"
        @open-settings="onOpenSettings"
        @open-object-type="onOpenObjectType"
        @sidebar-width-change="onSidebarWidthChange"
      />

      <output data-testid="screen">{{ activeScreen }}</output>
      <output data-testid="create-entry-count">{{ createEntryCount }}</output>
      <output data-testid="toggle-search-count">{{ toggleSearchCount }}</output>
      <output data-testid="sidebar-width">{{ sidebarWidth }}</output>
      <output data-testid="open-entry-id">{{ openEntryId }}</output>
      <output data-testid="open-settings-count">{{ openSettingsCount }}</output>
      <output data-testid="open-object-type-id">{{ openObjectTypeId }}</output>
    </div>
  `,
});

describe("EdenSidebar contract", () => {
  test("notes screen exposes create/search/settings actions and opens settings window", async () => {
    const screen = render(EdenSidebarFixture);

    await expect.element(screen.getByTestId("sidebar-header")).toBeInTheDocument();
    expect(document.querySelector('[data-testid="sidebar-main-scroll"]')).toBeNull();
    await expect.element(screen.getByTestId("sidebar-recent-virtual-list")).toBeInTheDocument();
    await expect.element(screen.getByText("В этом месяце")).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent("Недавние");
    await expect.element(screen.getByTestId("eden-sidebar-resize-handle")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-create-entry")).toBeInTheDocument();
    await expect.element(screen.getByTestId("widget-link-search")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-header-settings")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-open-objects")).toBeInTheDocument();
    await expect.element(screen.getByTestId("recent-entry-entry-1")).toBeInTheDocument();
    await expect
      .element(screen.getByTestId("recent-entry-entry-1"))
      .toHaveTextContent("Недавняя заметка 1");
    await expect.element(screen.getByTestId("recent-entry-entry-1")).toHaveTextContent("Заметки");
    await expect.element(screen.getByTestId("note-type-custom-project")).not.toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-header")).not.toHaveTextContent("Eden");

    await userEvent.click(screen.getByTestId("sidebar-create-entry"));
    await expect.element(screen.getByTestId("create-entry-count")).toHaveTextContent("1");

    await userEvent.click(screen.getByTestId("sidebar-header-settings"));
    await expect.element(screen.getByTestId("screen")).toHaveTextContent("notes");
    await expect.element(screen.getByTestId("open-settings-count")).toHaveTextContent("1");
    await expect.element(screen.getByTestId("sidebar-titlebar-toggle")).toBeInTheDocument();
    await expect.element(screen.getByTestId("widget-link-search")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-create-entry")).toBeInTheDocument();
    await expect.element(screen.getByTestId("eden-sidebar-resize-handle")).toBeInTheDocument();

    await userEvent.click(screen.getByTestId("sidebar-open-diary"));
    await expect.element(screen.getByTestId("screen")).toHaveTextContent("notes");
    await expect
      .element(screen.getByTestId("sidebar-open-diary"))
      .toHaveAttribute("data-active", "true");

    await userEvent.click(screen.getByTestId("widget-link-search"));
    await expect.element(screen.getByTestId("toggle-search-count")).toHaveTextContent("1");
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
  });

  test("objects button opens object type picker", async () => {
    const screen = render(EdenSidebarFixture);

    await userEvent.click(screen.getByTestId("sidebar-open-objects"));
    await expect.element(screen.getByTestId("sidebar-objects-modal")).toBeInTheDocument();
    const pickerNoteType = document.querySelector<HTMLButtonElement>(
      `[data-testid="objects-modal-object-type-${SYSTEM_TYPE_NOTE.id}"]`,
    );
    expect(pickerNoteType).not.toBeNull();
    pickerNoteType?.click();

    await expect.element(screen.getByTestId("sidebar-objects-modal")).not.toBeInTheDocument();
    await expect
      .element(screen.getByTestId("open-object-type-id"))
      .toHaveTextContent(SYSTEM_TYPE_NOTE.id);
    await expect.element(screen.getByTestId("screen")).toHaveTextContent("type-collection");
  });

  test("settings button keeps note navigation visible and only emits the open request", async () => {
    const screen = render(EdenSidebarFixture);

    await userEvent.click(screen.getByTestId("sidebar-header-settings"));

    await expect.element(screen.getByTestId("screen")).toHaveTextContent("notes");
    await expect.element(screen.getByTestId("open-settings-count")).toHaveTextContent("1");
    await expect.element(screen.getByTestId("sidebar-header-settings")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-open-diary")).toBeInTheDocument();
    await expect.element(screen.getByTestId("widget-link-search")).toBeInTheDocument();
    await expect.element(screen.getByTestId("sidebar-create-entry")).toBeInTheDocument();
  });
});

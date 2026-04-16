import { defineStore } from "pinia";

import { ref, computed } from "vue";

import { v4 as uuidv4 } from "uuid";

import { normalizeSlug } from "@/lib/typedNotes";

import { SYSTEM_TYPES, isSystemType } from "@/lib/systemTypes";

import type { SpaceId } from "@/components/sidebar/types";

import type { SortMode } from "@/components/sidebar/types";

import { useLayoutStore } from "./layout";

type ActiveScreen = "notes" | "settings";

const MY_SPACE_TITLE = "Мое пространство";

interface QueuedSaveRequest {
  entry: Entry;

  waiters: Array<{
    resolve: (result: SaveEntryResult | null) => void;

    reject: (error: unknown) => void;
  }>;
}

interface EntrySaveCoordinator {
  inFlight: boolean;

  queued: QueuedSaveRequest | null;
}

export const useEdenStore = defineStore("eden", () => {
  const entries = ref<Entry[]>([]);

  const noteTypes = ref<NoteType[]>([]);

  const currentEntry = ref<Entry | null>(null);

  const vaultPath = ref<string | null>(null);

  const recentVaultPaths = ref<string[]>([]);

  const isInitializing = ref(true);

  const activeScreen = ref<ActiveScreen>("notes");

  const activeSpace = ref<SpaceId>("my-space");

  const codeToolsSettings = ref<CodeToolsSettings | null>(null);

  const sortMode = ref<SortMode>("updated_at");

  // Non-reactive save coordination state (mutable internal mechanism)

  const latestSaveTimestamps: Record<string, number> = {};

  const saveCoordinators: Record<string, EntrySaveCoordinator> = {};

  const vaultName = computed(() => {
    if (!vaultPath.value) return null;

    return vaultPath.value.split("/").pop() ?? vaultPath.value;
  });

  const uniqueDraftTitle = computed(() => {
    const baseTitle = "Новая заметка";

    const siblingTitles = new Set(entries.value.map((e) => e.title));

    if (!siblingTitles.has(baseTitle)) return baseTitle;

    let suffix = 2;

    while (siblingTitles.has(`${baseTitle} ${suffix}`)) suffix++;

    return `${baseTitle} ${suffix}`;
  });

  async function refreshData() {
    if (!window.api) return;

    const [entriesData, noteTypesData] = await Promise.all([
      window.api.listEntries(),

      window.api.listNoteTypes(),
    ]);

    entries.value = entriesData;

    noteTypes.value = [...SYSTEM_TYPES, ...noteTypesData];

    if (currentEntry.value) {
      currentEntry.value =
        entriesData.find((e) => e.id === currentEntry.value!.id) ??
        currentEntry.value;
    }
  }

  async function initApp() {
    if (!window.api) return;

    const [path, recentPaths, settings, sidebarConfig] = await Promise.all([
      window.api.getVaultPath(),

      window.api.getRecentVaultPaths(),

      window.api.getCodeToolsSettings(),

      window.api.getSidebarConfig(),
    ]);

    vaultPath.value = path;

    recentVaultPaths.value = recentPaths;

    codeToolsSettings.value = settings;

    const layout = useLayoutStore();

    layout.widgetSidebarWidth = sidebarConfig.widget.width;

    layout.widgetSidebarHidden = sidebarConfig.widget.hidden;

    if (path) {
      const [entriesData, noteTypesData] = await Promise.all([
        window.api.listEntries(),

        window.api.listNoteTypes(),
      ]);

      entries.value = entriesData;

      noteTypes.value = [...SYSTEM_TYPES, ...noteTypesData];

      const existingMySpace =
        entriesData.find((e) => e.title.trim() === MY_SPACE_TITLE) ?? null;

      if (existingMySpace) {
        currentEntry.value = existingMySpace;
      } else {
        const mySpaceEntry: Entry = {
          id: uuidv4(),

          title: MY_SPACE_TITLE,

          content_json: JSON.stringify({
            type: "doc",
            content: [{ type: "paragraph" }],
          }),

          created_at: Date.now(),

          updated_at: Date.now(),

          folder_id: null,

          type_id: null,

          header_layout: null,

          header_props_json: "{}",

          schema_version: 1,

          deleted_at: null,
        };

        entries.value = [mySpaceEntry, ...entries.value];

        void window.api.saveEntry(mySpaceEntry);

        currentEntry.value = mySpaceEntry;
      }
    }

    isInitializing.value = false;
  }

  function createEntry(title: string): Entry {
    const newEntry: Entry = {
      id: uuidv4(),

      title,

      content_json: JSON.stringify({
        type: "doc",
        content: [{ type: "paragraph" }],
      }),

      created_at: Date.now(),

      updated_at: Date.now(),

      folder_id: null,

      type_id: null,

      header_layout: null,

      header_props_json: "{}",

      schema_version: 1,

      deleted_at: null,
    };

    entries.value = [newEntry, ...entries.value];

    if (window.api) {
      void window.api.saveEntry(newEntry).then((result) => {
        if (!result.ok) {
          entries.value = entries.value.filter((e) => e.id !== newEntry.id);

          if (currentEntry.value?.id === newEntry.id) currentEntry.value = null;
        }
      });
    }

    return newEntry;
  }

  function findMySpaceEntry(): Entry | null {
    return entries.value.find((e) => e.title.trim() === MY_SPACE_TITLE) ?? null;
  }

  async function openMySpace() {
    activeScreen.value = "notes";

    activeSpace.value = "my-space";

    const existing = findMySpaceEntry();

    if (existing) {
      currentEntry.value = existing;

      return;
    }

    currentEntry.value = createEntry(MY_SPACE_TITLE);
  }

  async function createNewEntry() {
    activeScreen.value = "notes";

    const newEntry = createEntry(uniqueDraftTitle.value);

    currentEntry.value = newEntry;
  }

  async function selectFolder() {
    if (!window.api) return;

    const path = await window.api.selectFolder();

    if (!path) return;

    await window.api.setVaultPath(path);

    vaultPath.value = path;

    recentVaultPaths.value = await window.api.getRecentVaultPaths();

    currentEntry.value = null;

    activeSpace.value = "my-space";

    await refreshData();
  }

  async function selectVaultPath(nextPath: string) {
    if (!window.api || !nextPath || nextPath === vaultPath.value) return;

    await window.api.setVaultPath(nextPath);

    vaultPath.value = nextPath;

    recentVaultPaths.value = await window.api.getRecentVaultPaths();

    currentEntry.value = null;

    activeSpace.value = "my-space";

    activeScreen.value = "notes";

    await refreshData();
  }

  async function navigateTo(entryId: string) {
    if (!window.api) return;

    const entry = await window.api.loadEntry(entryId);

    if (entry) {
      activeScreen.value = "notes";

      currentEntry.value = entry;
    }
  }

  async function updateCodeToolsSettings(patch: Partial<CodeToolsSettings>) {
    if (!window.api) return;

    const next = await window.api.updateCodeToolsSettings(patch);

    codeToolsSettings.value = next;
  }

  async function saveNoteType(
    draft: Omit<NoteType, "id" | "created_at" | "updated_at" | "slug"> & {
      id?: string;

      slug?: string;
    },
  ): Promise<SaveNoteTypeResult> {
    if (!window.api)
      return { ok: false, reason: "invalid_definition", message: "No API" };

    const now = Date.now();

    const noteType: NoteType = {
      id: draft.id ?? uuidv4(),

      name: draft.name,

      slug: normalizeSlug(draft.slug || draft.name),

      icon: draft.icon,

      color: draft.color,

      schema_json: draft.schema_json,

      header_template_json: draft.header_template_json,

      created_at: draft.id
        ? (noteTypes.value.find((t) => t.id === draft.id)?.created_at ?? now)
        : now,

      updated_at: now,
    };

    const result = await window.api.saveNoteType(noteType);

    if (result.ok) await refreshData();

    return result;
  }

  async function deleteNoteType(noteTypeId: string) {
    if (!window.api || isSystemType(noteTypeId)) return;

    await window.api.deleteNoteType(noteTypeId);

    await refreshData();
  }

  async function handleSave(entry: Entry): Promise<SaveEntryResult | null> {
    if (!window.api) return null;

    const persistEntry = async (
      entryToPersist: Entry,
    ): Promise<SaveEntryResult | null> => {
      latestSaveTimestamps[entryToPersist.id] = entryToPersist.updated_at;

      const result = await window.api.saveEntry(entryToPersist);

      if (!result.ok) return result;

      if (latestSaveTimestamps[entryToPersist.id] !== entryToPersist.updated_at)
        return result;

      const idx = entries.value.findIndex((e) => e.id === entryToPersist.id);

      if (idx >= 0) {
        entries.value[idx] = entryToPersist;
      } else {
        entries.value = [entryToPersist, ...entries.value];
      }

      if (currentEntry.value?.id === entryToPersist.id) {
        currentEntry.value = entryToPersist;
      }

      return result;
    };

    const coordinator = saveCoordinators[entry.id] ?? {
      inFlight: false,
      queued: null,
    };

    saveCoordinators[entry.id] = coordinator;

    const runSaveLoop = async (
      nextEntry: Entry,
    ): Promise<SaveEntryResult | null> => {
      coordinator.inFlight = true;

      try {
        const result = await persistEntry(nextEntry);

        const queued = coordinator.queued;

        if (!queued) {
          coordinator.inFlight = false;

          return result;
        }

        coordinator.queued = null;

        const queuedResult = await runSaveLoop(queued.entry);

        queued.waiters.forEach((w) => w.resolve(queuedResult));

        return queuedResult;
      } catch (error) {
        const queued = coordinator.queued;

        coordinator.queued = null;

        coordinator.inFlight = false;

        if (queued) queued.waiters.forEach((w) => w.reject(error));

        throw error;
      } finally {
        if (!coordinator.queued) {
          coordinator.inFlight = false;

          delete saveCoordinators[nextEntry.id];
        }
      }
    };

    if (!coordinator.inFlight) return runSaveLoop(entry);

    return new Promise<SaveEntryResult | null>((resolve, reject) => {
      if (coordinator.queued) {
        coordinator.queued.entry = entry;

        coordinator.queued.waiters.push({ resolve, reject });

        return;
      }

      coordinator.queued = { entry, waiters: [{ resolve, reject }] };
    });
  }

  return {
    entries,

    noteTypes,

    currentEntry,

    vaultPath,

    recentVaultPaths,

    isInitializing,

    activeScreen,

    activeSpace,

    codeToolsSettings,

    sortMode,

    vaultName,

    uniqueDraftTitle,

    refreshData,

    initApp,

    createEntry,

    openMySpace,

    createNewEntry,

    selectFolder,

    selectVaultPath,

    navigateTo,

    updateCodeToolsSettings,

    saveNoteType,

    deleteNoteType,

    handleSave,
  };
});

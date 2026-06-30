import type { Ref } from "vue";

import type { SpaceId } from "@/components/sidebar/types";

import { useLayoutStore } from "./layout";
import { startEdenLiveRefreshSubscription } from "./edenLiveRefreshSubscription";
import {
  mergeEntriesById,
  mergeNoteTypesWithSystem,
  pruneTransientSaveState,
  readLastVisitedEntryId,
  type ActiveScreen,
  type EntrySaveCoordinator,
} from "./edenStoreHelpers";

interface EdenStoreDataActionState {
  activeScreen: Ref<ActiveScreen>;
  activeSpace: Ref<SpaceId>;
  currentEntry: Ref<Entry | null>;
  dirtyEntryId: Ref<string | null>;
  entries: Ref<Entry[]>;
  entriesLoaded: Ref<boolean>;
  isCurrentEntryDirty: Ref<boolean>;
  isHydratingVault: Ref<boolean>;
  isInitializing: Ref<boolean>;
  latestSaveTimestamps: Map<string, number>;
  noteTypes: Ref<NoteType[]>;
  recentVaultPaths: Ref<string[]>;
  saveCoordinators: Record<string, EntrySaveCoordinator>;
  vaultPath: Ref<string | null>;
  ensureEntryCmSafe(entry: Entry): Promise<Entry>;
  upsertEntryBaseline(entry: Entry): void;
}

export function createEdenStoreDataActions(state: EdenStoreDataActionState) {
  async function refreshData() {
    if (!window.api) return;

    const [entriesData, noteTypesData] = await Promise.all([
      window.api.listEntries(),
      window.api.listNoteTypes(),
    ]);

    state.noteTypes.value = mergeNoteTypesWithSystem(noteTypesData);
    const collectionEntries = await window.api.ensureCollectionObjects(state.noteTypes.value);

    if (state.currentEntry.value) {
      const currentId = state.currentEntry.value.id;
      const shouldPreserveLocalBody =
        state.isCurrentEntryDirty.value ||
        state.dirtyEntryId.value === currentId ||
        Boolean(state.saveCoordinators[currentId]);
      const refreshed =
        entriesData.find((entry) => entry.id === currentId) ?? state.currentEntry.value;
      const fullEntry = shouldPreserveLocalBody
        ? null
        : ((await window.api.loadEntry(currentId)) ?? null);
      const safeCurrentEntry = await state.ensureEntryCmSafe({
        ...(fullEntry ?? refreshed),
        content_json: shouldPreserveLocalBody
          ? state.currentEntry.value.content_json
          : (fullEntry?.content_json ?? refreshed.content_json),
        content_loaded: shouldPreserveLocalBody
          ? (state.currentEntry.value.content_loaded ?? true)
          : Boolean(fullEntry) || refreshed.content_loaded === true,
      });
      state.currentEntry.value = safeCurrentEntry;
      const idx = entriesData.findIndex((entry) => entry.id === safeCurrentEntry.id);
      if (idx >= 0) entriesData[idx] = safeCurrentEntry;
    }

    const nextEntries = mergeEntriesById(entriesData, collectionEntries);
    state.entries.value = nextEntries;
    state.entriesLoaded.value = true;
    pruneTransientSaveState(state.latestSaveTimestamps, state.saveCoordinators, nextEntries);
  }

  async function hydrateVaultData() {
    if (!window.api || !state.vaultPath.value) return;

    const startingEntryId = state.currentEntry.value?.id ?? null;
    state.isHydratingVault.value = true;

    try {
      const [entriesData, noteTypesData] = await Promise.all([
        window.api.listEntries(),
        window.api.listNoteTypes(),
      ]);

      state.noteTypes.value = mergeNoteTypesWithSystem(noteTypesData);
      const collectionEntries = await window.api.ensureCollectionObjects(state.noteTypes.value);
      const nextEntries = mergeEntriesById(entriesData, collectionEntries);
      state.entries.value = nextEntries;
      state.entriesLoaded.value = true;
      pruneTransientSaveState(state.latestSaveTimestamps, state.saveCoordinators, nextEntries);
      const lastEntryId = readLastVisitedEntryId();
      const lastEntry =
        nextEntries.find((entry) => entry.id === lastEntryId && entry.deleted_at === null) ?? null;
      const loadedLastEntry = lastEntry ? await window.api.loadEntry(lastEntry.id) : null;
      if (
        (state.currentEntry.value?.id ?? null) !== startingEntryId ||
        state.isCurrentEntryDirty.value
      ) {
        return;
      }
      state.currentEntry.value = loadedLastEntry
        ? await state.ensureEntryCmSafe(loadedLastEntry)
        : null;
    } finally {
      state.isHydratingVault.value = false;
    }
  }

  async function initApp() {
    if (!window.api) return;

    startEdenLiveRefreshSubscription({
      entries: state.entries,
      currentEntry: state.currentEntry,
      isCurrentEntryDirty: state.isCurrentEntryDirty,
      upsertEntryBaseline: state.upsertEntryBaseline,
    });

    const [path, recentPaths, sidebarConfig] = await Promise.all([
      window.api.getVaultPath(),
      window.api.getRecentVaultPaths(),
      window.api.getSidebarConfig(),
    ]);

    state.vaultPath.value = path;
    state.recentVaultPaths.value = recentPaths;
    const layout = useLayoutStore();
    layout.widgetSidebarHidden = sidebarConfig.widget.hidden;
    layout.widgetSidebarWidth = sidebarConfig.widget.width;
    state.isInitializing.value = false;

    if (path) {
      void hydrateVaultData();
    }
  }

  async function selectFolder() {
    if (!window.api) return;

    const path = await window.api.selectFolder();
    if (!path) return;

    await window.api.setVaultPath(path);
    state.vaultPath.value = path;
    state.recentVaultPaths.value = await window.api.getRecentVaultPaths();
    state.currentEntry.value = null;
    state.entriesLoaded.value = false;
    state.activeSpace.value = "diary";
    await refreshData();
  }

  async function selectVaultPath(nextPath: string) {
    if (!window.api || !nextPath || nextPath === state.vaultPath.value) return;

    await window.api.setVaultPath(nextPath);
    state.vaultPath.value = nextPath;
    state.recentVaultPaths.value = await window.api.getRecentVaultPaths();
    state.currentEntry.value = null;
    state.entriesLoaded.value = false;
    state.activeSpace.value = "diary";
    state.activeScreen.value = "notes";
    await refreshData();
  }

  return {
    refreshData,
    hydrateVaultData,
    initApp,
    selectFolder,
    selectVaultPath,
  };
}

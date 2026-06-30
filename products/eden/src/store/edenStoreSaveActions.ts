import type { Ref } from "vue";

import type { EntrySaveCoordinator } from "./edenStoreHelpers";

interface EdenStoreSaveActionState {
  currentEntry: Ref<Entry | null>;
  entries: Ref<Entry[]>;
  isCurrentEntryDirty: Ref<boolean>;
  dirtyEntryId: Ref<string | null>;
  latestSaveTimestamps: Map<string, number>;
  saveCoordinators: Record<string, EntrySaveCoordinator>;
  markLatestLocalEntry(entry: Entry): void;
}

export function createEdenStoreSaveActions(state: EdenStoreSaveActionState) {
  async function handleSave(entry: Entry): Promise<SaveEntryResult | null> {
    if (!window.api) return null;

    const persistEntry = async (entryToPersist: Entry): Promise<SaveEntryResult | null> => {
      state.markLatestLocalEntry(entryToPersist);

      const result = await window.api.saveEntry(entryToPersist);

      if (!result.ok) return result;

      if (state.latestSaveTimestamps.get(entryToPersist.id) !== entryToPersist.updated_at) {
        return result;
      }

      const idx = state.entries.value.findIndex((e) => e.id === entryToPersist.id);

      if (idx >= 0) {
        state.entries.value[idx] = entryToPersist;
      } else {
        state.entries.value = [entryToPersist, ...state.entries.value];
      }

      if (state.currentEntry.value?.id === entryToPersist.id) {
        state.currentEntry.value = entryToPersist;
        state.isCurrentEntryDirty.value = false;
      }
      if (state.dirtyEntryId.value === entryToPersist.id) {
        state.dirtyEntryId.value = null;
      }

      return result;
    };

    const coordinator = state.saveCoordinators[entry.id] ?? {
      inFlight: false,
      queued: null,
    };

    state.saveCoordinators[entry.id] = coordinator;

    const runSaveLoop = async (nextEntry: Entry): Promise<SaveEntryResult | null> => {
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
        queued.waiters.forEach((waiter) => waiter.resolve(queuedResult));
        return queuedResult;
      } catch (error) {
        const queued = coordinator.queued;

        coordinator.queued = null;
        coordinator.inFlight = false;

        if (queued) queued.waiters.forEach((waiter) => waiter.reject(error));
        throw error;
      } finally {
        if (!coordinator.queued) {
          coordinator.inFlight = false;
          delete state.saveCoordinators[nextEntry.id];

          if (state.latestSaveTimestamps.get(nextEntry.id) === nextEntry.updated_at) {
            state.latestSaveTimestamps.delete(nextEntry.id);
          }
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

  return { handleSave };
}

import type { Ref } from "vue";

import { hasUserVisibleEntryChanges } from "./entryChanges";

interface EdenStoreDraftActionState {
  currentEntry: Ref<Entry | null>;
  entries: Ref<Entry[]>;
  isCurrentEntryDirty: Ref<boolean>;
  dirtyEntryId: Ref<string | null>;
  noteTypes: Ref<NoteType[]>;
  markLatestLocalEntry(entry: Entry): void;
}

export function createEdenStoreDraftActions(state: EdenStoreDraftActionState) {
  function updateEntryDraft(entry: Entry) {
    state.markLatestLocalEntry(entry);

    const idx = state.entries.value.findIndex((candidate) => candidate.id === entry.id);

    if (idx >= 0) {
      if (!hasUserVisibleEntryChanges(entry, state.entries.value[idx], state.noteTypes.value))
        return;
      state.entries.value[idx] = entry;
    } else {
      state.entries.value = [entry, ...state.entries.value];
    }

    if (state.currentEntry.value?.id === entry.id) {
      state.currentEntry.value = entry;
      state.dirtyEntryId.value = entry.id;
      state.isCurrentEntryDirty.value = true;
    }
  }

  return { updateEntryDraft };
}

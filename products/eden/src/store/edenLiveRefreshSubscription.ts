import type { Ref } from "vue";
import { onScopeDispose } from "vue";
import { edenApi } from "@/lib/edenApi";
import { isOlderRemoteEntry, shouldApplyRemoteEntry } from "./liveRefresh";

export function startEdenLiveRefreshSubscription(deps: {
  entries: Ref<Entry[]>;
  currentEntry: Ref<Entry | null>;
  isCurrentEntryDirty: Ref<boolean>;
  upsertEntryBaseline: (entry: Entry) => void;
}): void {
  const { entries, currentEntry, isCurrentEntryDirty, upsertEntryBaseline } = deps;
  const unsubscribe = edenApi.subscribeObjectChanges(async (payload) => {
    if (payload.event === "object_deleted") {
      entries.value = entries.value.filter((entry) => entry.id !== payload.id);
      if (currentEntry.value?.id === payload.id) {
        currentEntry.value = null;
        isCurrentEntryDirty.value = false;
      }
      return;
    }

    const existingIdx = entries.value.findIndex((entry) => entry.id === payload.id);
    if (existingIdx >= 0) {
      void (async () => {
        if (!window.api) return;
        let fresh: Entry | undefined;
        try {
          fresh = await window.api.loadEntry(payload.id);
        } catch (err) {
          console.warn("[eden] live-refresh: loadEntry (list update) failed:", err);
          return;
        }
        if (!fresh) return;

        const idx = entries.value.findIndex((entry) => entry.id === payload.id);
        if (idx >= 0 && isOlderRemoteEntry(fresh, entries.value[idx]!)) return;

        if (idx >= 0) {
          entries.value[idx] = fresh;
        }

        if (currentEntry.value?.id !== payload.id) return;
        if (isOlderRemoteEntry(fresh, currentEntry.value)) return;
        const decision = shouldApplyRemoteEntry({
          fresh,
          currentContentJson: currentEntry.value.content_json,
          isEditorDirty: isCurrentEntryDirty.value,
        });
        if (decision !== "apply") return;
        upsertEntryBaseline(fresh);
        currentEntry.value = fresh;
      })();
      return;
    }

    void (async () => {
      if (!window.api) return;
      let listable: Entry | undefined;
      try {
        listable = await edenApi.loadListableEntry(payload.id, payload.typeId);
      } catch (err) {
        console.warn("[eden] live-refresh: loadListableEntry failed:", err);
        return;
      }
      if (!listable) return;
      entries.value = [listable, ...entries.value].sort((a, b) => b.updated_at - a.updated_at);
    })();
  });

  onScopeDispose(() => {
    unsubscribe();
  });
}

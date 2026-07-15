import { effectScope, ref } from "vue";
import { afterEach, describe, expect, test, vi } from "vitest";

import { edenApi } from "../../src/lib/edenApi";
import { SYSTEM_TYPE_NOTE_ID } from "../../src/lib/systemTypes";
import { startEdenLiveRefreshSubscription } from "../../src/store/edenLiveRefreshSubscription";

function entry(id: string): Entry {
  return {
    id,
    title: "Новая заметка",
    content_json: "{}",
    created_at: 1,
    updated_at: 2,
    folder_id: null,
    type_id: SYSTEM_TYPE_NOTE_ID,
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
    content_loaded: false,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("Eden live refresh subscription", () => {
  test("does not duplicate an entry inserted while its live refresh load is pending", async () => {
    let emit!: Parameters<typeof edenApi.subscribeObjectChanges>[0];
    let resolveListable!: (value: Entry) => void;
    const incoming = entry("new-entry");
    const entries = ref<Entry[]>([]);

    vi.spyOn(edenApi, "subscribeObjectChanges").mockImplementation((handler) => {
      emit = handler;
      return vi.fn();
    });
    const loadListableEntry = vi
      .spyOn(edenApi, "loadListableEntry")
      .mockImplementation(() => new Promise<Entry>((resolve) => (resolveListable = resolve)));
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {},
    });

    const upsertEntryBaseline = (next: Entry) => {
      const index = entries.value.findIndex((candidate) => candidate.id === next.id);
      if (index >= 0) entries.value[index] = next;
      else entries.value = [next, ...entries.value];
    };
    const scope = effectScope();
    scope.run(() =>
      startEdenLiveRefreshSubscription({
        entries,
        currentEntry: ref(null),
        isCurrentEntryDirty: ref(false),
        upsertEntryBaseline,
      }),
    );

    emit({ event: "object_upserted", id: incoming.id, typeId: incoming.type_id ?? undefined });
    await vi.waitFor(() => expect(loadListableEntry).toHaveBeenCalledOnce());
    upsertEntryBaseline(incoming);
    resolveListable(incoming);

    await vi.waitFor(() => expect(entries.value.map(({ id }) => id)).toEqual([incoming.id]));
    scope.stop();
  });
});

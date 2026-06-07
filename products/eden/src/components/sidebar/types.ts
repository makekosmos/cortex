import { getEntryDisplayTitle } from "@/lib/entryTitles";

export type SpaceId = "my-space" | "all-objects" | "all-notes" | "all-properties" | "diary";

export type SortMode = "updated_at" | "created_at" | "title";

export type DragPayload = { type: "entry"; id: string };

export function sortEntries(entries: Entry[], sortMode: SortMode) {
  return [...entries].sort((entryA, entryB) => {
    if (sortMode === "title") {
      return getEntryDisplayTitle(entryA.title, entryA.header_props_json).localeCompare(
        getEntryDisplayTitle(entryB.title, entryB.header_props_json),
        "ru",
      );
    }

    if (sortMode === "created_at") {
      return entryB.created_at - entryA.created_at;
    }

    return entryB.updated_at - entryA.updated_at;
  });
}

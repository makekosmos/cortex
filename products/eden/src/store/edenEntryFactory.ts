import { writeEntryMarkdown } from "@/editor-content/content";
import { createUntitledEntryHeaderProps } from "@/lib/entryTitles";
import { SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE_ID } from "@/lib/systemTypes";

export function createBlankEntry(args: { id: string; title?: string; noteTypeId?: string }): Entry {
  const now = Date.now();
  return {
    id: args.id,
    title: args.title ?? "",
    content_json: JSON.stringify(writeEntryMarkdown("")),
    created_at: now,
    updated_at: now,
    folder_id: null,
    type_id: args.noteTypeId ?? SYSTEM_TYPE_NOTE_ID,
    header_layout: null,
    header_props_json: JSON.stringify(createUntitledEntryHeaderProps()),
    schema_version: 1,
    deleted_at: null,
  };
}

export function todayJournalTitle(now = new Date()): string {
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

export function createTodayJournalEntry(id: string, title = todayJournalTitle()): Entry {
  const now = Date.now();
  return {
    id,
    title,
    content_json: JSON.stringify(writeEntryMarkdown("")),
    created_at: now,
    updated_at: now,
    folder_id: null,
    type_id: SYSTEM_TYPE_JOURNAL_ID,
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
  };
}

// Чистая логика сравнения черновика с сохранённой записью.
//
// Изолирована от Vue — легко тестируется через bun test.
// Используется в updateEntryDraft (store/eden.ts) чтобы определить,
// содержит ли черновик реальные пользовательские изменения.
//
// Проблема, которую решает этот модуль (см. postmortems 2026-06-17):
// CmEditor гидратирует headerProps.value через safeParseHeaderProps, которая
// добавляет дефолтные поля из схемы типа (description, related_notes для NOTE;
// first_name, last_name, ... для PERSON). Если в БД header_props_json пустой,
// первый buildEntryDraft уже содержит эти поля — и наивное сравнение JSON
// видит расхождение, хотя пользователь ничего не менял.
//
// Аналогично: header_layout=null в БД, но CmEditor ставит resolved дефолт
// ("inline"/"column") → наивное сравнение также видит расхождение.
//
// Решение: нормализовать оба entry одинаково через схему типа из noteTypes.

import { readEntryMarkdown } from "../editor-cm/content";
import { normalizeHeaderProps, resolveNoteTypeHeaderLayout } from "../lib/typedNotes";

function normalizedEntryTypeId(entry: Entry): string {
  return entry.type_id ?? "note_obj";
}

/**
 * Нормализует header_layout: если не задан явно — разворачивает в дефолт для типа.
 * Совпадает с логикой headerLayout.value в CmEditor.vue.
 */
function resolveEntryHeaderLayout(entry: Entry, noteTypes: NoteType[]): string {
  if (entry.header_layout != null) return entry.header_layout;
  const noteType = noteTypes.find((nt) => nt.id === normalizedEntryTypeId(entry)) ?? null;
  return resolveNoteTypeHeaderLayout(noteType);
}

/**
 * Нормализует header_props_json через схему типа.
 * Совпадает с тем, что safeParseHeaderProps возвращает в CmEditor:
 * добавляет дефолтные поля, приводит типы значений.
 * Этим устраняется ложное расхождение при первом открытии.
 */
function resolveEntryHeaderProps(entry: Entry, noteTypes: NoteType[]): string {
  const noteType = noteTypes.find((nt) => nt.id === normalizedEntryTypeId(entry)) ?? null;
  let raw: unknown;
  try {
    raw = entry.header_props_json ? JSON.parse(entry.header_props_json) : {};
  } catch {
    raw = {};
  }
  const normalized = normalizeHeaderProps(noteType, raw);
  return JSON.stringify(normalized);
}

/**
 * Возвращает true если черновик содержит реальные пользовательские изменения
 * относительно previousEntry.
 *
 * Нормализует header_layout и header_props через схему типа, чтобы
 * чисто-нормализационные отличия (открытие заметки без ввода) не давали false positive.
 */
export function hasUserVisibleEntryChanges(
  nextEntry: Entry,
  previousEntry: Entry,
  noteTypes: NoteType[],
): boolean {
  if (nextEntry.title !== previousEntry.title) return true;
  if (normalizedEntryTypeId(nextEntry) !== normalizedEntryTypeId(previousEntry)) return true;
  if (
    resolveEntryHeaderLayout(nextEntry, noteTypes) !==
    resolveEntryHeaderLayout(previousEntry, noteTypes)
  ) {
    return true;
  }
  if (
    resolveEntryHeaderProps(nextEntry, noteTypes) !==
    resolveEntryHeaderProps(previousEntry, noteTypes)
  ) {
    return true;
  }
  return (
    readEntryMarkdown(nextEntry.content_json) !== readEntryMarkdown(previousEntry.content_json)
  );
}

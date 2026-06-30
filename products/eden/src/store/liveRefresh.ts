// Чистая функция-решение для live-обновления открытой заметки при удалённом изменении.
//
// Изолирована от Vue и любых сторонних зависимостей — легко тестируется через bun test.
// Вся опасная логика (не затирать ввод, не зацикливаться на self-echo) сконцентрирована здесь.

import { readEntryMarkdown } from "../editor-content/content";

export type RemoteEntryDecision =
  /** Применить удалённые изменения — безопасно, редактор не dirty, контент отличается. */
  | "apply"
  /** Пропустить — markdown идентичен текущему (self-echo от autosave или нет изменений). */
  | "skip-same-content"
  /** Пропустить — редактор dirty (пользователь редактирует), чтобы не затирать ввод. */
  | "skip-dirty";

export interface ShouldApplyRemoteEntryParams {
  /** Свежезагруженная заметка из ARK (результат loadEntry). */
  fresh: Entry;
  /** content_json текущей открытой заметки в редакторе. */
  currentContentJson: string;
  /** Редактор содержит несохранённые изменения. */
  isEditorDirty: boolean;
}

export function isOlderRemoteEntry(
  fresh: Pick<Entry, "updated_at">,
  current: Pick<Entry, "updated_at">,
): boolean {
  return fresh.updated_at < current.updated_at;
}

/**
 * Решает: нужно ли применить удалённое изменение к открытой заметке.
 *
 * Порядок проверок:
 * 1. Content-equality guard (предпочтительный, устойчив к гонкам):
 *    если markdown fresh == markdown current → self-echo или нет реальных изменений → skip.
 * 2. Dirty guard: если редактор dirty (пользователь редактирует) → skip, не затирать ввод.
 * 3. Иначе → apply.
 */
export function shouldApplyRemoteEntry(params: ShouldApplyRemoteEntryParams): RemoteEntryDecision {
  const { fresh, currentContentJson, isEditorDirty } = params;

  const freshMarkdown = readEntryMarkdown(fresh.content_json);
  const currentMarkdown = readEntryMarkdown(currentContentJson);

  // 1. Content-equality guard — ловит self-echo и случаи без реальных изменений.
  if (freshMarkdown === currentMarkdown) {
    return "skip-same-content";
  }

  // 2. Dirty guard — не затираем активный пользовательский ввод.
  if (isEditorDirty) {
    return "skip-dirty";
  }

  return "apply";
}

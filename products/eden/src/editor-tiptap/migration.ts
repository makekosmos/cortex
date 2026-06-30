import {
  isMarkdownContent,
  isTiptapContent,
  readEntryTiptapDoc,
  writeEntryTiptapDoc,
} from "../editor-cm/content";

export type TipTapMigrationSkipReason =
  | "deleted"
  | "already_tiptap"
  | "invalid_json"
  | "unsupported_content";

export type TipTapMigrationFailureStage = "convert" | "persist" | "rollback";

export interface TipTapMigrationSkipped {
  id: string;
  reason: TipTapMigrationSkipReason;
}

export interface TipTapMigrationFailure {
  id: string;
  error: string;
  stage: TipTapMigrationFailureStage;
}

export interface TipTapMigrationResult {
  converted: number;
  skipped: number;
  failed: number;
  skippedEntries: TipTapMigrationSkipped[];
  updatedIds: string[];
  durationMs: number;
  correlationId: string;
  attempt: number;
  failures?: TipTapMigrationFailure[];
}

export interface TipTapMigrationPlan {
  total: number;
  migratable: number;
  skipped: number;
  skippedDeleted: number;
  skippedAlreadyTiptap: number;
  skippedInvalid: number;
  skippedUnsupported: number;
}

export interface TipTapRollbackResult {
  restored: number;
  skipped: number;
  failed: number;
  restoredIds: string[];
  durationMs: number;
  correlationId: string;
  attempt: number;
  failures?: TipTapMigrationFailure[];
}

export interface TipTapMigrationProgress {
  phase: "skip" | "persist" | "done";
  id?: string;
  processed: number;
  total: number;
  converted: number;
  skipped: number;
  failed: number;
  correlationId: string;
  attempt: number;
}

export interface TipTapMigrationOptions {
  signal?: AbortSignal;
  correlationId?: string;
  attempt?: number;
  onProgress?: (progress: TipTapMigrationProgress) => void;
}

export function buildTipTapMigrationContentJson(contentJson: string): string | null {
  const parsed = parseContent(contentJson);
  if (!parsed.ok || !isMarkdownContent(parsed.value)) return null;
  return JSON.stringify(writeEntryTiptapDoc(readEntryTiptapDoc(parsed.value)));
}

export function analyzeTipTapMigration(entries: ReadonlyArray<Entry>): TipTapMigrationPlan {
  const plan: TipTapMigrationPlan = {
    total: entries.length,
    migratable: 0,
    skipped: 0,
    skippedDeleted: 0,
    skippedAlreadyTiptap: 0,
    skippedInvalid: 0,
    skippedUnsupported: 0,
  };

  for (const entry of entries) {
    const skipReason = migrationSkipReason(entry);
    if (!skipReason) {
      plan.migratable += 1;
      continue;
    }

    plan.skipped += 1;
    if (skipReason === "deleted") plan.skippedDeleted += 1;
    if (skipReason === "already_tiptap") plan.skippedAlreadyTiptap += 1;
    if (skipReason === "invalid_json") plan.skippedInvalid += 1;
    if (skipReason === "unsupported_content") plan.skippedUnsupported += 1;
  }

  return plan;
}

export function createTipTapMigrationSnapshot(entries: ReadonlyArray<Entry>): Entry[] {
  return entries.filter((entry) => !migrationSkipReason(entry)).map((entry) => ({ ...entry }));
}

export function countMigratableEntries(
  entries: ReadonlyArray<Pick<Entry, "content_json">>,
): number {
  return entries.reduce(
    (count, entry) => count + (buildTipTapMigrationContentJson(entry.content_json) ? 1 : 0),
    0,
  );
}

export async function migrateEntriesToTiptapJson(
  entries: ReadonlyArray<Entry>,
  saveEntry: (entry: Entry) => Promise<unknown>,
  updateEntryDraft?: (entry: Entry) => void,
  options: TipTapMigrationOptions = {},
): Promise<TipTapMigrationResult> {
  const startedAt = nowMilliseconds();
  const correlationId = options.correlationId ?? createCorrelationId();
  const attempt = options.attempt ?? 1;
  let converted = 0;
  let skipped = 0;
  let failed = 0;
  const skippedEntries: TipTapMigrationSkipped[] = [];
  const failures: TipTapMigrationFailure[] = [];
  const updatedIds: string[] = [];

  for (const [index, entry] of entries.entries()) {
    assertNotAborted(options.signal);

    const skipReason = migrationSkipReason(entry);
    if (skipReason) {
      skipped += 1;
      skippedEntries.push({ id: entry.id, reason: skipReason });
      emitProgress(options, {
        phase: "skip",
        id: entry.id,
        processed: index + 1,
        total: entries.length,
        converted,
        skipped,
        failed,
        correlationId,
        attempt,
      });
      continue;
    }

    let nextContentJson: string;
    try {
      nextContentJson = buildTipTapMigrationContentJson(entry.content_json) ?? "";
      if (!nextContentJson) throw new Error("Unsupported content_json");
    } catch (error) {
      failed += 1;
      failures.push({
        id: entry.id,
        stage: "convert",
        error: error instanceof Error ? error.message : "Unknown conversion error",
      });
      emitProgress(options, {
        phase: "persist",
        id: entry.id,
        processed: index + 1,
        total: entries.length,
        converted,
        skipped,
        failed,
        correlationId,
        attempt,
      });
      continue;
    }

    const migratedEntry: Entry = {
      ...entry,
      content_json: nextContentJson,
      updated_at: Math.max(Date.now(), entry.updated_at + 1),
    };

    try {
      const result = await saveEntry(migratedEntry);
      if (isFailedSaveResult(result)) {
        failed += 1;
        failures.push({
          id: entry.id,
          stage: "persist",
          error: describeMigrationError(result),
        });
        continue;
      }
      updateEntryDraft?.(migratedEntry);
      updatedIds.push(entry.id);
      converted += 1;
    } catch (error) {
      failed += 1;
      failures.push({
        id: entry.id,
        stage: "persist",
        error: error instanceof Error ? error.message : "Unknown migration error",
      });
    } finally {
      emitProgress(options, {
        phase: "persist",
        id: entry.id,
        processed: index + 1,
        total: entries.length,
        converted,
        skipped,
        failed,
        correlationId,
        attempt,
      });
    }
  }

  const result: TipTapMigrationResult = {
    converted,
    skipped,
    failed,
    skippedEntries,
    updatedIds,
    durationMs: Math.round(nowMilliseconds() - startedAt),
    correlationId,
    attempt,
  };
  if (failures.length > 0) result.failures = failures;
  emitProgress(options, {
    phase: "done",
    processed: entries.length,
    total: entries.length,
    converted,
    skipped,
    failed,
    correlationId,
    attempt,
  });
  return result;
}

export async function rollbackTipTapMigration(
  snapshot: ReadonlyArray<Entry>,
  saveEntry: (entry: Entry) => Promise<unknown>,
  updateEntryDraft?: (entry: Entry) => void,
  options: TipTapMigrationOptions = {},
): Promise<TipTapRollbackResult> {
  const startedAt = nowMilliseconds();
  const correlationId = options.correlationId ?? createCorrelationId();
  const attempt = options.attempt ?? 1;
  let restored = 0;
  let skipped = 0;
  let failed = 0;
  const failures: TipTapMigrationFailure[] = [];
  const restoredIds: string[] = [];

  for (const [index, originalEntry] of snapshot.entries()) {
    assertNotAborted(options.signal);

    if (originalEntry.deleted_at !== null) {
      skipped += 1;
      continue;
    }

    const restoredEntry: Entry = {
      ...originalEntry,
      updated_at: Math.max(Date.now(), originalEntry.updated_at + 1),
    };

    try {
      const result = await saveEntry(restoredEntry);
      if (isFailedSaveResult(result)) {
        failed += 1;
        failures.push({
          id: originalEntry.id,
          stage: "rollback",
          error: describeMigrationError(result),
        });
        continue;
      }
      updateEntryDraft?.(restoredEntry);
      restoredIds.push(originalEntry.id);
      restored += 1;
    } catch (error) {
      failed += 1;
      failures.push({
        id: originalEntry.id,
        stage: "rollback",
        error: error instanceof Error ? error.message : "Unknown rollback error",
      });
    } finally {
      emitProgress(options, {
        phase: "persist",
        id: originalEntry.id,
        processed: index + 1,
        total: snapshot.length,
        converted: restored,
        skipped,
        failed,
        correlationId,
        attempt,
      });
    }
  }

  const result: TipTapRollbackResult = {
    restored,
    skipped,
    failed,
    restoredIds,
    durationMs: Math.round(nowMilliseconds() - startedAt),
    correlationId,
    attempt,
  };
  if (failures.length > 0) result.failures = failures;
  return result;
}

function migrationSkipReason(entry: Entry): TipTapMigrationSkipReason | null {
  if (entry.deleted_at !== null) return "deleted";
  const parsed = parseContent(entry.content_json);
  if (!parsed.ok) return "invalid_json";
  if (isMarkdownContent(parsed.value)) return null;
  if (isTiptapContent(parsed.value)) return "already_tiptap";
  return "unsupported_content";
}

function parseContent(contentJson: string): { ok: true; value: unknown } | { ok: false } {
  try {
    return { ok: true, value: JSON.parse(contentJson) };
  } catch {
    return { ok: false };
  }
}

function isFailedSaveResult(result: unknown): result is object {
  return !!result && typeof result === "object" && "ok" in result && result.ok === false;
}

function describeMigrationError(result: object): string {
  if ("message" in result && typeof result.message === "string" && result.message.trim()) {
    return result.message;
  }
  if ("reason" in result && typeof result.reason === "string" && result.reason.trim()) {
    return result.reason;
  }
  return "Save returned ok=false";
}

function emitProgress(options: TipTapMigrationOptions, progress: TipTapMigrationProgress): void {
  options.onProgress?.(progress);
}

function assertNotAborted(signal: AbortSignal | undefined): void {
  if (!signal?.aborted) return;
  throw signal.reason instanceof Error ? signal.reason : new Error("Migration aborted");
}

function createCorrelationId(): string {
  const randomUUID = globalThis.crypto?.randomUUID?.bind(globalThis.crypto);
  return randomUUID ? randomUUID() : `tiptap-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function nowMilliseconds(): number {
  return globalThis.performance?.now?.() ?? Date.now();
}

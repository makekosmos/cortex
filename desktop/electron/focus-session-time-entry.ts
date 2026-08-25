import type { ArkObjectLike } from "./focus-session-types";
import { isRecord, isString, type JsonRecord } from "../src/shared/runtimeGuards";

type Invoke = <T = unknown>(operation: string, params?: JsonRecord) => Promise<T>;

type WorkContext = { title: string; taskId: string | null; taskTitle: string | null };

let currentEntryId: string | null = null;
let lastWorkContext: WorkContext | null = null;

const EMPTY_PROPS: JsonRecord = Object.freeze({});

function arkPayload<T>(value: T): JsonRecord {
  const payload: JsonRecord = {};
  Object.assign(payload, value);
  return payload;
}

function toProps(record: ArkObjectLike): JsonRecord {
  const props = record.propsJson ?? record.props_json;
  return isRecord(props) ? props : EMPTY_PROPS;
}

function makeEntryId(): string {
  return `te-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function getCurrentEntryId(): string | null {
  return currentEntryId;
}

export function getLastWorkContext(): WorkContext | null {
  return lastWorkContext;
}

interface FocusTimeEntryApi {
  rehydrateRunningEntry: () => Promise<string | null>;
  startTimeEntry: (ctx: WorkContext) => Promise<string>;
  closeTimeEntry: (completed: boolean) => Promise<void>;
  markTaskDone: (taskId: string) => Promise<void>;
}

export function createFocusTimeEntryApi(invoke: Invoke): FocusTimeEntryApi {
  async function listRunningPomodoroEntries(): Promise<ArkObjectLike[]> {
    const list = await invoke<ArkObjectLike[]>("list_running_time_entries", {
      source: "pomodoro",
    });
    return Array.isArray(list) ? list : [];
  }

  async function rehydrateRunningEntry(): Promise<string | null> {
    const running = await listRunningPomodoroEntries();
    const target = running.find((entry) => {
      const startedAt = toProps(entry).startedAt;
      return isString(startedAt) && startedAt.length > 0;
    });
    currentEntryId = target?.id ?? null;
    if (target) {
      const props = toProps(target);
      lastWorkContext = {
        title: target.title ?? "Фокус",
        taskId: isString(props.taskId) ? props.taskId : null,
        taskTitle: isString(props.taskTitle) ? props.taskTitle : null,
      };
    }
    return currentEntryId;
  }

  return {
    rehydrateRunningEntry,

    async startTimeEntry(ctx: WorkContext): Promise<string> {
      const now = new Date().toISOString();
      const id = makeEntryId();
      const record = {
        id,
        typeId: "time_entry_obj",
        title: ctx.title,
        contentJson: {},
        propsJson: {
          startedAt: now,
          endedAt: null,
          source: "pomodoro",
          billable: false,
          taskId: ctx.taskId,
          taskTitle: ctx.taskTitle,
          completed: false,
        },
        createdAt: now,
        updatedAt: now,
        deletedAt: null,
      };
      // SAFETY: the ARK upsert payload is assembled from validated object fields above.
      await invoke("upsert_object", arkPayload({ object: record }));
      currentEntryId = id;
      lastWorkContext = ctx;
      return id;
    },

    async closeTimeEntry(completed: boolean): Promise<void> {
      const id = currentEntryId ?? (await rehydrateRunningEntry());
      if (!id) return;
      const existing = await invoke<ArkObjectLike | null>("get_object", { id });
      if (!existing) {
        currentEntryId = null;
        return;
      }
      const now = new Date().toISOString();
      const ctx = lastWorkContext;
      const props = {
        ...toProps(existing),
        endedAt: now,
        completed,
        taskId: ctx?.taskId ?? toProps(existing).taskId ?? null,
        taskTitle: ctx?.taskTitle ?? toProps(existing).taskTitle ?? null,
      };
      const record = {
        id: existing.id,
        typeId: existing.typeId ?? existing.type_id ?? "time_entry_obj",
        title: ctx?.title ?? existing.title ?? "Фокус",
        contentJson: existing.contentJson ?? existing.content_json ?? {},
        propsJson: props,
        createdAt: existing.createdAt ?? existing.created_at ?? now,
        updatedAt: now,
        deletedAt: existing.deletedAt ?? existing.deleted_at ?? null,
      };
      // SAFETY: the ARK upsert payload is assembled from validated object fields above.
      await invoke("upsert_object", arkPayload({ object: record }));
      currentEntryId = null;
    },

    async markTaskDone(taskId: string): Promise<void> {
      const existing = await invoke<ArkObjectLike | null>("get_object", { id: taskId });
      if (!existing) return;
      const now = new Date().toISOString();
      const props = {
        ...toProps(existing),
        status: "done",
        is_completed: true,
        completed_at: now,
      };
      const record = {
        id: existing.id,
        typeId: existing.typeId ?? existing.type_id ?? "task_obj",
        title: existing.title ?? "",
        contentJson: existing.contentJson ?? existing.content_json ?? {},
        propsJson: props,
        createdAt: existing.createdAt ?? existing.created_at ?? now,
        updatedAt: now,
        deletedAt: existing.deletedAt ?? existing.deleted_at ?? null,
      };
      await invoke("upsert_object", arkPayload({ object: record }));
    },
  };
}

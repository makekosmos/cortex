// Адаптер ARK операций для Horologion-extension'а.
//
// В standalone Horologion (apps/horologion) main процесс предоставлял
// `window.horologion.*` IPC контракт. В Vue extension Kepler shell нет
// собственного main процесса — все ARK-операции идут через
// `window.kepler.ark.request(operation, params)`, который main proxy
// прокидывает в `ArkClient.invokeOperation`.
//
// Этот модуль восстанавливает форму `window.horologion.*` чтобы
// portированные composable'ы и view'ы (usePomodoro, StopwatchView,
// PomodoroView, ListView, MentionInput) могли работать без массового
// переписывания call-site'ов. Внутри каждый метод трансформирует ввод
// в нужный ARK operation и mapping возвращаемых ArkObjectRecord →
// TimeEntry / Tag / DelphiTask.
//
// Главное отличие от legacy main процесса:
//   - Никакого command-bus registration здесь не делается (живёт в main.ts
//     extension'а через kepler.ark.subscribe).
//   - `settings.open()` намеренно не реализован — settings рисуется внутри
//     основного окна (route /settings).

import type { ArkObjectRecord, JsonValue } from "@kosmos/ark";

import type {
  CreateTimeEntryInput,
  DelphiTask,
  StartTimerInput,
  Tag,
  TimeEntry,
  UpdateTimeEntryInput,
} from "../types";

interface KeplerArk {
  request: <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

function kepler(): KeplerArk {
  const k = (window as unknown as { kepler?: { ark: KeplerArk } }).kepler;
  if (!k) {
    throw new Error("window.kepler.ark not available — extension preload не подключён");
  }
  return k.ark;
}

function ark<T = unknown>(operation: string, params?: Record<string, unknown>): Promise<T> {
  return kepler().request<T>(operation, params);
}

function asObject(value: JsonValue | undefined): Record<string, JsonValue> {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, JsonValue>;
  }
  return {};
}

function objectToTimeEntry(obj: ArkObjectRecord): TimeEntry {
  const props = asObject(obj.propsJson);
  // Запись считается валидной только если props.startedAt — non-empty string.
  // Раньше fallback на obj.createdAt маскировал orphan entries (props пуст,
  // createdAt всё равно есть → entry «running вечно»). StopwatchView'овский
  // filter `Boolean(e.startedAt)` после такого fallback'а проходил насквозь.
  const rawStarted = props.startedAt;
  const startedAt =
    typeof rawStarted === "string" && rawStarted.length > 0 ? rawStarted : "";
  const rawEnded = props.endedAt;
  const endedAt =
    typeof rawEnded === "string" && rawEnded.length > 0 ? rawEnded : null;
  return {
    id: obj.id,
    title: obj.title ?? "",
    startedAt,
    endedAt,
    source: (props.source as TimeEntry["source"]) ?? "manual",
    billable: Boolean(props.billable ?? false),
    tagIds: [],
    taskId: (props.taskId as string | null | undefined) ?? null,
    taskTitle: (props.taskTitle as string | null | undefined) ?? null,
    completed:
      typeof props.completed === "boolean" ? (props.completed as boolean) : undefined,
  };
}

async function listTimeEntries(): Promise<TimeEntry[]> {
  const list = await ark<ArkObjectRecord[]>("list_objects_by_type", { type_id: "time_entry_obj" });
  return list
    .filter((o) => !o.deletedAt)
    .map(objectToTimeEntry)
    .sort((a, b) => b.startedAt.localeCompare(a.startedAt));
}

async function listRunning(opts?: {
  /** Фильтр по source. Передай "manual" чтобы получить только обычные stopwatch
   * сессии без pomodoro work/break (StopwatchView нельзя показывать pomodoro_break
   * — её «стоп» закроет entry в обход pomodoro session lifecycle). */
  source?: TimeEntry["source"] | TimeEntry["source"][];
}): Promise<TimeEntry[]> {
  // Используем dedicated ARK op `list_running_time_entries` — SQL-уровневый
  // фильтр (endedAt IS NULL + source через json_extract), без обхода ВСЕХ
  // time_entry_obj. Сортировка startedAt DESC сделана на бэке.
  //
  // Для массива source делаем параллельные вызовы — running entries в общей
  // сложности обычно ≤ 3 (одна manual + одна pomodoro work/break).
  const sources: Array<TimeEntry["source"] | undefined> = Array.isArray(opts?.source)
    ? opts!.source
    : [opts?.source];

  const lists = await Promise.all(
    sources.map((s) =>
      ark<ArkObjectRecord[]>(
        "list_running_time_entries",
        s ? { source: s } : {},
      ),
    ),
  );

  // Dedup по id (на случай дубликатов между параллельными запросами — в норме
  // не должно быть, но защита от race condition с upsert'ами между вызовами).
  const byId = new Map<string, ArkObjectRecord>();
  for (const list of lists) {
    for (const o of list) {
      if (o.deletedAt) continue;
      byId.set(o.id, o);
    }
  }

  const entries = [...byId.values()].map(objectToTimeEntry);
  // Backend уже сортирует startedAt DESC, но при склейке нескольких source'ов
  // нужна повторная сортировка. И — orphan filter (startedAt пуст) сохраняется,
  // потому что StopwatchView tick делает new Date(startedAt) и без guard'а
  // получит NaN на каждом тике.
  return entries
    .filter((e) => Boolean(e.startedAt))
    .sort((a, b) => b.startedAt.localeCompare(a.startedAt));
}

function makeId(): string {
  return `te-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

async function startTimer(input: StartTimerInput): Promise<TimeEntry> {
  const now = new Date().toISOString();
  const record: ArkObjectRecord = {
    id: makeId(),
    typeId: "time_entry_obj",
    title: input.title,
    contentJson: {},
    propsJson: {
      startedAt: now,
      endedAt: null,
      source: input.source ?? "manual",
      billable: Boolean(input.billable),
      taskId: input.taskId ?? null,
      taskTitle: input.taskTitle ?? null,
      // completed выставляется отдельным update'ом из usePomodoroSession при
      // natural finish; на старте — false по умолчанию (отсутствие = не завершён).
      completed: false,
    },
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
  };
  await ark("upsert_object", { object: record });
  return objectToTimeEntry(record);
}

async function stopTimer(id: string): Promise<TimeEntry> {
  const existing = await ark<ArkObjectRecord | null>("get_object", { id });
  if (!existing) throw new Error(`time_entry ${id} not found`);
  const now = new Date().toISOString();
  const props: Record<string, JsonValue> = { ...asObject(existing.propsJson), endedAt: now };
  const record: ArkObjectRecord = { ...existing, propsJson: props, updatedAt: now };
  await ark("upsert_object", { object: record });
  return objectToTimeEntry(record);
}

async function updateTimeEntry(input: UpdateTimeEntryInput): Promise<TimeEntry> {
  const existing = await ark<ArkObjectRecord | null>("get_object", { id: input.id });
  if (!existing) throw new Error(`time_entry ${input.id} not found`);
  const now = new Date().toISOString();
  const props = asObject(existing.propsJson);
  if (input.startedAt !== undefined) props.startedAt = input.startedAt;
  if (input.endedAt !== undefined) props.endedAt = input.endedAt;
  if (input.billable !== undefined) props.billable = input.billable;
  if (input.taskId !== undefined) props.taskId = input.taskId;
  if (input.taskTitle !== undefined) props.taskTitle = input.taskTitle;
  if (input.completed !== undefined) props.completed = input.completed;
  const record: ArkObjectRecord = {
    ...existing,
    title: input.title !== undefined ? input.title : existing.title,
    propsJson: props,
    updatedAt: now,
  };
  await ark("upsert_object", { object: record });
  return objectToTimeEntry(record);
}

async function createTimeEntry(input: CreateTimeEntryInput): Promise<TimeEntry> {
  const now = new Date().toISOString();
  const record: ArkObjectRecord = {
    id: makeId(),
    typeId: "time_entry_obj",
    title: input.title,
    contentJson: {},
    propsJson: {
      startedAt: input.startedAt,
      endedAt: input.endedAt,
      source: input.source ?? "manual",
      billable: Boolean(input.billable),
      taskId: input.taskId ?? null,
      taskTitle: input.taskTitle ?? null,
      completed: input.completed ?? false,
    },
    createdAt: input.startedAt,
    updatedAt: now,
    deletedAt: null,
  };
  await ark("upsert_object", { object: record });
  return objectToTimeEntry(record);
}

async function deleteTimeEntry(id: string): Promise<void> {
  await ark("delete_object", { id });
}

/**
 * Сколько pomodoro work-фаз дошло до конца сегодня (локальная дата).
 *
 * Считается только запись с source="pomodoro" И completed=true.
 * Прерванный pomodoro (stop / pause-без-resume) остаётся как time_entry,
 * но не учитывается — будущая статистика «отменённые» возьмёт его как
 * source="pomodoro" + completed=false.
 */
async function countTodayCompletedPomodoros(): Promise<number> {
  const list = await ark<ArkObjectRecord[]>("list_objects_by_type", {
    type_id: "time_entry_obj",
  });
  const now = new Date();
  const dayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const dayEnd = dayStart + 24 * 60 * 60 * 1000;
  let n = 0;
  for (const o of list) {
    if (o.deletedAt) continue;
    const props = asObject(o.propsJson);
    if (props.source !== "pomodoro") continue;
    if (props.completed !== true) continue;
    const started = typeof props.startedAt === "string" ? props.startedAt : "";
    if (!started) continue;
    const t = Date.parse(started);
    if (Number.isNaN(t)) continue;
    if (t >= dayStart && t < dayEnd) n++;
  }
  return n;
}

async function listTags(): Promise<Tag[]> {
  const list = await ark<ArkObjectRecord[]>("list_objects_by_type", { type_id: "tag_obj" });
  return list
    .filter((o) => !o.deletedAt)
    .map((o) => {
      const props = asObject(o.propsJson);
      return {
        id: o.id,
        name: o.title ?? "",
        color: (props.color as string | null | undefined) ?? null,
      };
    });
}

async function listDelphiTasks(): Promise<DelphiTask[]> {
  const list = await ark<ArkObjectRecord[]>("list_objects_by_type", { type_id: "task_obj" });
  return list
    .filter((o) => !o.deletedAt)
    .map((o) => {
      const props = asObject(o.propsJson);
      return {
        id: o.id,
        title: o.title ?? "",
        status: (props.status as string | null | undefined) ?? null,
      };
    });
}

/**
 * Минимальная форма-имитация `window.horologion.*` API. Только те части,
 * которые реально используются portированными composable'ами и view'ами.
 * Settings open / onCommand вырезаны — см. module-doc выше.
 */
export interface HorologionExtensionApi {
  timeEntries: {
    list(): Promise<TimeEntry[]>;
    listRunning(opts?: {
      source?: TimeEntry["source"] | TimeEntry["source"][];
    }): Promise<TimeEntry[]>;
    startTimer(input: StartTimerInput): Promise<TimeEntry>;
    stopTimer(id: string): Promise<TimeEntry>;
    update(input: UpdateTimeEntryInput): Promise<TimeEntry>;
    create(input: CreateTimeEntryInput): Promise<TimeEntry>;
    delete(id: string): Promise<void>;
    countTodayCompletedPomodoros(): Promise<number>;
  };
  tags: { list(): Promise<Tag[]> };
  tasks: { list(): Promise<DelphiTask[]> };
}

export const horologionApi: HorologionExtensionApi = {
  timeEntries: {
    list: listTimeEntries,
    listRunning,
    startTimer,
    stopTimer,
    update: updateTimeEntry,
    create: createTimeEntry,
    delete: deleteTimeEntry,
    countTodayCompletedPomodoros,
  },
  tags: { list: listTags },
  tasks: { list: listDelphiTasks },
};

// Глобальный shim — все portированные модули обращаются к `window.horologion`
// (legacy конвенция). В extension'е этот объект формируется здесь, а не
// preload'ом. Settings open не реализован — call-site'ы (App.vue, PomodoroView,
// pomodoroSettings) обновлены, чтобы не использовать эти ветки.
(window as unknown as { horologion: HorologionExtensionApi }).horologion = horologionApi;

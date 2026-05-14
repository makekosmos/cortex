// Bridge legacy `window.electronAPI.*` (legacy delphi main process IPC) поверх
// `window.kepler.ark.request(operation, params)` — единственного канала к ARK,
// доступного в extension renderer'е.
//
// Зачем shim: existing Vue компоненты Delphi (App.vue, ProjectPage.vue, store,
// space-manager, SpaceSetup, SpacesSettingsTab, local-db, json.electron) были
// портированы как есть из standalone приложения, где Electron main process
// предоставлял широкий набор IPC каналов (db:*, ark:*, lan-sync:*, sync:*,
// space:*). В extension'е этих каналов нет — есть только `kepler.ark.request`.
// Чтобы не переписывать каждый call-site, мы восстанавливаем форму
// `window.electronAPI` и внутри транслируем legacy каналы в ARK operations
// либо graceful no-op'ы для функций, которых физически нет (P2P sync, file
// system, switch space).
//
// Импортируется в `main.ts` как side-effect до `createApp(App).mount(...)`,
// чтобы любой ранний обращающийся код увидел уже установленный shim.

import type { Area, Project, Tag, TodoItem } from "@/types/task";

// ---------------------------------------------------------------------------
// Types & helpers
// ---------------------------------------------------------------------------

type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string | null;
  contentJson?: JsonValue;
  propsJson?: JsonValue;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

interface KeplerArk {
  request: <T = unknown>(
    operation: string,
    params?: Record<string, unknown>,
  ) => Promise<T>;
  subscribe: (event: string, handler: (payload: unknown) => void) => () => void;
}

function kepler(): KeplerArk | null {
  const k = (window as unknown as { kepler?: { ark: KeplerArk } }).kepler;
  return k ? k.ark : null;
}

const DELPHI_TASK_OBJECT_TYPE_ID = "task_obj";

function isoNow(): string {
  return new Date().toISOString();
}

function asObject(value: unknown): Record<string, unknown> {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  return {};
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((v): v is string => typeof v === "string")
    : [];
}

function asBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function asNumber(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function asNullableString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function textToArkContentJson(text: string | null | undefined): JsonValue {
  const trimmed = (text ?? "").trim();
  if (!trimmed) {
    return { type: "doc", content: [{ type: "paragraph" }] };
  }
  return {
    type: "doc",
    content: [
      { type: "paragraph", content: [{ type: "text", text: trimmed }] },
    ],
  };
}

function extractPlainTextFromContentJson(value: unknown): string | null {
  const chunks: string[] = [];
  function visit(node: unknown) {
    if (!node) return;
    if (typeof node === "string") {
      chunks.push(node);
      return;
    }
    if (Array.isArray(node)) {
      node.forEach(visit);
      return;
    }
    if (typeof node !== "object") return;
    const record = node as Record<string, unknown>;
    if (typeof record.text === "string") chunks.push(record.text);
    if (Array.isArray(record.content)) {
      for (const child of record.content) visit(child);
      if (
        record.type === "paragraph" ||
        record.type === "heading" ||
        record.type === "blockquote" ||
        record.type === "listItem"
      ) {
        chunks.push("\n");
      }
    }
  }
  visit(value);
  const normalized = chunks.join("").replace(/\n{3,}/g, "\n\n").trim();
  return normalized || null;
}

// ---------------------------------------------------------------------------
// task_obj <-> TodoItem mapping
// (Source of truth: apps/delphi/ts/shared/task-ark.ts — keep in sync.)
// ---------------------------------------------------------------------------

function todoToArkTaskObject(todo: TodoItem): ArkObjectRecord {
  const updatedAt = isoNow();
  const description = todo.notes ?? null;
  return {
    id: todo.id,
    typeId: DELPHI_TASK_OBJECT_TYPE_ID,
    title: todo.title,
    contentJson: textToArkContentJson(description),
    propsJson: {
      description,
      priority: todo.priority,
      scheduled_date: todo.scheduledDate ?? null,
      deadline: todo.deadline ?? null,
      reminder_date: todo.reminderDate ?? null,
      is_today: todo.isToday,
      is_evening: todo.isEvening,
      is_someday: todo.isSomeday,
      is_completed: todo.isCompleted,
      completed_at: todo.completedAt ?? null,
      is_cancelled: todo.isCancelled,
      cancelled_at: todo.cancelledAt ?? null,
      is_trashed: todo.isTrashed,
      sort_order: todo.sortOrder,
      heading_id: todo.headingId ?? null,
      project_id: todo.projectId ?? null,
      area_id: todo.areaId ?? null,
      tag_ids: todo.tagIds,
      checklist_items: todo.checklistItems as unknown as JsonValue,
      recurrence_rule: (todo.recurrenceRule ?? null) as unknown as JsonValue,
      billable: todo.billable ?? false,
      price: todo.price ?? null,
      created_at: todo.createdAt,
      source_app: "delphi",
      model_version: 1,
    },
    createdAt: todo.createdAt,
    updatedAt,
    deletedAt: todo.isTrashed ? updatedAt : null,
  };
}

function arkTaskObjectToTodo(object: ArkObjectRecord): TodoItem {
  const props = asObject(object.propsJson);
  const description =
    asNullableString(props.description) ??
    extractPlainTextFromContentJson(object.contentJson);
  return {
    id: object.id.toLowerCase(),
    title: object.title ?? "",
    notes: description,
    priority: asNumber(props.priority, 0),
    scheduledDate: asNullableString(props.scheduled_date),
    deadline: asNullableString(props.deadline),
    reminderDate: asNullableString(props.reminder_date),
    isToday: asBoolean(props.is_today, false),
    isEvening: asBoolean(props.is_evening, false),
    isSomeday: asBoolean(props.is_someday, false),
    isCompleted: asBoolean(props.is_completed, false),
    completedAt: asNullableString(props.completed_at),
    isCancelled: asBoolean(props.is_cancelled, false),
    cancelledAt: asNullableString(props.cancelled_at),
    isTrashed: asBoolean(props.is_trashed, false) || Boolean(object.deletedAt),
    sortOrder: asNumber(props.sort_order, 0),
    createdAt: asNullableString(props.created_at) ?? object.createdAt,
    headingId: asNullableString(props.heading_id),
    projectId: asNullableString(props.project_id),
    areaId: asNullableString(props.area_id),
    tagIds: asStringArray(props.tag_ids),
    checklistItems: Array.isArray(props.checklist_items)
      ? (props.checklist_items as TodoItem["checklistItems"])
      : [],
    recurrenceRule:
      props.recurrence_rule && typeof props.recurrence_rule === "object"
        ? (props.recurrence_rule as TodoItem["recurrenceRule"])
        : null,
    billable: asBoolean(props.billable, false),
    price:
      typeof props.price === "number" && Number.isFinite(props.price)
        ? props.price
        : null,
  };
}

// ---------------------------------------------------------------------------
// Ark task operations
// ---------------------------------------------------------------------------

async function arkListTasks(): Promise<TodoItem[]> {
  const ark = kepler();
  if (!ark) return [];
  const list = await ark.request<ArkObjectRecord[]>("list_objects_by_type", {
    type_id: DELPHI_TASK_OBJECT_TYPE_ID,
  });
  return Array.isArray(list)
    ? list
        .filter((o) => o.typeId === DELPHI_TASK_OBJECT_TYPE_ID && !o.deletedAt)
        .map(arkTaskObjectToTodo)
    : [];
}

async function arkUpsertTask(todo: TodoItem): Promise<boolean> {
  const ark = kepler();
  if (!ark) return false;
  await ark.request("upsert_object", { object: todoToArkTaskObject(todo) });
  return true;
}

async function arkDeleteTask(id: string): Promise<boolean> {
  const ark = kepler();
  if (!ark) return false;
  await ark.request("delete_object", { id });
  return true;
}

interface TimeEntrySummary {
  id: string;
  taskId: string | null;
  billable: boolean;
  startedAt: string;
  endedAt: string | null;
}

async function arkListTimeEntries(): Promise<TimeEntrySummary[]> {
  const ark = kepler();
  if (!ark) return [];
  const list = await ark.request<ArkObjectRecord[]>("list_objects_by_type", {
    type_id: "time_entry_obj",
  });
  if (!Array.isArray(list)) return [];
  return list
    .filter((o) => !o.deletedAt)
    .map((o) => {
      const props = asObject(o.propsJson);
      return {
        id: o.id,
        taskId: asNullableString(props.taskId),
        billable: asBoolean(props.billable, false),
        startedAt: typeof props.startedAt === "string" ? props.startedAt : "",
        endedAt: asNullableString(props.endedAt),
      };
    });
}

// ---------------------------------------------------------------------------
// Channel dispatcher
// ---------------------------------------------------------------------------

const warnedChannels = new Set<string>();
function warnOnce(channel: string, detail?: string): void {
  if (warnedChannels.has(channel)) return;
  warnedChannels.add(channel);
  // eslint-disable-next-line no-console
  console.warn(
    `[delphi-extension] electronAPI channel '${channel}' недоступен в extension; используется no-op${detail ? ` (${detail})` : ""}`,
  );
}

async function invokeChannel(
  channel: string,
  args: unknown[],
): Promise<unknown> {
  switch (channel) {
    // ---- DB / Ark task ops ----
    case "db:loadAll":
    case "ark:listDelphiTasks": {
      const todos = await arkListTasks();
      // db:loadAll возвращает full snapshot — projects/areas/tags нет в ARK
      // для Delphi, fallback'и обрабатываются через localStorage / store.
      if (channel === "db:loadAll") {
        return {
          todos,
          projects: [] as Project[],
          areas: [] as Area[],
          tags: [] as Tag[],
          headings: [],
        };
      }
      return todos;
    }

    case "db:upsertTodo":
    case "ark:upsertDelphiTask":
      return arkUpsertTask(args[0] as TodoItem);

    case "db:deleteTodo":
    case "ark:deleteDelphiTask":
      return arkDeleteTask(args[0] as string);

    case "db:batchUpsertTodos": {
      const todos = (args[0] ?? []) as TodoItem[];
      for (const todo of todos) await arkUpsertTask(todo);
      return true;
    }

    case "ark:listTimeEntries":
      return arkListTimeEntries();

    // Project / KV — нет соответствующих ARK operations здесь.
    case "db:upsertProject":
    case "db:deleteProject":
    case "db:getSyncKv":
    case "db:setSyncKv":
    case "db:clearAll":
    case "db:deleteTrashed":
      warnOnce(channel);
      return channel === "db:getSyncKv" ? null : true;

    // ---- LAN sync / space management ----
    // В extension'е P2P sync не реализован — kepler-backend имеет собственный
    // sync runtime, недоступный отсюда. Возвращаем "пусто", чтобы UI graceful
    // показывал offline без crash'а.
    case "lan-sync:start":
      warnOnce(channel, "P2P sync вне scope extension");
      return false;
    case "lan-sync:getStatus":
      return { active: false, peers: 0, peerNames: [] };
    case "lan-sync:broadcastChange":
    case "lan-sync:leaveSpace":
      warnOnce(channel);
      return true;
    case "sync:getOwnAddresses":
      warnOnce(channel);
      return [] as string[];
    case "sync:getQrPayload":
      // Возвращаем undefined → caller fallback'ает на formatSpaceCode().
      return undefined;

    case "db:switchSpace":
      // В extension'е база одна — глобальная ARK shell'а. Поэтому здесь
      // graceful no-op: данные уже соответствуют активному пространству
      // (если бэкенд переключал базы — это его ответственность).
      warnOnce(channel);
      return true;

    case "db:deleteSpace":
      warnOnce(channel);
      return false;

    // ---- File system (json.electron) ----
    // Extension renderer не имеет filesystem доступа — компонент json.electron
    // используется только legacy кодом, который сегодня не достижим в
    // extension flow'е, поэтому warn'аем и возвращаем пусто.
    case "space:getAll":
    case "space:save":
    case "space:remove":
    case "space:rename":
    case "space:getActive":
    case "space:setActive":
    case "space:getDbPath":
      // space-manager у себя fallback'ает на localStorage если ipc вернул
      // не то, что ожидается — поэтому возвращаем undefined для большинства,
      // а для get-вызовов возвращаем undefined чтобы fallback сработал.
      return undefined;

    default:
      warnOnce(channel, "неизвестный канал");
      return null;
  }
}

// ---------------------------------------------------------------------------
// Event channel mapping (electronAPI.on)
// ---------------------------------------------------------------------------

function subscribeChannel(
  channel: string,
  handler: (...args: unknown[]) => void,
): () => void {
  const ark = kepler();
  if (!ark) return () => {};

  switch (channel) {
    case "delphi:cmd:task:create":
    case "delphi:cmd:task:today":
      // Команды бридж'ятся в main.ts через kepler.ark.subscribe("command_invoked")
      // и тамошние router.push(...) / show() — отдельной подписки не нужно.
      return () => {};

    case "lan-sync:change":
    case "lan-sync:peerConnected":
    case "lan-sync:peerDisconnected":
      // P2P события вне scope extension'а — см. invokeChannel("lan-sync:start").
      // Возвращаем no-op unsubscribe.
      return () => {};

    default:
      warnOnce(`on:${channel}`, "неизвестное событие");
      return () => {};
  }
}

// ---------------------------------------------------------------------------
// Install shim
// ---------------------------------------------------------------------------

const electronApiShim = {
  invoke: (channel: string, ...args: unknown[]): Promise<unknown> =>
    invokeChannel(channel, args),
  on: (channel: string, handler: (...args: unknown[]) => void): (() => void) =>
    subscribeChannel(channel, handler),
  // fs / db namespaces намеренно отсутствуют:
  //   - `electronAPI.fs` нужен только legacy json.electron.ts, в extension
  //     flow'е недостижимому. `getElectronFs()` вернёт undefined → no-op.
  //   - `electronAPI.db` использовался для прямого Rust sidecar bridge'а;
  //     в extension'е `isLocalDbAvailable()` вернёт false и компоненты
  //     graceful fallback'ают на ARK через `electronAPI.invoke`.
};

// Устанавливаем shim только если real preload его не предоставил
// (защита от двойной инсталляции в потенциальном гибридном окружении).
const w = window as unknown as Record<string, unknown>;
if (!w.electronAPI) {
  w.electronAPI = electronApiShim;
}

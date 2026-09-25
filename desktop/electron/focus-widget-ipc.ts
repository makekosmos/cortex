import { BrowserWindow, Menu, ipcMain, type MenuItemConstructorOptions } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { FocusState } from "./focus-widget-state";
import type { FocusSessionSnapshot } from "./focus-session-types";
import type { JsonRecord, JsonValue } from "./json-types";

type ArkRequest = Parameters<ArkClient["invokeOperation"]>[0];

export interface FocusWidgetSessionActions {
  pause: () => Promise<FocusSessionSnapshot>;
  resume: () => Promise<FocusSessionSnapshot>;
  skip: () => Promise<FocusSessionSnapshot>;
  stop: () => Promise<FocusSessionSnapshot>;
  complete: () => Promise<FocusSessionSnapshot>;
}

interface ArkObjectLike {
  id: string;
  typeId?: string;
  type_id?: string;
  title?: string | null;
  contentJson?: JsonValue;
  content_json?: JsonValue;
  propsJson?: JsonRecord;
  props_json?: JsonRecord;
  createdAt?: string;
  created_at?: string;
  updatedAt?: string;
  updated_at?: string;
  deletedAt?: string | null;
  deleted_at?: string | null;
}

const EMPTY_PROPS: JsonRecord = Object.freeze({});

function timeEntryProps(record: ArkObjectLike): JsonRecord {
  return record.propsJson ?? record.props_json ?? EMPTY_PROPS;
}

export function registerFocusWidgetIpcHandlers(deps: {
  getWidgetWindow: () => BrowserWindow | null;
  hideWidget: () => void;
  openFocusSessionFromWidget: () => void;
  resetWidgetPosition: () => void;
  setFocusState: (next: Partial<FocusState>) => void;
  getFocusState: () => FocusState;
  getSessionActions: () => FocusWidgetSessionActions;
  requireRuntime: () => { awaitArkReady: () => Promise<ArkClient> };
}): void {
  ipcMain.handle("kepler:focus-widget:set-state", (_e, patch: Partial<FocusState>) => {
    deps.setFocusState(patch);
  });

  ipcMain.handle("kepler:focus-widget:get-state", () => deps.getFocusState());

  ipcMain.handle("kepler:focus-widget:hide", () => {
    deps.hideWidget();
  });

  ipcMain.handle("kepler:focus-widget:open-focus-session", async () => {
    deps.openFocusSessionFromWidget();
  });

  ipcMain.handle("kepler:focus-widget:show-menu", () => {
    const win = deps.getWidgetWindow();
    if (!win || win.isDestroyed()) return;
    const state = deps.getFocusState();
    const sessionItems: MenuItemConstructorOptions[] =
      state.mode === "stopwatch"
        ? [
            {
              label: "Остановить секундомер",
              click: () => void stopManualStopwatch(deps),
            },
          ]
        : focusSessionMenuItems(deps, state.isPaused);
    const menu = Menu.buildFromTemplate([
      ...sessionItems,
      { type: "separator" },
      {
        label: "Сбросить позицию",
        click: () => {
          deps.resetWidgetPosition();
        },
      },
      { type: "separator" },
      {
        label: "Скрыть виджет",
        click: () => {
          deps.hideWidget();
        },
      },
    ]);
    menu.popup({ window: win });
  });

  ipcMain.handle("kepler:focus-widget:pomodoro:pause", async () => {
    await deps.getSessionActions().pause();
  });
  ipcMain.handle("kepler:focus-widget:pomodoro:resume", async () => {
    await deps.getSessionActions().resume();
  });
  ipcMain.handle("kepler:focus-widget:pomodoro:skip", async () => {
    await deps.getSessionActions().skip();
  });
  ipcMain.handle("kepler:focus-widget:pomodoro:complete", async () => {
    await deps.getSessionActions().complete();
  });
  ipcMain.handle("kepler:focus-widget:pomodoro:stop", async () => {
    await deps.getSessionActions().stop();
  });
  ipcMain.handle("kepler:focus-widget:stopwatch:stop", async () => {
    await stopManualStopwatch(deps);
  });
}

function focusSessionMenuItems(
  deps: Parameters<typeof registerFocusWidgetIpcHandlers>[0],
  isPaused: boolean,
): MenuItemConstructorOptions[] {
  const actions = deps.getSessionActions();
  return [
    {
      label: isPaused ? "Продолжить" : "Пауза",
      click: () =>
        void runFocusAction(
          isPaused ? "resume" : "pause",
          isPaused ? actions.resume : actions.pause,
        ),
    },
    {
      label: "Редактировать",
      click: deps.openFocusSessionFromWidget,
    },
    {
      label: "Пропустить сессию",
      click: () => void runFocusAction("skip", actions.skip),
    },
    {
      label: "Выполнено",
      click: () => void runFocusAction("complete", actions.complete),
    },
    {
      label: "Отменить фокус",
      click: () => void runFocusAction("stop", actions.stop),
    },
  ];
}

async function runFocusAction(
  name: string,
  action: () => Promise<FocusSessionSnapshot>,
): Promise<void> {
  try {
    await action();
  } catch (error) {
    console.error(`[focus-widget] focus session ${name} failed:`, error);
  }
}

async function stopManualStopwatch(
  deps: Parameters<typeof registerFocusWidgetIpcHandlers>[0],
): Promise<void> {
  try {
    const client = await deps.requireRuntime().awaitArkReady();
    // SQL-уровневый фильтр endedAt IS NULL + source='manual' через json_extract
    // в ARK (`list_running_time_entries`). До 2026-05-21 здесь был
    // list_objects_by_type + client-side фильтр/сортировка — на больших
    // историях это тянуло всю time_entry_obj таблицу через WS.
    const running = await client.invokeOperation<ArkObjectLike[]>({
      operation: "list_running_time_entries",
      source: "manual",
    });
    if (!Array.isArray(running)) return;
    const nowIso = new Date().toISOString();
    // Backend уже отсортировал startedAt DESC и отфильтровал endedAt/deleted_at.
    // Сохраняем только startedAt guard — orphan entries без started тоже не
    // нужны (хотя по идее их нет, потому что endedAt IS NULL && startedAt пуст
    // — это broken state, но defensive).
    const target = running.find((o) => {
      const props = timeEntryProps(o);
      const started = props.startedAt;
      return isNonEmptyString(started);
    });
    if (!target) {
      console.warn("[focus-widget] stopwatch stop: no running manual time_entry");
      return;
    }
    const props = {
      ...timeEntryProps(target),
      endedAt: nowIso,
    };
    const record = {
      id: target.id,
      typeId: target.typeId ?? target.type_id ?? "time_entry_obj",
      title: target.title ?? "",
      contentJson: target.contentJson ?? target.content_json ?? {},
      propsJson: props,
      createdAt: target.createdAt ?? target.created_at ?? nowIso,
      updatedAt: nowIso,
      deletedAt: target.deletedAt ?? target.deleted_at ?? null,
    };
    const request: ArkRequest = {
      operation: "upsert_object",
      object: record,
    };
    await client.invokeOperation(request);
  } catch (e) {
    console.error("[focus-widget] stopwatch stop failed:", e);
  }
}

function isNonEmptyString(value: JsonValue | undefined): value is string {
  return typeof value === "string" && value.length > 0;
}

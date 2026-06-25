import { BrowserWindow, Menu, ipcMain } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { assertExtensionSenderHostPermissionIfExtension } from "./extension-host";
import {
  deriveFocusStateFromBackend,
  type FocusState,
  type PomodoroEventState,
} from "./focus-widget-state";

interface ArkObjectLike {
  id: string;
  typeId?: string;
  type_id?: string;
  title?: string | null;
  contentJson?: unknown;
  content_json?: unknown;
  propsJson?: Record<string, unknown>;
  props_json?: Record<string, unknown>;
  createdAt?: string;
  created_at?: string;
  updatedAt?: string;
  updated_at?: string;
  deletedAt?: string | null;
  deleted_at?: string | null;
}

const EMPTY_PROPS: Record<string, never> = Object.freeze({});

function timeEntryProps(record: ArkObjectLike): Record<string, unknown> {
  return record.propsJson ?? record.props_json ?? EMPTY_PROPS;
}

export function registerFocusWidgetIpcHandlers(deps: {
  getWidgetWindow: () => BrowserWindow | null;
  hideWidget: () => void;
  openFocusSessionFromWidget: () => void;
  resetWidgetPosition: () => void;
  setFocusState: (next: Partial<FocusState>) => void;
  getFocusState: () => FocusState;
  requireRuntime: () => { awaitArkReady: () => Promise<ArkClient> };
}): void {
  ipcMain.handle("kepler:focus-widget:set-state", (e, patch: Partial<FocusState>) => {
    if (!patch || typeof patch !== "object") return;
    assertExtensionSenderHostPermissionIfExtension(e.sender, "focus.control");
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
    const menu = Menu.buildFromTemplate([
      {
        label: "Редактировать",
        click: () => {
          deps.openFocusSessionFromWidget();
        },
      },
      {
        label: "Пропустить сессию",
        click: () => {
          void invokePomodoro(deps, "skip");
        },
      },
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
    await invokePomodoro(deps, "pause");
  });
  ipcMain.handle("kepler:focus-widget:pomodoro:resume", async () => {
    await invokePomodoro(deps, "resume");
  });
  ipcMain.handle("kepler:focus-widget:pomodoro:skip", async () => {
    await invokePomodoro(deps, "skip");
  });
  ipcMain.handle("kepler:focus-widget:pomodoro:stop", async () => {
    await invokePomodoro(deps, "stop");
  });
  ipcMain.handle("kepler:focus-widget:stopwatch:stop", async () => {
    await stopManualStopwatch(deps);
  });
}

async function invokePomodoro(
  deps: Parameters<typeof registerFocusWidgetIpcHandlers>[0],
  op: "pause" | "resume" | "skip" | "stop",
): Promise<void> {
  try {
    const client = await deps.requireRuntime().awaitArkReady();
    const state = await client.invokeOperation<PomodoroEventState>({
      operation: `pomodoro.${op}`,
    });
    if (state && typeof state === "object") {
      // См. postmortems.md § 2026-05-30: pause/resume return state but do not emit
      // pomodoro events, so the widget must apply the operation response directly.
      deps.setFocusState(deriveFocusStateFromBackend(state));
    }
  } catch (e) {
    console.error(`[focus-widget] pomodoro.${op} failed:`, e);
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
    const running = (await client.invokeOperation({
      operation: "list_running_time_entries",
      source: "manual",
    } as { operation: string; [k: string]: unknown })) as ArkObjectLike[];
    if (!Array.isArray(running)) return;
    const nowIso = new Date().toISOString();
    // Backend уже отсортировал startedAt DESC и отфильтровал endedAt/deleted_at.
    // Сохраняем только startedAt guard — orphan entries без started тоже не
    // нужны (хотя по идее их нет, потому что endedAt IS NULL && startedAt пуст
    // — это broken state, но defensive).
    const target = running.find((o) => {
      const props = timeEntryProps(o);
      const started = props.startedAt;
      return typeof started === "string" && started.length > 0;
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
    await client.invokeOperation({
      operation: "upsert_object",
      object: record,
    } as { operation: string; [k: string]: unknown });
  } catch (e) {
    console.error("[focus-widget] stopwatch stop failed:", e);
  }
}

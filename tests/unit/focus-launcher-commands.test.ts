import { describe, expect, test } from "bun:test";
import type { CommandRecord } from "../../platform/desktop/shared/ipc-types";
import {
  buildFocusAwareCommands,
  FOCUS_START_COMMAND_ID,
  FOCUS_PAUSE_TOGGLE_ID,
  FOCUS_DONE_ID,
  FOCUS_STOP_ID,
  FOCUS_EDIT_ID,
} from "../../platform/desktop/src/lib/focusLauncherCommands";

function baseCommands(): CommandRecord[] {
  return [
    { id: "settings:open", title: "Открыть настройки", category: "open", kind: "command" },
    { id: FOCUS_START_COMMAND_ID, title: "Начать фокус", category: "open", kind: "command" },
    {
      id: "kepler:focus-pause",
      title: "Поставить фокус на паузу",
      category: "action",
      kind: "command",
    },
    { id: "kepler:focus-complete", title: "Завершить фокус", category: "action", kind: "command" },
    { id: "app:steam", title: "Steam", category: "open", kind: "app" },
  ];
}

const ids = (list: CommandRecord[]) => list.map((c) => c.id);

describe("focus launcher commands", () => {
  test("idle: keeps «Начать фокус», drops raw command-bus focus actions", () => {
    const out = buildFocusAwareCommands(baseCommands(), { active: false, paused: false });
    expect(ids(out)).toEqual(["settings:open", FOCUS_START_COMMAND_ID, "app:steam"]);
  });

  test("active running: replaces «Начать фокус» with widget-parity commands", () => {
    const out = buildFocusAwareCommands(baseCommands(), { active: true, paused: false });
    expect(ids(out)).toEqual([
      "settings:open",
      FOCUS_PAUSE_TOGGLE_ID,
      FOCUS_DONE_ID,
      FOCUS_STOP_ID,
      FOCUS_EDIT_ID,
      "app:steam",
    ]);
    const toggle = out.find((c) => c.id === FOCUS_PAUSE_TOGGLE_ID)!;
    expect(toggle.title).toBe("Приостановить фокус");
    const done = out.find((c) => c.id === FOCUS_DONE_ID)!;
    expect(done.title).toBe("Отметить задачу выполненной");
  });

  test("active paused: pause toggle reads «Продолжить фокус»", () => {
    const out = buildFocusAwareCommands(baseCommands(), { active: true, paused: true });
    const toggle = out.find((c) => c.id === FOCUS_PAUSE_TOGGLE_ID)!;
    expect(toggle.title).toBe("Продолжить фокус");
  });

  test("preserves order and non-focus commands untouched", () => {
    const out = buildFocusAwareCommands(baseCommands(), { active: true, paused: false });
    expect(out[0]!.id).toBe("settings:open");
    expect(out.at(-1)!.id).toBe("app:steam");
  });
});

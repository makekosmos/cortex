import type { CommandRecord } from "@shared/ipc-types";

// Команда «Начать фокус» из command bus (commands.ts).
export const FOCUS_START_COMMAND_ID = "kepler:focus-session";

// Синтетические launcher-команды управления активной фокус-сессией. Не
// существуют в command bus — обрабатываются прямо в LauncherView.invokeSelected.
export const FOCUS_PAUSE_TOGGLE_ID = "kepler:focus-launcher-pause-toggle";
export const FOCUS_DONE_ID = "kepler:focus-launcher-done";
export const FOCUS_STOP_ID = "kepler:focus-launcher-stop";
export const FOCUS_EDIT_ID = "kepler:focus-launcher-edit";

export const FOCUS_LAUNCHER_COMMAND_IDS: ReadonlySet<string> = new Set([
  FOCUS_PAUSE_TOGGLE_ID,
  FOCUS_DONE_ID,
  FOCUS_STOP_ID,
  FOCUS_EDIT_ID,
]);

// Сырые command-bus focus-команды прячем из лаунчера — их заменяет
// состояние-зависимый набор (или «Начать фокус» в idle).
const RAW_FOCUS_ACTION_IDS: ReadonlySet<string> = new Set([
  "kepler:focus-toggle",
  "kepler:focus-pause",
  "kepler:focus-resume",
  "kepler:focus-skip",
  "kepler:focus-complete",
]);

export interface FocusCommandState {
  active: boolean;
  paused: boolean;
}

export function activeFocusCommands(state: FocusCommandState): CommandRecord[] {
  return [
    {
      id: FOCUS_PAUSE_TOGGLE_ID,
      title: state.paused ? "Продолжить фокус" : "Приостановить фокус",
      subtitle: state.paused
        ? "Снять таймер с паузы и продолжить"
        : "Пауза таймера — можно вернуться позже",
      category: "action",
      kind: "command",
      appName: "Kosmos",
    },
    {
      id: FOCUS_DONE_ID,
      title: "Отметить задачу выполненной",
      subtitle: "Остановить сессию и отметить задачу выполненной",
      category: "action",
      kind: "command",
      appName: "Kosmos",
    },
    {
      id: FOCUS_STOP_ID,
      title: "Завершить фокус",
      subtitle: "Остановить сессию, не трогая задачу",
      category: "action",
      kind: "command",
      appName: "Kosmos",
    },
    {
      id: FOCUS_EDIT_ID,
      title: "Редактировать фокус",
      subtitle: "Изменить цель, длительность и блокировку",
      category: "action",
      kind: "command",
      appName: "Kosmos",
    },
  ];
}

// На месте «Начать фокус» подставляем состояние-зависимый набор: idle — сама
// «Начать фокус»; активная сессия — пауза/выполнена/завершить/редактировать.
// Остальные сырые command-bus focus-команды убираем (дублируют).
export function buildFocusAwareCommands(
  commands: readonly CommandRecord[],
  state: FocusCommandState,
): CommandRecord[] {
  const out: CommandRecord[] = [];
  for (const cmd of commands) {
    if (cmd.id === FOCUS_START_COMMAND_ID) {
      if (state.active) out.push(...activeFocusCommands(state));
      else out.push(cmd);
      continue;
    }
    if (RAW_FOCUS_ACTION_IDS.has(cmd.id)) continue;
    out.push(cmd);
  }
  return out;
}

import kosmosIconPng from "../../../build/icon.png";
import {
  FOCUS_DONE_ID,
  FOCUS_EDIT_ID,
  FOCUS_PAUSE_TOGGLE_ID,
  FOCUS_SKIP_ID,
  FOCUS_START_COMMAND_ID,
  FOCUS_STOP_ID,
} from "../../lib/focusLauncherCommands";

import type { AppCommandSetting, AppSettingsTab } from "./navigation";


const FOCUS_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.7 0.16 145)",
  iconTo: "oklch(0.46 0.14 165)",
};

export const appCommandSettings = {
  "time-tracker": [
    {
      id: FOCUS_START_COMMAND_ID,
      title: "Начать фокус",
      icon: kosmosIconPng,
      ...FOCUS_COMMAND_GRADIENT,
    },
    {
      id: FOCUS_PAUSE_TOGGLE_ID,
      title: "Приостановить / продолжить фокус",
      icon: kosmosIconPng,
      ...FOCUS_COMMAND_GRADIENT,
    },
    {
      id: FOCUS_SKIP_ID,
      title: "Пропустить сессию",
      icon: kosmosIconPng,
      ...FOCUS_COMMAND_GRADIENT,
    },
    {
      id: FOCUS_DONE_ID,
      title: "Отметить задачу выполненной",
      icon: kosmosIconPng,
      ...FOCUS_COMMAND_GRADIENT,
    },
    {
      id: FOCUS_STOP_ID,
      title: "Отменить фокус",
      icon: kosmosIconPng,
      ...FOCUS_COMMAND_GRADIENT,
    },
    {
      id: FOCUS_EDIT_ID,
      title: "Редактировать фокус",
      icon: kosmosIconPng,
      ...FOCUS_COMMAND_GRADIENT,
    },
  ],
} satisfies Record<AppSettingsTab, AppCommandSetting[]>;

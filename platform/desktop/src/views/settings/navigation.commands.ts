import arraSvg from "../../assets/arra.svg";
import delphiAddSvg from "../../assets/delphi-add.svg";
import delphiSvg from "../../assets/delphi.svg";
import edenAddSvg from "../../assets/eden-add.svg";
import edenDiarySvg from "../../assets/eden-diary.svg";
import edenSvg from "../../assets/eden.svg";
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

const EDEN_COMMAND_GRADIENT = { iconFrom: "#ff5c00", iconTo: "#b33800" };
const DELPHI_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.78 0.14 230)",
  iconTo: "oklch(0.5 0.18 245)",
};
const FOCUS_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.7 0.16 145)",
  iconTo: "oklch(0.46 0.14 165)",
};
const ARRANCADOR_COMMAND_GRADIENT = {
  iconFrom: "oklch(0.7 0.2 25)",
  iconTo: "oklch(0.45 0.18 20)",
};
const DICTATION_COMMAND_GRADIENT = {
  iconFrom: "#F472B6",
  iconTo: "#BE185D",
};

export const appCommandSettings: Record<AppSettingsTab, AppCommandSetting[]> = {
  notes: [
    {
      id: "eden:open",
      title: "Открыть Eden",
      icon: edenSvg,
      ...EDEN_COMMAND_GRADIENT,
    },
    {
      id: "eden:note:create",
      title: "Создать заметку",
      icon: edenAddSvg,
      ...EDEN_COMMAND_GRADIENT,
    },
    {
      id: "eden:note:open-today",
      title: "Открыть сегодняшнюю заметку",
      icon: edenDiarySvg,
      ...EDEN_COMMAND_GRADIENT,
    },
  ],
  tasks: [
    {
      id: "delphi:open",
      title: "Открыть Delphi",
      icon: delphiSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
    {
      id: "delphi:inbox",
      title: "Открыть входящие",
      icon: delphiAddSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
    {
      id: "delphi:task:create",
      title: "Создать задачу",
      icon: delphiAddSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
    {
      id: "delphi:task:today",
      title: "Открыть сегодняшние задачи",
      icon: delphiSvg,
      ...DELPHI_COMMAND_GRADIENT,
    },
  ],
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
  games: [
    {
      id: "arrancador:open",
      title: "Открыть Arrancador",
      icon: arraSvg,
      ...ARRANCADOR_COMMAND_GRADIENT,
    },
  ],
  dictation: [
    {
      id: "kepler:dictation",
      title: "Переключить диктовку",
      icon: kosmosIconPng,
      ...DICTATION_COMMAND_GRADIENT,
    },
  ],
};

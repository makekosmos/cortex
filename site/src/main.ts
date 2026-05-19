import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";

// Stub `window.kepler` чтобы реальный LauncherView.vue (импортированный
// из shell/src/views/LauncherView.vue) рендерился в браузере без Electron
// IPC. Surface — минимум того что LauncherView использует.

const HARDCODED_COMMANDS = [
  {
    id: "delphi:open",
    title: "Открыть Delphi",
    category: "open",
    kind: "app",
    icon: undefined,
  },
  {
    id: "horologion:open",
    title: "Открыть Horologion",
    category: "open",
    kind: "app",
    icon: undefined,
  },
  {
    id: "dashboard:open",
    title: "Открыть таблицу данных",
    subtitle: "Kepler",
    category: "open",
    kind: "command",
    appName: "Kepler",
    icon: undefined,
  },
  {
    id: "settings:open",
    title: "Открыть настройки",
    subtitle: "Kepler",
    category: "open",
    kind: "command",
    appName: "Kepler",
    icon: undefined,
  },
  {
    id: "kepler:check-updates",
    title: "Проверить обновления",
    subtitle: "Kepler",
    category: "open",
    kind: "command",
    appName: "Kepler",
    icon: undefined,
  },
  {
    id: "delphi:inbox",
    title: "Открыть входящие",
    subtitle: "Delphi",
    category: "open",
    kind: "command",
    appName: "Delphi",
    icon: undefined,
  },
  {
    id: "horologion:pomodoro:25",
    title: "Помодоро 25 минут",
    subtitle: "Horologion",
    category: "action",
    kind: "command",
    appName: "Horologion",
    icon: undefined,
  },
  {
    id: "horologion:stopwatch:start",
    title: "Запустить секундомер",
    subtitle: "Horologion",
    category: "action",
    kind: "command",
    appName: "Horologion",
    icon: undefined,
  },
];

const noop = () => undefined;
const unsub = () => () => undefined;

// Минимальный stub window.kepler — тоже что preload экспонирует в shell,
// но возвращает hardcoded данные. Все мутирующие методы — no-op'ы.
(window as unknown as { kepler: unknown }).kepler = {
  commands: {
    list: () => Promise.resolve(HARDCODED_COMMANDS),
    invoke: () => Promise.resolve(),
    onUpdated: () => unsub(),
  },
  window: {
    hide: () => Promise.resolve(),
    onShow: () => unsub(),
    setExpanded: () => Promise.resolve(),
  },
  settings: {
    update: {
      check: () => Promise.resolve({ kind: "idle" }),
      install: () => Promise.resolve(),
      state: () => Promise.resolve({ kind: "idle" }),
      onStateChanged: () => unsub(),
    },
  },
};

createApp(App).mount("#app");

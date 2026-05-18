// Horologion Vue extension entry — Kepler shell host.
//
// Архитектура отличается от standalone `apps/horologion/src/main.ts`:
//   1. Нет собственного Electron main процесса — все ARK операции идут через
//      `window.kepler.ark.request(operation, params)`. См. `./lib/horologionApi.ts`,
//      где формируется shim `window.horologion.*` поверх ARK request'ов.
//   2. Router — memory history (extension загружается через `file://.../dist/index.html`,
//      hash navigation не нужен; back/forward через кнопку в topbar).
//   3. Settings — внутренний route `/settings`, рендерится в том же окне
//      (legacy открывал отдельный BrowserWindow через IPC).
//   4. Command bus — регистрируем `horologion:pomodoro:25`, `:50`, и
//      `stopwatch:start` через `kepler.ark.request("commands.register", ...)`
//      и слушаем `command_invoked` через `kepler.ark.subscribe(...)`.
//   5. Tray icon — out of scope для extension'а (требует tray API).

import { createApp } from "vue";
import { createMemoryHistory, createRouter, type RouteRecordRaw } from "vue-router";

import "@kosmos/visuals/theme/css";
import "./styles.css";
import "./lib/horologionApi"; // side-effect: устанавливает window.horologion shim
import App from "./App.vue";
import { pomodoroDraft, timerMode } from "./lib/store";
import { usePomodoroSession as usePomodoro } from "./lib/usePomodoroSession";

// Horologion использует тёмную тему kosmos-visuals (класс `.dark` в css-variables).
document.documentElement.classList.add("dark");

const routes: RouteRecordRaw[] = [
  { path: "/", component: () => import("./views/HomeView.vue"), name: "home" },
  { path: "/settings", component: () => import("./views/SettingsView.vue"), name: "settings" },
];

const router = createRouter({
  history: createMemoryHistory(),
  routes,
});

const app = createApp(App);
app.use(router);
app.mount("#app");

// Deep links: Kepler shell делает `openExtension(id, route)` и доставляет route
// через IPC. Hash в URL не используется — memoryHistory его не разбирает.
if (window.kepler?.navigation) {
  void window.kepler.navigation.initialRoute().then((r) => {
    if (r) void router.push(r);
  });
  window.kepler.navigation.onNavigate((r) => {
    void router.push(r);
  });
}

// ---------------------------------------------------------------------------
// Command bus — kepler.ark.subscribe("command_invoked", ...)
// ---------------------------------------------------------------------------
//
// Регистрируем horologion-команды и слушаем dispatch'и от Kosmos launcher'а.
// На каждый id применяем соответствующую mutation: pomodoro:start (с override
// длительности) или stopwatch:start. Pomodoro state — singleton из
// `./lib/usePomodoro`, так что start() ниже сразу же стартует фазу.

interface CommandInvokedEvent {
  id: string;
  params?: unknown;
}

const kepler = window.kepler;

if (kepler) {
  void kepler.ark
    .request("commands.register", {
      commands: [
        {
          id: "horologion:pomodoro:25",
          title: "Pomodoro 25 минут",
          subtitle: "Horologion",
          category: "action",
        },
        {
          id: "horologion:pomodoro:50",
          title: "Pomodoro 50 минут",
          subtitle: "Horologion",
          category: "action",
        },
        {
          id: "horologion:stopwatch:start",
          title: "Запустить секундомер",
          subtitle: "Horologion",
          category: "action",
        },
      ],
    })
    .catch((err: unknown) => {
      console.warn("[horologion-extension] commands.register failed:", err);
    });

  const pomodoro = usePomodoro();

  const off = kepler.ark.subscribe("command_invoked", (payload: unknown) => {
    const event = payload as CommandInvokedEvent | null;
    if (!event || typeof event.id !== "string") return;
    if (!event.id.startsWith("horologion:")) return;

    if (event.id === "horologion:pomodoro:25" || event.id === "horologion:pomodoro:50") {
      const durationMin = event.id === "horologion:pomodoro:25" ? 25 : 50;
      timerMode.value = "pomodoro";
      if (pomodoro.isRunning.value) return;
      void pomodoro.start({
        title: pomodoroDraft.value.title,
        tasks: pomodoroDraft.value.tasks.slice(),
        workMinOverride: durationMin,
      });
      return;
    }

    if (event.id === "horologion:stopwatch:start") {
      timerMode.value = "stopwatch";
      void (async () => {
        // source: "manual" — pomodoro_break / pomodoro running не считается
        // конфликтом для stopwatch start (это разные dimensions).
        const running = await window.horologion.timeEntries.listRunning({ source: "manual" });
        if (running.length > 0) return;
        const draft = pomodoroDraft.value;
        const firstTask = draft.tasks[0];
        await window.horologion.timeEntries.startTimer({
          title: draft.title.trim() || "Без названия",
          taskId: firstTask?.id ?? null,
          taskTitle: firstTask?.title ?? null,
        });
      })();
    }
  });

  window.addEventListener("beforeunload", () => {
    off();
  });
}

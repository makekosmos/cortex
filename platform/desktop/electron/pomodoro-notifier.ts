// pomodoro-notifier.ts — main-process подписчик на `pomodoro_phase_changed`,
// который шлёт native OS toast'ы при смене фазы Pomodoro независимо от того,
// открыто ли окно Horologion. До 2026-05-21 уведомления отправлялись из
// renderer'ом через DOM `Notification` — это работало только пока extension
// был открыт. Юзер закрыл Horologion → пропустил конец фокус-сегмента.
//
// Архитектура:
//   ArkClient.onArkEvent → фильтр по event === "pomodoro_phase_changed"
//   → выбор текста в зависимости от `from`/`to` → system-notifications.notify().
//
// Wire format (flat, см. forbidden.md):
//   { event: "pomodoro_phase_changed", from: Phase, to: Phase,
//     phase, remainingMs, totalMs, isRunning, ... }
//
// Transitions / тексты (только когда work ↔ break, idle skip'ается):
//   work → shortBreak | longBreak  →  «Pomodoro: время отдыха» + «Перерыв N минут»
//   shortBreak | longBreak → work  →  «Pomodoro: фокус» + «Работаем N минут»
//
// Длительность N берём из `totalMs` события (это новая фаза `to`) —
// renderer/backend уже подсчитал её, не надо угадывать.
//
// Lifecycle: arkClient может пересоздаваться при backend respawn (см.
// `resetArkClient` в main.ts). Поэтому subscribe-функция аттач'ит unsubscribe
// per arkClient instance, а wiring в main.ts вызывает её заново после каждого
// успешного initArkClient.

import { ipcMain } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { notify } from "./system-notifications";

type Phase = "idle" | "work" | "shortBreak" | "longBreak";

const TITLE_BREAK = "Pomodoro: время отдыха";
const TITLE_WORK = "Pomodoro: фокус";

function isBreak(p: Phase): boolean {
  return p === "shortBreak" || p === "longBreak";
}

function bodyForBreak(totalMs: number): string {
  const min = Math.max(1, Math.round(totalMs / 60_000));
  return `Перерыв ${min} ${pluralMinutes(min)}`;
}

function bodyForWork(totalMs: number): string {
  const min = Math.max(1, Math.round(totalMs / 60_000));
  return `Работаем ${min} ${pluralMinutes(min)}`;
}

function pluralMinutes(n: number): string {
  // Простой русский плюрал: 1 минута / 2-4 минуты / 5+ минут.
  const mod10 = n % 10;
  const mod100 = n % 100;
  if (mod10 === 1 && mod100 !== 11) return "минута";
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return "минуты";
  return "минут";
}

/**
 * Подписаться на phase_changed events для текущего arkClient'а.
 * Возвращает unsubscribe — звать при resetArkClient / shutdown.
 */
function subscribePhaseChanged(arkClient: ArkClient): () => void {
  return arkClient.onArkEvent((e) => {
    if (e.event !== "pomodoro_phase_changed") return;
    const raw = e as unknown as Record<string, unknown>;
    const from = (typeof raw.from === "string" ? raw.from : "idle") as Phase;
    const to = (typeof raw.to === "string" ? raw.to : "idle") as Phase;
    const totalMs = typeof raw.totalMs === "number" ? raw.totalMs : 0;

    // → idle: юзер сам остановил, native toast не нужен.
    if (to === "idle") return;

    // work → break.
    if (from === "work" && isBreak(to)) {
      notify({ title: TITLE_BREAK, body: bodyForBreak(totalMs) });
      return;
    }
    // break → work.
    if (isBreak(from) && to === "work") {
      notify({ title: TITLE_WORK, body: bodyForWork(totalMs) });
      return;
    }
    // idle → work (первый старт): тоже полезный toast.
    if (from === "idle" && to === "work") {
      notify({ title: TITLE_WORK, body: bodyForWork(totalMs) });
      return;
    }
    // Остальные комбинации (work → work, break → break) теоретически
    // невозможны — игнор.
  });
}

let currentUnsubscribe: (() => void) | null = null;
let ipcRegistered = false;

/**
 * Wiring entry-point: звать после каждого успешного `initArkClient()` в
 * main.ts. Idempotent: предыдущий subscribe отписывается перед новым.
 */
export function setupPomodoroNotifier(opts: { arkClient: ArkClient }): void {
  if (currentUnsubscribe) {
    try {
      currentUnsubscribe();
    } catch (e) {
      console.error("[pomodoro-notifier] previous unsubscribe failed:", e);
    }
    currentUnsubscribe = null;
  }
  currentUnsubscribe = subscribePhaseChanged(opts.arkClient);
  console.log("[pomodoro-notifier] subscribed to pomodoro_phase_changed");

  // Тестовый IPC: позволяет e2e/manual триггернуть toast без 25-минутного
  // ожидания фазы. Регистрируем только один раз — handler stateless.
  if (!ipcRegistered) {
    ipcMain.handle("kepler:pomodoro:notify-now", (_e, args?: { title?: string; body?: string }) =>
      notify({
        title: args?.title ?? TITLE_BREAK,
        body: args?.body ?? "Тестовое уведомление",
      }),
    );
    ipcRegistered = true;
  }
}

/** Cleanup при shutdown / backend reset. */
export function teardownPomodoroNotifier(): void {
  if (currentUnsubscribe) {
    try {
      currentUnsubscribe();
    } catch (e) {
      console.error("[pomodoro-notifier] teardown unsubscribe failed:", e);
    }
    currentUnsubscribe = null;
  }
}

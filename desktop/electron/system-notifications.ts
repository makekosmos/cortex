// system-notifications.ts — утилитарный wrapper над Electron Notification API.
//
// Зачем: native OS toast'ы должны приходить independently от того, открыто ли
// окно extension'а. Renderer'ный `new Notification(...)` работает только пока
// webContents жив; main process — пока жив Kepler.
//
// Headless / test guard: KOSMOS_HEADLESS=1 / KOSMOS_TEST_MODE=1 — `show()` не
// зовём. Подсчёт вызовов остаётся (для тестов / диагностики), просто toast не
// материализуется на экране.

import { Notification } from "electron";

export interface NotifyOptions {
  title: string;
  body: string;
  /** OS-default sound. true → silent. По умолчанию false (звук есть). */
  silent?: boolean;
  /** Reserved для будущих расширений (custom sound path, urgency, actions). */
  urgency?: "low" | "normal" | "critical";
}

const liveNotifications = new Set<Notification>();

function isHeadless(): boolean {
  return process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
}

function retainNotification(notification: Notification): void {
  liveNotifications.add(notification);
  const release = () => liveNotifications.delete(notification);
  notification.once("close", release);
  notification.once("failed", release);
}

/**
 * Показывает native OS toast. В headless / test mode — no-op (логируем для
 * диагностики). Не throw'ит: ошибки Notification API глотаются.
 *
 * Возвращает `true` если toast действительно был показан, `false` если
 * пропущен (headless / Notification API недоступен / OS отверг).
 */
export function notify(opts: NotifyOptions): boolean {
  if (isHeadless()) {
    console.log(`[system-notifications] headless skip: ${opts.title} — ${opts.body}`);
    return false;
  }
  if (!Notification.isSupported()) {
    console.warn("[system-notifications] Notification API not supported on this OS");
    return false;
  }
  try {
    const n = new Notification({
      title: opts.title,
      body: opts.body,
      silent: opts.silent ?? false,
      // urgency прокидывается только на Linux (no-op на Windows / macOS).
      urgency: opts.urgency,
    });
    n.show();
    // Держим ссылку чтобы GC не закрыл toast до того как OS его покажет.
    // Один live ref достаточен — OS буферизует toast'ы независимо от
    // JS-объекта после show().
    retainNotification(n);
    return true;
  } catch (e) {
    console.error("[system-notifications] notify failed:", e);
    return false;
  }
}

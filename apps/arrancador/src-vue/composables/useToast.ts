import { computed, shallowRef } from "vue";
import { subscribeAppEvent } from "../../src/lib/browser";

export type ToastTone = "info" | "success" | "warning" | "error";

export interface ToastInput {
  title: string;
  description?: string;
  tone?: ToastTone;
  durationMs?: number;
}

interface ToastEntry extends ToastInput {
  id: string;
}

const toasts = shallowRef<ToastEntry[]>([]);
let counter = 0;
let subscribed = false;

function removeToast(id: string) {
  toasts.value = toasts.value.filter((toast) => toast.id !== id);
}

export function initializeToastBridge() {
  if (subscribed) {
    return;
  }

  subscribed = true;
  subscribeAppEvent<{ game_name: string }>("game:save-path-missing", (payload) => {
    notify({
      tone: "warning",
      title: "Сохранения не найдены",
      description: payload?.game_name
        ? `Для "${payload.game_name}" не найден путь к сохранениям.`
        : "Не найден путь к сохранениям.",
    });
  });
}

export function notify(toast: ToastInput) {
  const entry: ToastEntry = {
    id: `${Date.now()}-${counter++}`,
    tone: "info",
    durationMs: 4200,
    ...toast,
  };

  toasts.value = [...toasts.value, entry];

  if (entry.durationMs && entry.durationMs > 0 && typeof window !== "undefined") {
    window.setTimeout(() => removeToast(entry.id), entry.durationMs);
  }
}

export function useToast() {
  return {
    toasts: computed(() => toasts.value),
    notify,
    removeToast,
  };
}

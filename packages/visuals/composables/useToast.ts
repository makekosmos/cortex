import { inject, provide, ref, type InjectionKey, type Ref } from "vue";

export type ToastTone = "info" | "success" | "error";

export interface ToastOptions {
  message: string;
  title?: string;
  description?: string;
  tone?: ToastTone;
  /** Auto-dismiss timeout в ms. По умолчанию 2000. */
  duration?: number;
  loading?: boolean;
  closable?: boolean;
}

export interface ToastItem {
  id: number;
  message: string;
  title?: string;
  description?: string;
  tone: ToastTone;
  duration: number;
  loading: boolean;
  closable: boolean;
}

export interface ToastApi {
  show(opts: ToastOptions): number;
  update(id: number, patch: Partial<ToastOptions>): void;
  dismiss(id: number): void;
}

interface ToastHostState {
  items: Ref<ToastItem[]>;
  api: ToastApi;
}

export const ToastKey: InjectionKey<ToastHostState> = Symbol("kosmos-toast");

/** Максимум видимых toast'ов одновременно — старые сжимаются/сбрасываются. */
const MAX_STACK = 5;

/**
 * Зовётся в setup() компонента-родителя (например App.vue). После вызова
 * `useToast()` работает в любом descendant'е. Сам host-компонент
 * (`<ToastHost />`) — отдельный визуальный renderer, делает inject и
 * рисует Teleport'ом в body. Сначала provide, потом mount host.
 */
export function provideToastHost(): { items: Ref<ToastItem[]>; api: ToastApi } {
  const items = ref<ToastItem[]>([]);
  let nextId = 1;
  // Regression M5 (2026-05-24): update(id, { duration }) used to schedule a
  // second setTimeout without cancelling the first — toast dismissed at the
  // earlier (old) timeout instead of the new one. Track timers per-id.
  const timers = new Map<number, number>();

  function scheduleDismiss(id: number, duration: number): void {
    const prev = timers.get(id);
    if (prev) {
      window.clearTimeout(prev);
      timers.delete(id);
    }
    if (duration > 0) {
      timers.set(
        id,
        window.setTimeout(() => dismiss(id), duration),
      );
    }
  }

  function show(opts: ToastOptions): number {
    const id = nextId++;
    const item: ToastItem = {
      id,
      message: opts.message,
      title: opts.title,
      description: opts.description,
      tone: opts.tone ?? "info",
      duration: opts.duration ?? 2000,
      loading: opts.loading ?? false,
      closable: opts.closable ?? false,
    };
    const next = [...items.value, item];
    items.value = next.length > MAX_STACK ? next.slice(next.length - MAX_STACK) : next;
    scheduleDismiss(id, item.duration);
    return id;
  }

  function update(id: number, patch: Partial<ToastOptions>): void {
    items.value = items.value.map((item) =>
      item.id === id
        ? {
            ...item,
            message: patch.message ?? item.message,
            title: patch.title ?? item.title,
            description: patch.description ?? item.description,
            tone: patch.tone ?? item.tone,
            duration: patch.duration ?? item.duration,
            loading: patch.loading ?? item.loading,
            closable: patch.closable ?? item.closable,
          }
        : item,
    );
    if (patch.duration !== undefined) {
      scheduleDismiss(id, patch.duration);
    }
  }

  function dismiss(id: number): void {
    const timer = timers.get(id);
    if (timer) {
      window.clearTimeout(timer);
      timers.delete(id);
    }
    items.value = items.value.filter((t) => t.id !== id);
  }

  const api: ToastApi = { show, update, dismiss };
  provide(ToastKey, { items, api });
  return { items, api };
}

/**
 * Use в любом потомке ToastHost'а. Возвращает API; если host не смонтирован —
 * no-op fallback (чтобы Storybook / unit-тесты компонентов не падали).
 */
export function useToast(): ToastApi {
  const state = inject(ToastKey, null);
  if (!state) {
    return {
      show: () => 0,
      update: () => {},
      dismiss: () => {},
    };
  }
  return state.api;
}

import { inject, provide, ref, type InjectionKey, type Ref } from "vue";

export type ToastTone = "info" | "success" | "error";

export interface ToastOptions {
  message: string;
  tone?: ToastTone;
  /** Auto-dismiss timeout в ms. По умолчанию 2000. */
  duration?: number;
}

export interface ToastItem {
  id: number;
  message: string;
  tone: ToastTone;
  duration: number;
}

export interface ToastApi {
  show(opts: ToastOptions): void;
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

  function show(opts: ToastOptions): void {
    const id = nextId++;
    const item: ToastItem = {
      id,
      message: opts.message,
      tone: opts.tone ?? "info",
      duration: opts.duration ?? 2000,
    };
    const next = [...items.value, item];
    items.value = next.length > MAX_STACK ? next.slice(next.length - MAX_STACK) : next;
    if (item.duration > 0) {
      window.setTimeout(() => dismiss(id), item.duration);
    }
  }

  function dismiss(id: number): void {
    items.value = items.value.filter((t) => t.id !== id);
  }

  const api: ToastApi = { show, dismiss };
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
      show: () => {},
      dismiss: () => {},
    };
  }
  return state.api;
}

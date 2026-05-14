import { ref, type Ref } from "vue";

/**
 * Хелпер для управления состоянием контекст-меню.
 *
 * `T` — тип payload'а, который ты передаёшь при открытии. Это id записи,
 * объект, или что-то ещё. После выбора пункта меню оно доступно как
 * `menu.payload.value`.
 *
 * Использование:
 * ```ts
 * const menu = useContextMenu<string>();
 * function onContextMenu(e: MouseEvent, id: string) {
 *   menu.open(e, id);
 * }
 * function onDelete() {
 *   if (menu.payload.value) removeEntry(menu.payload.value);
 *   menu.close();
 * }
 * ```
 */
export interface ContextMenuState<T> {
  isOpen: Ref<boolean>;
  x: Ref<number>;
  y: Ref<number>;
  payload: Ref<T | null>;
  open(event: MouseEvent, payload: T): void;
  close(): void;
}

export function useContextMenu<T = unknown>(): ContextMenuState<T> {
  const isOpen = ref(false);
  const x = ref(0);
  const y = ref(0);
  const payload = ref<T | null>(null) as Ref<T | null>;

  function open(event: MouseEvent, p: T) {
    event.preventDefault();
    x.value = event.clientX;
    y.value = event.clientY;
    payload.value = p;
    isOpen.value = true;
  }

  function close() {
    isOpen.value = false;
  }

  return { isOpen, x, y, payload, open, close };
}

import { computed, type Ref } from "vue";
import { storeToRefs } from "pinia";
import { useTodoStore } from "@/store/todos";
import type { SmartList, TodoItem } from "@/types/task";

/**
 * Returns filtered todos for the given smart list,
 * plus counts for all smart lists (for sidebar badges).
 */
export function useSmartList(list: Ref<SmartList> | SmartList) {
  const store = useTodoStore();
  const { smartListCounts } = storeToRefs(store);

  const filtered = computed<TodoItem[]>(() => {
    const listValue = typeof list === "string" ? list : list.value;
    return store.filteredTodos(listValue);
  });

  return { filtered, counts: smartListCounts };
}

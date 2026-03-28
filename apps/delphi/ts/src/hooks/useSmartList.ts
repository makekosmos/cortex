import { useMemo } from "react";
import useTodoStore from "@/store/todos";
import { SmartList } from "@/types/task";
import { filterTodos, countAll } from "@/services/filters/todoFilterService";

/**
 * Returns filtered todos for the given smart list,
 * plus counts for all smart lists (for sidebar badges).
 */
export default function useSmartList(list: SmartList) {
  const todos = useTodoStore((s) => s.todos);

  const filtered = useMemo(() => filterTodos(list, todos), [list, todos]);
  const counts = useMemo(() => countAll(todos), [todos]);

  return { filtered, counts };
}

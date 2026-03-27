import { useMemo } from 'react';
import useTask from '@/store/tasks';

export type Sorted = ReturnType<typeof useSortTasks>;
export type TaskFilterType = keyof Sorted;

export default function useSortTasks() {
  const tasks = useTask((s) => s.tasks);

  return useMemo(
    () => ({
      notCompleted: tasks.filter((task) => !task.completed),
      completed: tasks.filter((task) => task.completed),
    }),
    [tasks],
  );
}

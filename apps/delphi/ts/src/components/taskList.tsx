import MockTaskList from '@/components/mockTaskList';
import TaskItem from '@/components/taskItem';
import useSortTasks, {
  type Sorted,
  type TaskFilterType,
} from '@/hooks/useSortTasks';
import useTask from '@/store/tasks';

type TaskListProps = {
  sortType: TaskFilterType;
};

export default function TaskList({ sortType }: TaskListProps) {
  const sorted: Sorted = useSortTasks();
  const list = sorted[sortType];

  const hydrated = useTask((s) => s.hydrated);

  const completeTask = useTask((s) => s.completeTask);

  return (
    <div className="scrollbar-gutter flex-1 overflow-y-auto">
      <div className="max-w-threadcontentwidth scrollbar-gutter mx-auto w-full overflow-y-auto pb-20 select-none">
        {hydrated ? (
          <ul className="flex w-full min-w-0 flex-col gap-2">
            {list.map((task) => (
              <TaskItem
                key={task.id}
                task={task}
                onToggle={completeTask}
              />
            ))}
          </ul>
        ) : (
          <MockTaskList />
        )}
      </div>
    </div>
  );
}

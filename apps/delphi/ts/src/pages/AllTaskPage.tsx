import { Inbox } from 'lucide-react';
import useSmartList from '@/hooks/useSmartList';
import { SmartList } from '@/types/task';
import TodoRow from '@/components/TodoRow';
import useTodoStore from '@/store/todos';

export default function AllTaskPage() {
  const { filtered } = useSmartList(SmartList.Inbox);
  const completeTodo = useTodoStore((s) => s.completeTodo);
  const trashTodo = useTodoStore((s) => s.trashTodo);

  return (
    <div className="flex w-full min-w-0 flex-col">
      <div className="flex items-center gap-2.5 px-7 pb-3 pt-6">
        <Inbox size={24} className="text-blue-500" />
        <h1 className="text-2xl font-bold text-(--foreground) select-none">Входящие</h1>
        {filtered.length > 0 && (
          <span className="text-sm text-(--muted-foreground)">{filtered.length}</span>
        )}
      </div>

      <div className="scrollbar-gutter flex-1 overflow-y-auto">
        <div className="pb-20 pt-1">
          {filtered.length === 0 ? (
            <div className="px-7 py-10 text-center text-sm text-(--muted-foreground)/60">
              Нет входящих задач
            </div>
          ) : (
            <div className="flex flex-col">
              {filtered.map((todo) => (
                <TodoRow
                  key={todo.id}
                  todo={todo}
                  onComplete={() => completeTodo(todo.id)}
                  onTrash={() => trashTodo(todo.id)}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

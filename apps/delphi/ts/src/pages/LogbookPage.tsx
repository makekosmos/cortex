import { Book } from 'lucide-react';
import useSmartList from '@/hooks/useSmartList';
import { SmartList } from '@/types/task';
import TodoRow from '@/components/TodoRow';
import useTodoStore from '@/store/todos';

export default function LogbookPage() {
  const { filtered } = useSmartList(SmartList.Logbook);
  const incompleteTodo = useTodoStore((s) => s.incompleteTodo);

  return (
    <div className="flex w-full min-w-0 flex-col">
      <div className="flex items-center gap-2.5 px-7 pb-3 pt-6">
        <Book size={24} className="text-green-500" />
        <h1 className="text-2xl font-bold text-(--foreground) select-none">Журнал</h1>
        {filtered.length > 0 && (
          <span className="text-sm text-(--muted-foreground)">{filtered.length}</span>
        )}
      </div>

      <div className="scrollbar-gutter flex-1 overflow-y-auto">
        <div className="pb-20 pt-1">
          {filtered.length === 0 ? (
            <div className="px-7 py-10 text-center text-sm text-(--muted-foreground)/60">
              Завершённых задач нет
            </div>
          ) : (
            <div className="flex flex-col">
              {filtered.map((todo) => (
                <TodoRow
                  key={todo.id}
                  todo={todo}
                  onComplete={() => incompleteTodo(todo.id)}
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

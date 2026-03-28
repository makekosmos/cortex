import { Archive } from "lucide-react";
import useSmartList from "@/hooks/useSmartList";
import { SmartList } from "@/types/task";
import TodoRow from "@/components/TodoRow";
import useTodoStore from "@/store/todos";

export default function TrashPage() {
  const { filtered } = useSmartList(SmartList.Trash);
  const restoreTodo = useTodoStore((s) => s.restoreTodo);

  return (
    <div className="flex w-full min-w-0 flex-col">
      <div className="flex items-center gap-2.5 px-7 pb-3 pt-6">
        <Archive size={24} className="text-gray-500" />
        <h1 className="text-2xl font-bold text-(--foreground) select-none">
          Корзина
        </h1>
        {filtered.length > 0 && (
          <span className="text-sm text-(--muted-foreground)">
            {filtered.length}
          </span>
        )}
      </div>

      <div className="scrollbar-gutter flex-1 overflow-y-auto">
        <div className="pb-20 pt-1">
          {filtered.length === 0 ? (
            <div className="px-7 py-10 text-center text-sm text-(--muted-foreground)/60">
              Корзина пуста
            </div>
          ) : (
            <div className="flex flex-col">
              {filtered.map((todo) => (
                <TodoRow
                  key={todo.id}
                  todo={todo}
                  onComplete={() => restoreTodo(todo.id)}
                  extra={
                    <button
                      type="button"
                      onClick={() => restoreTodo(todo.id)}
                      className="rounded px-1.5 py-0.5 text-[10px] text-(--muted-foreground) hover:bg-emerald-500/15 hover:text-emerald-500"
                    >
                      Восстановить
                    </button>
                  }
                />
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

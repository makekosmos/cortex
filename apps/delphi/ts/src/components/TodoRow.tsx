import { Circle, CheckCircle2 } from "lucide-react";
import type { TodoItem } from "@/types/task";

type TodoRowProps = {
  todo: TodoItem;
  onComplete: () => void;
  onTrash?: () => void;
  extra?: React.ReactNode;
};

export default function TodoRow({
  todo,
  onComplete,
  onTrash,
  extra,
}: TodoRowProps) {
  const isCompleted = todo.isCompleted || todo.isCancelled;

  return (
    <div className="group flex items-center gap-3 px-7 py-2 hover:bg-(--secondary)">
      {/* Checkbox */}
      <button type="button" onClick={onComplete} className="shrink-0">
        {isCompleted ? (
          <CheckCircle2 size={18} className="text-(--muted-foreground)" />
        ) : (
          <Circle
            size={18}
            className="text-(--ring) hover:text-(--foreground)"
          />
        )}
      </button>

      {/* Content */}
      <div className="min-w-0 flex-1">
        <div
          className={`truncate text-sm ${isCompleted ? "text-(--muted-foreground) line-through" : "text-(--foreground)"}`}
        >
          {todo.title}
        </div>
        {todo.notes && (
          <div className="truncate text-xs text-(--muted-foreground)/70">
            {todo.notes}
          </div>
        )}
      </div>

      {extra}

      {/* Quick actions (visible on hover) */}
      {onTrash && !todo.isTrashed && (
        <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          <button
            type="button"
            onClick={onTrash}
            title="В корзину"
            className="rounded px-1.5 py-0.5 text-[10px] text-(--muted-foreground) hover:bg-red-500/15 hover:text-red-500"
          >
            Удалить
          </button>
        </div>
      )}
    </div>
  );
}

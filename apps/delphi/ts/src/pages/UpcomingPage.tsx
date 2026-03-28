import { Calendar } from "lucide-react";
import { useMemo } from "react";
import useTodoStore from "@/store/todos";
import type { TodoItem } from "@/types/task";

// ---------------------------------------------------------------------------
// Section type
// ---------------------------------------------------------------------------

type UpcomingSection = {
  id: string;
  title: string;
  dateLabel: string;
  todos: TodoItem[];
};

// ---------------------------------------------------------------------------
// Date helpers
// ---------------------------------------------------------------------------

function startOfDay(date: Date): Date {
  const d = new Date(date);
  d.setHours(0, 0, 0, 0);
  return d;
}

function addDays(date: Date, days: number): Date {
  const d = new Date(date);
  d.setDate(d.getDate() + days);
  return d;
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

const shortDateFmt = new Intl.DateTimeFormat("ru-RU", {
  weekday: "short",
  month: "short",
  day: "numeric",
});

const dayNameFmt = new Intl.DateTimeFormat("ru-RU", { weekday: "long" });

const monthFmt = new Intl.DateTimeFormat("ru-RU", {
  month: "long",
  year: "numeric",
});

function capitalize(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function dayTitle(date: Date, offset: number): string {
  if (offset === 1) return "Завтра";
  return capitalize(dayNameFmt.format(date));
}

function weekOfYear(date: Date): number {
  const d = new Date(
    Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()),
  );
  const dayNum = d.getUTCDay() || 7;
  d.setUTCDate(d.getUTCDate() + 4 - dayNum);
  const yearStart = new Date(Date.UTC(d.getUTCFullYear(), 0, 1));
  return Math.ceil(((d.getTime() - yearStart.getTime()) / 86400000 + 1) / 7);
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function UpcomingPage() {
  const todos = useTodoStore((s) => s.todos);
  const projects = useTodoStore((s) => s.projects);
  const completeTodo = useTodoStore((s) => s.completeTodo);
  const trashTodo = useTodoStore((s) => s.trashTodo);
  const updateTodo = useTodoStore((s) => s.updateTodo);

  // Upcoming todos: has scheduledDate, not completed/cancelled/trashed/someday
  const upcomingTodos = useMemo(
    () =>
      todos.filter(
        (t) =>
          t.scheduledDate &&
          !t.isCompleted &&
          !t.isCancelled &&
          !t.isTrashed &&
          !t.isSomeday,
      ),
    [todos],
  );

  const sections: UpcomingSection[] = useMemo(() => {
    const today = startOfDay(new Date());
    const result: UpcomingSection[] = [];

    // Next 7 days individually
    for (let offset = 1; offset <= 7; offset++) {
      const date = addDays(today, offset);
      const dayTodos = upcomingTodos
        .filter((t) => {
          if (!t.scheduledDate) return false;
          return isSameDay(new Date(t.scheduledDate), date);
        })
        .sort((a, b) => a.sortOrder - b.sortOrder);

      result.push({
        id: `day-${offset}`,
        title: dayTitle(date, offset),
        dateLabel: shortDateFmt.format(date),
        todos: dayTodos,
      });
    }

    // Weeks 2-4 beyond the first 7 days
    const weekBoundary = addDays(today, 8);
    for (let weekOffset = 2; weekOffset <= 4; weekOffset++) {
      const weekStart = addDays(weekBoundary, (weekOffset - 2) * 7);
      const weekEnd = addDays(weekStart, 7);

      const weekTodos = upcomingTodos
        .filter((t) => {
          if (!t.scheduledDate) return false;
          const d = new Date(t.scheduledDate);
          return d >= weekStart && d < weekEnd;
        })
        .sort((a, b) => {
          const da = a.scheduledDate ?? "";
          const db = b.scheduledDate ?? "";
          return da.localeCompare(db);
        });

      if (weekTodos.length > 0) {
        const wn = weekOfYear(weekStart);
        result.push({
          id: `week-${wn}`,
          title: `Неделя ${wn}`,
          dateLabel: shortDateFmt.format(weekStart),
          todos: weekTodos,
        });
      }
    }

    // Months beyond that, up to 1 year
    const monthBoundary = addDays(today, 30);
    const yearBoundary = addDays(today, 365);
    let cursor = new Date(
      monthBoundary.getFullYear(),
      monthBoundary.getMonth(),
      1,
    );

    while (cursor < yearBoundary) {
      const nextMonth = new Date(
        cursor.getFullYear(),
        cursor.getMonth() + 1,
        1,
      );
      const monthTodos = upcomingTodos
        .filter((t) => {
          if (!t.scheduledDate) return false;
          const d = new Date(t.scheduledDate);
          return d >= cursor && d < nextMonth;
        })
        .sort((a, b) => {
          const da = a.scheduledDate ?? "";
          const db = b.scheduledDate ?? "";
          return da.localeCompare(db);
        });

      if (monthTodos.length > 0) {
        const label = capitalize(monthFmt.format(cursor));
        result.push({
          id: `month-${label}`,
          title: label,
          dateLabel: "",
          todos: monthTodos,
        });
      }
      cursor = nextMonth;
    }

    return result;
  }, [upcomingTodos]);

  const projectById = useMemo(() => {
    const map = new Map<string, string>();
    for (const p of projects) {
      map.set(p.id, p.title);
    }
    return map;
  }, [projects]);

  return (
    <div className="flex w-full min-w-0 flex-col">
      {/* Header */}
      <div className="flex items-center gap-2.5 px-7 pb-3 pt-6">
        <Calendar size={24} className="text-red-500" />
        <h1 className="text-2xl font-bold text-(--foreground) select-none">
          Планы
        </h1>
      </div>

      {/* Scrollable content */}
      <div className="scrollbar-gutter flex-1 overflow-y-auto">
        <div className="pb-20 pt-1">
          {sections.map((section) => (
            <div key={section.id}>
              {/* Section header */}
              <div className="sticky top-0 z-10 flex items-center gap-2 bg-(--background) px-7 py-2">
                <span className="text-sm font-bold text-(--foreground)">
                  {section.title}
                </span>
                {section.todos.length > 0 && (
                  <span className="text-xs font-medium text-(--muted-foreground)">
                    {section.todos.length}
                  </span>
                )}
                <div className="flex-1" />
                {section.dateLabel && (
                  <span className="text-xs text-(--muted-foreground)/60">
                    {section.dateLabel}
                  </span>
                )}
              </div>

              {/* Section content */}
              {section.todos.length === 0 ? (
                <div className="px-7 py-2 text-xs text-(--muted-foreground)/50">
                  Нет задач
                </div>
              ) : (
                <div className="flex flex-col">
                  {section.todos.map((todo) => (
                    <UpcomingTodoRow
                      key={todo.id}
                      todo={todo}
                      projectName={
                        todo.projectId
                          ? projectById.get(todo.projectId)
                          : undefined
                      }
                      onComplete={() => completeTodo(todo.id)}
                      onTrash={() => trashTodo(todo.id)}
                      onToggleToday={() =>
                        updateTodo(todo.id, { isToday: !todo.isToday })
                      }
                    />
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Todo row for upcoming view
// ---------------------------------------------------------------------------

function UpcomingTodoRow({
  todo,
  projectName,
  onComplete,
  onTrash,
  onToggleToday,
}: {
  todo: TodoItem;
  projectName?: string;
  onComplete: () => void;
  onTrash: () => void;
  onToggleToday: () => void;
}) {
  return (
    <div
      className="group flex items-center gap-3 px-7 py-2 hover:bg-(--secondary)"
      onContextMenu={(e) => {
        e.preventDefault();
        // Context menu actions could be extended here
      }}
    >
      {/* Checkbox */}
      <button type="button" onClick={onComplete} className="shrink-0">
        <div className="h-[18px] w-[18px] rounded-full border border-(--ring) transition-colors hover:bg-(--ring)" />
      </button>

      {/* Content */}
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm text-(--foreground)">{todo.title}</div>
        {projectName && (
          <div className="truncate text-xs text-(--muted-foreground)">
            {projectName}
          </div>
        )}
      </div>

      {/* Quick actions (visible on hover) */}
      <div className="flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
        <button
          type="button"
          onClick={onToggleToday}
          title="На сегодня"
          className={`rounded px-1.5 py-0.5 text-[10px] transition-colors ${
            todo.isToday
              ? "bg-yellow-500/15 text-yellow-500"
              : "text-(--muted-foreground) hover:bg-(--accent)"
          }`}
        >
          Сегодня
        </button>
        <button
          type="button"
          onClick={onTrash}
          title="В корзину"
          className="rounded px-1.5 py-0.5 text-[10px] text-(--muted-foreground) hover:bg-red-500/15 hover:text-red-500"
        >
          Удалить
        </button>
      </div>
    </div>
  );
}

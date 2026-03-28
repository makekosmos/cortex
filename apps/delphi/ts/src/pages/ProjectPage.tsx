import { useParams, useNavigate } from "react-router-dom";
import { useEffect, useRef, useState } from "react";
import { Circle, MoreHorizontal } from "lucide-react";
import useTodoStore from "@/store/todos";
import { ProjectStatus } from "@/types/task";

/** Map colorTag strings to Tailwind text color classes. */
function colorTagClass(colorTag?: string | null): string {
  switch (colorTag) {
    case "red":
      return "text-red-500";
    case "orange":
      return "text-orange-500";
    case "yellow":
      return "text-yellow-500";
    case "green":
      return "text-green-500";
    case "blue":
      return "text-blue-500";
    case "purple":
      return "text-purple-500";
    case "pink":
      return "text-pink-500";
    default:
      return "text-(--muted-foreground)";
  }
}
import TodoRow from "@/components/TodoRow";
import { arkSync, projectToArkChange } from "@/services/sync/ark-client";

export default function ProjectPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const projects = useTodoStore((s) => s.projects);
  const updateProject = useTodoStore((s) => s.updateProject);
  const removeProject = useTodoStore((s) => s.removeProject);
  const todosForProject = useTodoStore((s) => s.todosForProject);
  const completeTodo = useTodoStore((s) => s.completeTodo);
  const trashTodo = useTodoStore((s) => s.trashTodo);

  const project = projects.find((p) => p.id === id);
  const todos = id ? todosForProject(id) : [];

  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState("");
  const [menuOpen, setMenuOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [editing]);

  // Close menu on outside click
  useEffect(() => {
    if (!menuOpen) return;
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [menuOpen]);

  if (!project) {
    return (
      <div className="flex w-full min-w-0 flex-col items-center justify-center">
        <p className="text-(--muted-foreground)">Проект не найден</p>
      </div>
    );
  }

  const startRename = () => {
    setEditTitle(project.title);
    setEditing(true);
    setMenuOpen(false);
  };

  const commitRename = () => {
    setEditing(false);
    const trimmed = editTitle.trim();
    if (trimmed && trimmed !== project.title) {
      updateProject(project.id, { title: trimmed });
      arkSync.sendChange(
        projectToArkChange({ ...project, title: trimmed }, "update"),
      );
    }
  };

  const handleDelete = () => {
    setMenuOpen(false);
    removeProject(project.id);
    arkSync.sendChange(projectToArkChange(project, "delete"));
    navigate("/");
  };

  const handleArchive = () => {
    setMenuOpen(false);
    updateProject(project.id, { status: ProjectStatus.Completed });
    arkSync.sendChange(
      projectToArkChange(
        { ...project, status: ProjectStatus.Completed },
        "update",
      ),
    );
    navigate("/");
  };

  const activeTodos = todos.filter((t) => !t.isCompleted && !t.isCancelled);
  const completedTodos = todos.filter((t) => t.isCompleted || t.isCancelled);

  return (
    <div className="flex w-full min-w-0 flex-col">
      {/* Header */}
      <div className="flex items-center gap-2.5 px-7 pb-3 pt-6">
        <Circle
          size={12}
          className={`shrink-0 fill-current ${colorTagClass(project.colorTag)}`}
        />

        {editing ? (
          <input
            ref={inputRef}
            type="text"
            value={editTitle}
            onChange={(e) => setEditTitle(e.target.value)}
            onBlur={commitRename}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitRename();
              if (e.key === "Escape") setEditing(false);
            }}
            className="flex-1 bg-transparent text-2xl font-bold text-(--foreground) outline-none"
          />
        ) : (
          <h1
            className="text-2xl font-bold text-(--foreground) select-none cursor-pointer"
            onDoubleClick={startRename}
          >
            {project.title}
          </h1>
        )}

        {todos.length > 0 && (
          <span className="text-sm text-(--muted-foreground)">
            {activeTodos.length}
          </span>
        )}

        {/* Context menu */}
        <div className="relative ml-auto" ref={menuRef}>
          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            className="rounded p-1 text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)"
          >
            <MoreHorizontal size={18} />
          </button>

          {menuOpen && (
            <div className="absolute right-0 top-full z-50 mt-1 w-44 rounded-lg border border-(--border) bg-(--popover) py-1 shadow-lg">
              <button
                type="button"
                onClick={startRename}
                className="flex w-full items-center px-3 py-2 text-sm text-(--foreground) hover:bg-(--secondary)"
              >
                Переименовать
              </button>
              <button
                type="button"
                onClick={handleArchive}
                className="flex w-full items-center px-3 py-2 text-sm text-(--foreground) hover:bg-(--secondary)"
              >
                Архивировать
              </button>
              <div className="my-1 border-t border-(--border)" />
              <button
                type="button"
                onClick={handleDelete}
                className="flex w-full items-center px-3 py-2 text-sm text-red-500 hover:bg-red-500/10"
              >
                Удалить
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Task list */}
      <div className="scrollbar-gutter flex-1 overflow-y-auto">
        <div className="pb-20 pt-1">
          {activeTodos.length === 0 && completedTodos.length === 0 ? (
            <div className="px-7 py-10 text-center text-sm text-(--muted-foreground)/60">
              Нет задач в проекте
            </div>
          ) : (
            <>
              {activeTodos.length > 0 && (
                <div className="flex flex-col">
                  {activeTodos.map((todo) => (
                    <TodoRow
                      key={todo.id}
                      todo={todo}
                      onComplete={() => completeTodo(todo.id)}
                      onTrash={() => trashTodo(todo.id)}
                    />
                  ))}
                </div>
              )}

              {completedTodos.length > 0 && (
                <>
                  <div className="px-7 pb-1 pt-4 text-xs font-semibold uppercase tracking-wider text-(--muted-foreground)/60 select-none">
                    Завершённые ({completedTodos.length})
                  </div>
                  <div className="flex flex-col">
                    {completedTodos.map((todo) => (
                      <TodoRow
                        key={todo.id}
                        todo={todo}
                        onComplete={() => completeTodo(todo.id)}
                      />
                    ))}
                  </div>
                </>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

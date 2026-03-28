import { Calendar, Folder, Moon, Star, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import useTodoStore from "@/store/todos";

export default function QuickEntry() {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [notes, setNotes] = useState("");
  const [isToday, setIsToday] = useState(false);
  const [isEvening, setIsEvening] = useState(false);
  const [scheduledDate, setScheduledDate] = useState("");
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    null,
  );
  const [showProjectMenu, setShowProjectMenu] = useState(false);

  const titleRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const projectMenuRef = useRef<HTMLDivElement>(null);

  const addTodo = useTodoStore((s) => s.addTodo);
  const projects = useTodoStore((s) => s.projects);

  const selectedProject = projects.find((p) => p.id === selectedProjectId);

  const reset = useCallback(() => {
    setTitle("");
    setNotes("");
    setIsToday(false);
    setIsEvening(false);
    setScheduledDate("");
    setSelectedProjectId(null);
    setShowProjectMenu(false);
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    reset();
  }, [reset]);

  const save = useCallback(() => {
    const trimmed = title.trim();
    if (!trimmed) {
      close();
      return;
    }

    addTodo({
      title: trimmed,
      notes: notes || null,
      isToday,
      isEvening,
      scheduledDate: scheduledDate || null,
      projectId: selectedProjectId,
    });

    close();
  }, [
    title,
    notes,
    isToday,
    isEvening,
    scheduledDate,
    selectedProjectId,
    addTodo,
    close,
  ]);

  // Cmd+N to toggle
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "n" && !e.shiftKey && !e.altKey) {
        e.preventDefault();
        setOpen((prev) => {
          if (prev) {
            reset();
            return false;
          }
          return true;
        });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [reset]);

  // Focus title when opened
  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => titleRef.current?.focus());
    }
  }, [open]);

  // Close project menu on outside click
  useEffect(() => {
    if (!showProjectMenu) return;
    const handler = (e: MouseEvent) => {
      if (
        projectMenuRef.current &&
        !projectMenuRef.current.contains(e.target as Node)
      ) {
        setShowProjectMenu(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showProjectMenu]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40" onClick={close} />

      {/* Panel */}
      <div
        ref={panelRef}
        className="relative z-10 w-[480px] rounded-2xl border border-(--border) bg-(--popover) shadow-2xl"
      >
        <div className="flex flex-col gap-3 p-5">
          {/* Title */}
          <input
            ref={titleRef}
            type="text"
            placeholder="Новая задача"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                save();
              }
              if (e.key === "Escape") {
                e.preventDefault();
                close();
              }
            }}
            className="w-full text-lg font-semibold text-(--foreground) placeholder:text-(--muted-foreground)"
          />

          {/* Notes */}
          <textarea
            placeholder="Заметки"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            rows={2}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                close();
              }
            }}
            className="w-full resize-none text-sm text-(--muted-foreground) placeholder:text-(--muted-foreground)/60"
          />

          {/* Divider */}
          <div className="h-px bg-(--border) opacity-50" />

          {/* Metadata row */}
          <div className="flex flex-wrap items-center gap-2">
            {/* Schedule date */}
            <label className="flex cursor-pointer items-center gap-1.5 rounded-full bg-(--secondary) px-3 py-1.5 text-xs transition-colors hover:bg-(--accent)">
              <Calendar
                size={12}
                className={
                  scheduledDate ? "text-blue-500" : "text-(--muted-foreground)"
                }
              />
              <input
                type="date"
                value={scheduledDate}
                onChange={(e) => setScheduledDate(e.target.value)}
                className="w-[100px] cursor-pointer bg-transparent text-xs"
              />
            </label>

            {/* Today */}
            <button
              type="button"
              onClick={() => {
                setIsToday((prev) => !prev);
                if (!isToday) setIsEvening(false);
              }}
              className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors ${
                isToday
                  ? "bg-yellow-500/15 text-yellow-500"
                  : "bg-(--secondary) text-(--muted-foreground) hover:bg-(--accent)"
              }`}
            >
              <Star size={12} />
              <span>Сегодня</span>
            </button>

            {/* Evening */}
            <button
              type="button"
              onClick={() => {
                setIsEvening((prev) => !prev);
                if (!isEvening) setIsToday(true);
              }}
              className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors ${
                isEvening
                  ? "bg-indigo-500/15 text-indigo-400"
                  : "bg-(--secondary) text-(--muted-foreground) hover:bg-(--accent)"
              }`}
            >
              <Moon size={12} />
              <span>Вечер</span>
            </button>

            <div className="flex-1" />

            {/* Project picker */}
            <div className="relative" ref={projectMenuRef}>
              <button
                type="button"
                onClick={() => setShowProjectMenu((prev) => !prev)}
                className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs transition-colors ${
                  selectedProject
                    ? "bg-blue-500/15 text-blue-500"
                    : "bg-(--secondary) text-(--muted-foreground) hover:bg-(--accent)"
                }`}
              >
                <Folder size={12} />
                <span>{selectedProject?.title ?? "Входящие"}</span>
              </button>

              {showProjectMenu && (
                <div className="absolute right-0 top-full z-20 mt-1 min-w-[180px] rounded-lg border border-(--border) bg-(--popover) py-1 shadow-lg">
                  <button
                    type="button"
                    onClick={() => {
                      setSelectedProjectId(null);
                      setShowProjectMenu(false);
                    }}
                    className="w-full px-3 py-1.5 text-left text-xs text-(--foreground) hover:bg-(--accent)"
                  >
                    Входящие
                  </button>
                  {projects.map((project) => (
                    <button
                      key={project.id}
                      type="button"
                      onClick={() => {
                        setSelectedProjectId(project.id);
                        setShowProjectMenu(false);
                      }}
                      className="w-full px-3 py-1.5 text-left text-xs text-(--foreground) hover:bg-(--accent)"
                    >
                      {project.title}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Close button */}
        <button
          type="button"
          onClick={close}
          className="absolute right-3 top-3 rounded-md p-1 text-(--muted-foreground) hover:bg-(--accent) hover:text-(--foreground)"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}

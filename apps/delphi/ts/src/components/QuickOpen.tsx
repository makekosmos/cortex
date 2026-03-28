import { CheckCircle, Folder, Search, Tag } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import useTodoStore from "@/store/todos";
import type { TodoItem, Project, Tag as TagType } from "@/types/task";

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

type QuickOpenResult =
  | { kind: "todo"; item: TodoItem }
  | { kind: "project"; item: Project }
  | { kind: "tag"; item: TagType };

function resultId(r: QuickOpenResult): string {
  return `${r.kind}-${r.item.id}`;
}

// ---------------------------------------------------------------------------
// Fuzzy search (simple substring / token match, case-insensitive)
// ---------------------------------------------------------------------------

function fuzzyScore(text: string, query: string): number {
  const lower = text.toLowerCase();
  const q = query.toLowerCase();

  // Exact substring match
  if (lower.includes(q)) return 1;

  // Token match: all query words present
  const queryTokens = q.split(/\s+/).filter(Boolean);
  if (
    queryTokens.length > 1 &&
    queryTokens.every((tok) => lower.includes(tok))
  ) {
    return 0.8;
  }

  // Prefix match on any word
  const textTokens = lower.split(/\s+/);
  if (queryTokens.some((qt) => textTokens.some((tt) => tt.startsWith(qt)))) {
    return 0.5;
  }

  return 0;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function QuickOpen() {
  const [open, setOpen] = useState(false);
  const [searchText, setSearchText] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);

  const inputRef = useRef<HTMLInputElement>(null);

  const todos = useTodoStore((s) => s.todos);
  const projects = useTodoStore((s) => s.projects);
  const tags = useTodoStore((s) => s.tags);

  const close = useCallback(() => {
    setOpen(false);
    setSearchText("");
    setDebouncedQuery("");
    setSelectedIndex(0);
  }, []);

  // Cmd+K to toggle
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "k") {
        e.preventDefault();
        setOpen((prev) => {
          if (prev) {
            setSearchText("");
            setDebouncedQuery("");
            setSelectedIndex(0);
            return false;
          }
          return true;
        });
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Focus on open
  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  // Debounce search text (200ms)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(searchText);
      setSelectedIndex(0);
    }, 200);
    return () => clearTimeout(timer);
  }, [searchText]);

  // Compute results
  const results: QuickOpenResult[] = useMemo(() => {
    if (!debouncedQuery.trim()) return [];
    const q = debouncedQuery.trim();
    const items: QuickOpenResult[] = [];

    // Todos (top 8)
    const scoredTodos = todos
      .filter((t) => !t.isTrashed)
      .map((t) => {
        const titleScore = fuzzyScore(t.title, q);
        const notesScore = t.notes ? fuzzyScore(t.notes, q) * 0.8 : 0;
        return { todo: t, score: Math.max(titleScore, notesScore) };
      })
      .filter((x) => x.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 8);

    items.push(
      ...scoredTodos.map((x) => ({ kind: "todo" as const, item: x.todo })),
    );

    // Projects (top 3)
    const matchedProjects = projects
      .filter((p) => fuzzyScore(p.title, q) > 0)
      .slice(0, 3);
    items.push(
      ...matchedProjects.map((p) => ({ kind: "project" as const, item: p })),
    );

    // Tags (top 3)
    const matchedTags = tags
      .filter((t) => fuzzyScore(t.title, q) > 0)
      .slice(0, 3);
    items.push(...matchedTags.map((t) => ({ kind: "tag" as const, item: t })));

    return items;
  }, [debouncedQuery, todos, projects, tags]);

  const activateSelected = useCallback(() => {
    if (selectedIndex >= results.length) return;
    const result = results[selectedIndex];

    // Navigate to relevant view based on result type
    if (result.kind === "todo") {
      // Could dispatch navigation event; for now just close
      console.log("Selected todo:", result.item.id);
    } else if (result.kind === "project") {
      console.log("Selected project:", result.item.id);
    } else if (result.kind === "tag") {
      console.log("Selected tag:", result.item.id);
    }

    close();
  }, [selectedIndex, results, close]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.min(results.length - 1, prev + 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => Math.max(0, prev - 1));
      } else if (e.key === "Enter") {
        e.preventDefault();
        activateSelected();
      } else if (e.key === "Escape") {
        e.preventDefault();
        close();
      }
    },
    [results.length, activateSelected, close],
  );

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[18vh]">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40" onClick={close} />

      {/* Panel */}
      <div className="relative z-10 w-[480px] overflow-hidden rounded-xl border border-(--border) bg-(--popover) shadow-2xl">
        {/* Search input */}
        <div className="flex items-center gap-2.5 px-4 py-3.5">
          <Search size={16} className="shrink-0 text-(--muted-foreground)" />
          <input
            ref={inputRef}
            type="text"
            placeholder="Поиск задач, проектов, тегов..."
            value={searchText}
            onChange={(e) => setSearchText(e.target.value)}
            onKeyDown={handleKeyDown}
            className="w-full text-base text-(--foreground) placeholder:text-(--muted-foreground)"
          />
        </div>

        {/* Results */}
        {results.length > 0 && (
          <>
            <div className="h-px bg-(--border) opacity-50" />
            <div className="max-h-[280px] overflow-y-auto py-1">
              {results.map((result, index) => (
                <ResultRow
                  key={resultId(result)}
                  result={result}
                  isSelected={index === selectedIndex}
                  projects={projects}
                  todos={todos}
                  onClick={() => {
                    setSelectedIndex(index);
                    activateSelected();
                  }}
                  onMouseEnter={() => setSelectedIndex(index)}
                />
              ))}
            </div>
          </>
        )}

        {/* Empty state */}
        {results.length === 0 && debouncedQuery.trim() !== "" && (
          <div className="px-4 py-3.5 text-center text-sm text-(--muted-foreground)">
            Ничего не найдено
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Result row
// ---------------------------------------------------------------------------

function ResultRow({
  result,
  isSelected,
  projects,
  todos,
  onClick,
  onMouseEnter,
}: {
  result: QuickOpenResult;
  isSelected: boolean;
  projects: Project[];
  todos: TodoItem[];
  onClick: () => void;
  onMouseEnter: () => void;
}) {
  const icon = (() => {
    switch (result.kind) {
      case "todo":
        return <CheckCircle size={16} className="text-blue-500" />;
      case "project":
        return <Folder size={16} className="text-purple-500" />;
      case "tag":
        return <Tag size={16} className="text-orange-500" />;
    }
  })();

  const title = result.item.title;

  const subtitle = (() => {
    switch (result.kind) {
      case "todo": {
        const todo = result.item as TodoItem;
        if (todo.isCompleted) return "Завершена";
        if (todo.projectId) {
          const project = projects.find((p) => p.id === todo.projectId);
          return project?.title ?? "Входящие";
        }
        return "Входящие";
      }
      case "project": {
        const project = result.item as Project;
        const count = todos.filter(
          (t) => t.projectId === project.id && !t.isTrashed,
        ).length;
        return `${count} задач`;
      }
      case "tag": {
        const tag = result.item;
        const count = todos.filter((t) => t.tagIds.includes(tag.id)).length;
        return `${count} задач`;
      }
    }
  })();

  return (
    <div
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      className={`flex cursor-pointer items-center gap-2.5 px-4 py-2 ${
        isSelected ? "bg-blue-500/10" : ""
      }`}
    >
      <div className="shrink-0">{icon}</div>
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm text-(--foreground)">{title}</div>
        <div className="truncate text-xs text-(--muted-foreground)">
          {subtitle}
        </div>
      </div>
    </div>
  );
}

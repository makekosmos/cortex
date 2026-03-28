import { useState, useEffect } from "react";
import {
  Archive,
  Book,
  Calendar,
  Circle,
  Inbox,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  Star,
} from "lucide-react";
import { Link, useLocation } from "react-router-dom";
import SideBarButton from "@/components/SideBarButton";
import useTodoStore from "@/store/todos";
import { ProjectStatus } from "@/types/task";

function colorTagClass(colorTag?: string | null): string {
  switch (colorTag) {
    case "red": return "text-red-500";
    case "orange": return "text-orange-500";
    case "yellow": return "text-yellow-500";
    case "green": return "text-green-500";
    case "blue": return "text-blue-500";
    case "purple": return "text-purple-500";
    case "pink": return "text-pink-500";
    default: return "text-(--muted-foreground)";
  }
}

export default function SideBar() {
  const projects = useTodoStore((s) => s.projects);
  const activeProjects = projects
    .filter((p) => p.status === ProjectStatus.Active)
    .sort((a, b) => a.sortOrder - b.sortOrder);
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);

  // Cmd+/ to toggle
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "/") {
        e.preventDefault();
        setCollapsed((prev) => !prev);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  if (collapsed) {
    return (
      <aside className="flex min-h-0 flex-col items-center border-r border-r-(--border) bg-(--sidebar) py-2 px-1">
        <div
          style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
          className="h-3 w-full shrink-0"
        />
        <button
          type="button"
          onClick={() => setCollapsed(false)}
          className="mb-2 rounded-lg p-2 text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)"
          title="Показать сайдбар (⌘/)"
        >
          <PanelLeftOpen size={18} />
        </button>
        <SideBarButton icon={Inbox} to="/" />
        <SideBarButton icon={Star} to="/today" />
        <SideBarButton icon={Calendar} to="/upcoming" />
        <SideBarButton icon={Book} to="/logbook" />
        <SideBarButton icon={Archive} to="/trash" />
        <div className="flex-1" />
        <SideBarButton icon={Settings} to="/settings" />
      </aside>
    );
  }

  return (
    <aside className="left-0 flex min-h-0 w-48 flex-col justify-between border-r border-r-(--border) bg-(--sidebar) p-2">
      {/* Drag region + collapse button */}
      <div className="flex items-center">
        <div
          style={{ WebkitAppRegion: "drag" } as React.CSSProperties}
          className="h-7 flex-1"
        />
        <button
          type="button"
          onClick={() => setCollapsed(true)}
          className="rounded-lg p-1.5 text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)"
          title="Скрыть сайдбар (⌘/)"
        >
          <PanelLeftClose size={16} />
        </button>
      </div>

      <div className="flex flex-1 flex-col overflow-y-auto">
        <SideBarButton icon={Inbox} to="/" label="Входящие" />
        <SideBarButton icon={Star} to="/today" label="Сегодня" />
        <SideBarButton icon={Calendar} to="/upcoming" label="Планы" />
        <SideBarButton icon={Book} to="/logbook" label="Журнал" />
        <SideBarButton icon={Archive} to="/trash" label="Корзина" />

        {activeProjects.length > 0 && (
          <>
            <div className="my-2 border-t border-(--border)" />
            <div className="px-3 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wider text-(--muted-foreground)/60 select-none">
              Проекты
            </div>
            {activeProjects.map((project) => {
              const path = `/project/${project.id}`;
              const isActive = location.pathname === path;
              const base =
                "flex h-10 w-full cursor-pointer items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors";
              const activeClass = isActive
                ? "bg-(--accent) text-(--foreground) font-medium"
                : "text-(--muted-foreground) hover:bg-(--secondary) hover:text-(--foreground)";
              return (
                <Link key={project.id} to={path} className={`${base} ${activeClass}`}>
                  <Circle size={10} className={`shrink-0 fill-current ${colorTagClass(project.colorTag)}`} />
                  <span className="truncate">{project.title}</span>
                </Link>
              );
            })}
          </>
        )}
      </div>

      <div className="flex flex-col gap-2">
        <SideBarButton icon={Settings} to="/settings" />
      </div>
    </aside>
  );
}

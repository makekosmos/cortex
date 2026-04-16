import {
  BarChart2,
  ChevronLeft,
  ChevronRight,
  FolderSearch,
  Gamepad2,
  Monitor,
  Settings,
  Star,
} from "lucide-react";
import { useEffect, useState } from "react";
import { NavLink, useLocation } from "react-router-dom";
import { useLanguage } from "@/components/language-provider";
import { cn } from "@/lib/utils";
import { useGamesState } from "@/store/GamesContext";

const SIDEBAR_STORAGE_KEY = "arrancador_sidebar_collapsed";

const navItems = [
  { key: "sidebar.library" as const, to: "/", icon: Gamepad2 },
  { key: "sidebar.scan" as const, to: "/scan", icon: FolderSearch },
  { key: "sidebar.statistics" as const, to: "/statistics", icon: BarChart2 },
  { key: "sidebar.system" as const, to: "/system", icon: Monitor },
];

export function Sidebar() {
  const { t } = useLanguage();
  const [collapsed, setCollapsed] = useState(() => {
    try {
      if (typeof localStorage === "undefined") return false;
      return localStorage.getItem(SIDEBAR_STORAGE_KEY) === "true";
    } catch {
      return false;
    }
  });
  const { favorites } = useGamesState();
  const location = useLocation();
  const sidebarWidthClass = collapsed
    ? "lg:w-[72px] lg:min-w-[72px]"
    : "lg:w-[260px] lg:min-w-[260px]";
  const navItemLayoutClass = collapsed ? "lg:justify-center lg:px-2" : "lg:px-3";

  useEffect(() => {
    try {
      if (typeof localStorage === "undefined") return;
      localStorage.setItem(SIDEBAR_STORAGE_KEY, String(collapsed));
    } catch {
      // Ignore write failures.
    }
  }, [collapsed]);

  return (
    <aside
      className={cn(
        "h-screen flex flex-col border-r border-border/70 bg-sidebar/95 text-sidebar-foreground flex-none",
        "supports-[backdrop-filter]:bg-sidebar/90 backdrop-blur-xl",
        "transition-[width] duration-200 ease-out relative z-50 shadow-[0_1px_0_rgba(255,255,255,0.02),0_24px_60px_rgba(0,0,0,0.28)]",
        "w-[280px] sm:w-[320px]",
        sidebarWidthClass,
      )}
    >
      <nav
        aria-label="Primary"
        className="flex-1 py-4 px-2 space-y-1 overflow-y-auto scrollbar-stable"
      >
        {navItems.map(({ key, to, icon: Icon }) => {
          const title = t(key);
          const isActive =
            location.pathname === to || (to === "/" && location.pathname.startsWith("/game/"));

          return (
            <NavLink
              key={to}
              to={to}
              title={title}
              aria-label={title}
            className={cn(
                "group relative flex items-center gap-3 rounded-md px-3 py-2 text-sm font-[510] transition-colors min-w-0",
                navItemLayoutClass,
                "text-sidebar-foreground/78 hover:bg-sidebar-accent/80 hover:text-sidebar-accent-foreground",
                isActive &&
                  "bg-sidebar-accent text-sidebar-accent-foreground shadow-[0_1px_0_rgba(255,255,255,0.02),0_10px_24px_rgba(0,0,0,0.18)]",
              )}
            >
              <Icon className="w-5 h-5 flex-shrink-0" />
              <span className={cn("truncate", collapsed && "lg:hidden")}>
                {title}
              </span>
            </NavLink>
          );
        })}

        {favorites.length > 0 && (
          <div className="mt-4 border-t border-border/70 pt-4">
            <div
              className={cn(
                "px-3 py-2 text-[11px] font-[510] uppercase tracking-[0.16em] text-muted-foreground",
                collapsed && "lg:sr-only",
              )}
            >
              {t("sidebar.favorites")}
            </div>
            {favorites.slice(0, 5).map((game) => (
              <NavLink
                key={game.id}
                to={`/game/${game.id}`}
                title={game.name}
                aria-label={game.name}
                className={cn(
                  "group flex items-center gap-3 rounded-md px-3 py-2 text-sm font-[510] transition-colors min-w-0",
                  collapsed && "lg:justify-center lg:px-2",
                  "text-sidebar-foreground/75 hover:bg-sidebar-accent/80 hover:text-sidebar-accent-foreground",
                  location.pathname === `/game/${game.id}` &&
                    "bg-sidebar-accent text-sidebar-accent-foreground shadow-[0_1px_0_rgba(255,255,255,0.02),0_8px_20px_rgba(0,0,0,0.16)]",
                )}
              >
                <Star className="w-4 h-4 flex-shrink-0 text-yellow-500" />
                <span className={cn("truncate", collapsed && "lg:hidden")}>
                  {game.name}
                </span>
              </NavLink>
            ))}
            {favorites.length > 5 && (
              <div className={cn("px-3 py-1 text-xs text-muted-foreground", collapsed && "lg:hidden")}>
                +{favorites.length - 5} {t("sidebar.more")}
              </div>
            )}
          </div>
        )}
      </nav>

      <div className="p-3 border-t border-border/70">
        <div
          className={cn(
            "flex items-center gap-2 justify-between px-1",
            collapsed && "lg:flex-col lg:justify-center lg:px-0",
          )}
        >
          <div className={cn("flex items-center", collapsed && "lg:justify-center")}>
            <NavLink
              to="/settings"
              aria-label={t("sidebar.settings")}
              title={t("sidebar.settings")}
              className={cn(
                "group flex items-center gap-3 rounded-md px-3 py-2 text-sm font-[510] transition-colors",
                "text-sidebar-foreground/80 hover:bg-sidebar-accent/80 hover:text-sidebar-accent-foreground",
                location.pathname === "/settings" &&
                  "bg-sidebar-accent text-sidebar-accent-foreground",
                collapsed && "lg:justify-center lg:px-2",
              )}
            >
              <Settings className="w-5 h-5 flex-shrink-0" />
              <span className={cn("truncate", collapsed && "lg:hidden")}>
                {t("sidebar.settings")}
              </span>
            </NavLink>
          </div>
          <button
            type="button"
            onClick={() => setCollapsed(!collapsed)}
            className="hidden lg:flex h-9 w-9 items-center justify-center rounded-md border border-border/70 bg-card/70 text-sidebar-foreground transition-colors hover:bg-card"
            title={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
            aria-label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
          >
            {collapsed ? (
              <ChevronRight className="w-4 h-4" />
            ) : (
              <ChevronLeft className="w-4 h-4" />
            )}
          </button>
        </div>
      </div>
    </aside>
  );
}

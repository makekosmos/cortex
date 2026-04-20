import {
  BarChart2,
  FolderSearch,
  Gamepad2,
  Monitor,
  PanelLeftClose,
  Settings,
  Star,
} from "lucide-react";
import type { MouseEvent as ReactMouseEvent } from "react";
import { useEffect, useMemo, useRef, useState } from "react";
import { NavLink, useLocation } from "react-router-dom";
import Spotlight from "@/components/Spotlight";
import { useLanguage } from "@/components/language-provider";
import { cn } from "@/lib/utils";
import { useGamesState } from "@/store/GamesContext";

export type SidebarConfig = {
  width: number;
  hidden: boolean;
};

export const SIDEBAR_STORAGE_KEY = "arrancador-sidebar-config";
export const SIDEBAR_DEFAULT_WIDTH = 200;
export const SIDEBAR_MIN_WIDTH = 160;
export const SIDEBAR_MAX_WIDTH = 320;

type SidebarProps = {
  hidden?: boolean;
  onHiddenChange?: (hidden: boolean) => void;
  onConfigChange?: (config: SidebarConfig) => void;
  initialConfig?: Partial<SidebarConfig>;
  showToggle?: boolean;
  showSearch?: boolean;
  reserveTopInset?: boolean;
  enableToggleShortcut?: boolean;
  mobile?: boolean;
  className?: string;
};

const navItems = [
  { key: "sidebar.library" as const, to: "/", icon: Gamepad2 },
  { key: "sidebar.scan" as const, to: "/scan", icon: FolderSearch },
  { key: "sidebar.statistics" as const, to: "/statistics", icon: BarChart2 },
  { key: "sidebar.system" as const, to: "/system", icon: Monitor },
];

function sanitizeSidebarWidth(width?: number): number {
  if (typeof width !== "number" || !Number.isFinite(width)) {
    return SIDEBAR_DEFAULT_WIDTH;
  }

  return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, width));
}

export function sanitizeSidebarConfig(
  config?: Partial<SidebarConfig>,
): SidebarConfig {
  return {
    width: sanitizeSidebarWidth(config?.width),
    hidden: config?.hidden ?? false,
  };
}

function loadInitialConfig(
  explicitConfig?: Partial<SidebarConfig>,
): SidebarConfig {
  if (explicitConfig?.width !== undefined || explicitConfig?.hidden !== undefined) {
    return sanitizeSidebarConfig(explicitConfig);
  }

  try {
    if (typeof localStorage === "undefined") {
      return sanitizeSidebarConfig();
    }

    const raw = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    if (!raw) {
      return sanitizeSidebarConfig();
    }

    return sanitizeSidebarConfig(JSON.parse(raw) as Partial<SidebarConfig>);
  } catch {
    return sanitizeSidebarConfig();
  }
}

export function Sidebar({
  hidden,
  onHiddenChange,
  onConfigChange,
  initialConfig,
  showToggle = true,
  showSearch = true,
  reserveTopInset = true,
  enableToggleShortcut = true,
  mobile = false,
  className,
}: SidebarProps = {}) {
  const { t } = useLanguage();
  const initial = useMemo(() => loadInitialConfig(initialConfig), [initialConfig]);
  const [width, setWidth] = useState(initial.width);
  const [internalHidden, setInternalHidden] = useState(initial.hidden);
  const [isResizing, setIsResizing] = useState(false);
  const [animating, setAnimating] = useState(false);
  const { favorites } = useGamesState();
  const location = useLocation();
  const hiddenState = hidden ?? internalHidden;
  const widthRef = useRef(initial.width);
  const resizeFrameRef = useRef<number | null>(null);
  const animationTimerRef = useRef<number | null>(null);
  const pointerOriginRef = useRef({ x: 0, y: 0 });
  const isMac =
    typeof navigator !== "undefined" && navigator.platform.startsWith("Mac");
  const showCollapsedRail = hiddenState && !mobile && showToggle;
  const unmountDesktopContent = hiddenState && !mobile && !showToggle;

  useEffect(() => {
    if (hidden !== undefined) {
      setInternalHidden(hidden);
    }
  }, [hidden]);

  useEffect(() => {
    return () => {
      if (resizeFrameRef.current !== null) {
        cancelAnimationFrame(resizeFrameRef.current);
      }
      if (animationTimerRef.current !== null) {
        window.clearTimeout(animationTimerRef.current);
      }
      document.body.classList.remove("sidebar-resizing");
    };
  }, []);

  const persistConfig = (nextConfig: SidebarConfig) => {
    try {
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(SIDEBAR_STORAGE_KEY, JSON.stringify(nextConfig));
      }
    } catch {
      // Ignore write failures.
    }

    onConfigChange?.(nextConfig);
  };

  const startAnimation = () => {
    if (animationTimerRef.current !== null) {
      window.clearTimeout(animationTimerRef.current);
    }

    setAnimating(true);
    animationTimerRef.current = window.setTimeout(() => {
      setAnimating(false);
      animationTimerRef.current = null;
    }, 330);
  };

  const setHiddenState = (nextHidden: boolean) => {
    if (hidden === undefined) {
      setInternalHidden(nextHidden);
    }
    onHiddenChange?.(nextHidden);
    persistConfig({ width: widthRef.current, hidden: nextHidden });
  };

  const toggleHidden = () => {
    startAnimation();
    setHiddenState(!hiddenState);
  };

  useEffect(() => {
    if (!enableToggleShortcut || mobile) {
      return;
    }

    const onKeyDown = (event: KeyboardEvent) => {
      const rawKey = event.key.toLowerCase();
      const isToggleKey = rawKey === "b" || event.code === "KeyB";
      if (!isToggleKey) {
        return;
      }

      if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) {
        return;
      }

      event.preventDefault();
      toggleHidden();
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [enableToggleShortcut, hiddenState, mobile, width]);

  useEffect(() => {
    if (!isResizing) {
      return;
    }

    const onMouseMove = (event: MouseEvent) => {
      if (hiddenState) {
        return;
      }

      if (resizeFrameRef.current !== null) {
        cancelAnimationFrame(resizeFrameRef.current);
      }

      resizeFrameRef.current = window.requestAnimationFrame(() => {
        const nextWidth = Math.max(
          SIDEBAR_MIN_WIDTH,
          Math.min(SIDEBAR_MAX_WIDTH, event.clientX),
        );
        widthRef.current = nextWidth;
        setWidth(nextWidth);
      });
    };

    const onMouseUp = (event: MouseEvent) => {
      if (resizeFrameRef.current !== null) {
        cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }

      setIsResizing(false);
      document.body.classList.remove("sidebar-resizing");

      const dx = Math.abs(event.clientX - pointerOriginRef.current.x);
      const dy = Math.abs(event.clientY - pointerOriginRef.current.y);
      if (dx < 4 && dy < 4) {
        persistConfig({ width: widthRef.current, hidden: hiddenState });
        return;
      }

      persistConfig({ width: widthRef.current, hidden: hiddenState });
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [hiddenState, isResizing, width]);

  const handleResizeStart = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (mobile) {
      return;
    }

    event.preventDefault();
    pointerOriginRef.current = { x: event.clientX, y: event.clientY };
    setIsResizing(true);
    document.body.classList.add("sidebar-resizing");
  };

  const wrapperStyle = mobile
    ? undefined
    : showCollapsedRail
      ? { width: "44px" }
      : hiddenState
      ? { width: "0px" }
      : { width: `${width}px` };
  const hasTopBar = showToggle;

  return (
    <div
      data-testid="kepler-sidebar"
      className={cn(
        !mobile && "kepler-sidebar-wrapper",
        !mobile && showCollapsedRail && "is-hidden collapsed",
        !mobile && unmountDesktopContent && "hidden collapsed",
        !mobile && animating && "animating",
        !mobile && isResizing && "is-resizing",
        mobile && "relative flex h-full w-[320px] max-w-[86vw] flex-col",
        className,
      )}
      style={wrapperStyle}
    >
      <div className="kepler-sidebar-content">
        <aside
          className={cn(
            "arrancador-sidebar-shell",
            reserveTopInset &&
              !mobile &&
              isMac &&
              "arrancador-sidebar-shell--mac-safe-top",
            (hasTopBar || !mobile) && "arrancador-sidebar-shell--with-top-bar",
          )}
        >
          {showToggle && (
            <div className="arrancador-sidebar-top">
              {showToggle && (
                <button
                  type="button"
                  className="arrancador-sidebar-top-toggle"
                  data-testid="sidebar-toggle"
                  aria-label={
                    hiddenState
                      ? "Показать боковую панель"
                      : "Скрыть боковую панель"
                  }
                  title={
                    hiddenState
                      ? "Показать боковую панель"
                      : "Скрыть боковую панель"
                  }
                  onClick={toggleHidden}
                >
                  <PanelLeftClose className="h-4 w-4" />
                </button>
              )}
            </div>
          )}

          {!showCollapsedRail && (
            <>
              <div className="arrancador-sidebar-body">
                {showSearch && (
                  <>
                    <Spotlight
                      enableShortcut={false}
                      triggerVariant="sidebar"
                      triggerClassName="arrancador-sidebar-search-button"
                    />
                    <div className="kepler-sidebar-divider" />
                  </>
                )}

                {navItems.map(({ key, to, icon: Icon }) => {
                  const title = t(key);
                  const isActive =
                    location.pathname === to ||
                    (to === "/" && location.pathname.startsWith("/game/"));

                  return (
                    <NavLink
                      key={to}
                      to={to}
                      end={to === "/"}
                      title={title}
                      aria-label={title}
                      className={cn(
                        "kepler-sidebar-btn",
                        isActive && "kepler-sidebar-btn--active",
                      )}
                    >
                      <Icon className="h-[18px] w-[18px] shrink-0" />
                      <span className="truncate">{title}</span>
                    </NavLink>
                  );
                })}

                {favorites.length > 0 && (
                  <>
                    <div className="kepler-sidebar-divider" />
                    <div className="kepler-sidebar-section-label">
                      {t("sidebar.favorites")}
                    </div>
                    {favorites.slice(0, 5).map((game) => (
                      <NavLink
                        key={game.id}
                        to={`/game/${game.id}`}
                        title={game.name}
                        aria-label={game.name}
                        className={cn(
                          "kepler-sidebar-project-link widget-nav-item",
                          location.pathname === `/game/${game.id}` &&
                            "kepler-sidebar-project-link--active",
                        )}
                      >
                        <Star className="h-[14px] w-[14px] shrink-0 text-yellow-500" />
                        <span className="kepler-sidebar-project-label">
                          {game.name}
                        </span>
                      </NavLink>
                    ))}
                    {favorites.length > 5 && (
                      <div className="px-3 py-1 text-xs text-muted-foreground">
                        +{favorites.length - 5} {t("sidebar.more")}
                      </div>
                    )}
                  </>
                )}
              </div>

              <div className="kepler-sidebar-footer">
                <NavLink
                  to="/settings"
                  aria-label={t("sidebar.settings")}
                  title={t("sidebar.settings")}
                  className={cn(
                    "kepler-sidebar-btn",
                    location.pathname === "/settings" &&
                      "kepler-sidebar-btn--active",
                  )}
                >
                  <Settings className="h-[18px] w-[18px] shrink-0" />
                  <span className="truncate">{t("sidebar.settings")}</span>
                </NavLink>
              </div>
            </>
          )}
        </aside>
      </div>

      {!mobile && !hiddenState && (
        <div
          className="kepler-sidebar-resize-handle"
          data-testid="kepler-sidebar-resize-handle"
          onMouseDown={handleResizeStart}
        >
          <div className="kepler-resize-handle-line" />
        </div>
      )}
    </div>
  );
}

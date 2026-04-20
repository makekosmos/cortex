import { Menu, X } from "lucide-react";
import type { CSSProperties } from "react";
import { useEffect, useState } from "react";
import {
  Outlet,
  useLocation,
  useNavigate,
  useNavigationType,
} from "react-router-dom";

import { AppTitlebar } from "@/components/AppTitlebar";
import Spotlight from "@/components/Spotlight";
import {
  type SidebarConfig,
  SIDEBAR_STORAGE_KEY,
  Sidebar,
  sanitizeSidebarConfig,
} from "@/components/Sidebar";
import { ToastProvider } from "@/components/ToastProvider";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { GamesProvider } from "@/store/GamesContext";

function loadSidebarConfig(): SidebarConfig {
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

function readBrowserHistoryIndex() {
  if (typeof window === "undefined") {
    return 0;
  }

  const state = window.history.state as { idx?: unknown } | null;
  return typeof state?.idx === "number" ? state.idx : 0;
}

export default function Layout() {
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const [sidebarConfig, setSidebarConfig] = useState(loadSidebarConfig);
  const [historyIndex, setHistoryIndex] = useState(readBrowserHistoryIndex);
  const [maxHistoryIndex, setMaxHistoryIndex] = useState(readBrowserHistoryIndex);
  const location = useLocation();
  const navigate = useNavigate();
  const navigationType = useNavigationType();

  const currentPath = `${location.pathname}${location.search}${location.hash}`;

  useEffect(() => {
    setIsMobileMenuOpen(false);
  }, [currentPath]);

  useEffect(() => {
    const nextHistoryIndex = readBrowserHistoryIndex();
    setHistoryIndex(nextHistoryIndex);
    setMaxHistoryIndex((previousMaxHistoryIndex) =>
      navigationType === "PUSH"
        ? nextHistoryIndex
        : Math.max(previousMaxHistoryIndex, nextHistoryIndex),
    );
  }, [currentPath, navigationType]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsMobileMenuOpen(false);
      }
    };

    if (isMobileMenuOpen) {
      window.addEventListener("keydown", onKeyDown);
    }

    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isMobileMenuOpen]);

  const persistSidebarConfig = (nextConfig: SidebarConfig) => {
    const sanitizedConfig = sanitizeSidebarConfig(nextConfig);
    setSidebarConfig(sanitizedConfig);
    try {
      if (typeof localStorage !== "undefined") {
        localStorage.setItem(
          SIDEBAR_STORAGE_KEY,
          JSON.stringify(sanitizedConfig),
        );
      }
    } catch {
      // Ignore write failures.
    }
  };

  const setSidebarHidden = (hidden: boolean) => {
    persistSidebarConfig({
      ...sidebarConfig,
      hidden,
    });
  };

  const canGoBack = historyIndex > 0;
  const canGoForward = historyIndex < maxHistoryIndex;

  const navigateBack = () => {
    if (!canGoBack) {
      return;
    }

    navigate(-1);
  };

  const navigateForward = () => {
    if (!canGoForward) {
      return;
    }

    navigate(1);
  };

  return (
    <GamesProvider>
      <ToastProvider>
        <div className="kepler-desktop-chrome flex h-screen min-h-0 flex-col overflow-hidden bg-background text-foreground">
          <div className="hidden lg:block" data-testid="desktop-titlebar">
            <AppTitlebar
              sidebarHidden={sidebarConfig.hidden}
              onToggleSidebar={() => setSidebarHidden(!sidebarConfig.hidden)}
              canGoBack={canGoBack}
              canGoForward={canGoForward}
              onBack={navigateBack}
              onForward={navigateForward}
            />
          </div>

          <div className="kepler-desktop-chrome__body">
            <aside className="kepler-desktop-chrome__sidebar hidden lg:flex">
              <Sidebar
                hidden={sidebarConfig.hidden}
                initialConfig={sidebarConfig}
                onHiddenChange={setSidebarHidden}
                onConfigChange={persistSidebarConfig}
                showToggle={false}
                reserveTopInset={false}
              />
            </aside>

            <div className="kepler-desktop-chrome__content">
              <div
                className="fixed top-0 left-0 right-0 z-50 flex h-14 items-center justify-between border-b px-4 backdrop-blur-xl lg:hidden"
                style={{
                  borderColor: "var(--border)",
                  background:
                    "color-mix(in srgb, var(--sidebar-bg) 90%, transparent)",
                }}
              >
                <span className="text-sm font-[510] tracking-[-0.01em]">
                  Arrancador
                </span>
                <div className="flex items-center gap-2">
                  <Spotlight
                    enableShortcut={false}
                    triggerClassName="h-9 px-3"
                  />
                  <button
                    type="button"
                    data-testid="mobile-menu-toggle"
                    aria-label={
                      isMobileMenuOpen
                        ? "\u0417\u0430\u043a\u0440\u044b\u0442\u044c \u043c\u0435\u043d\u044e"
                        : "\u041e\u0442\u043a\u0440\u044b\u0442\u044c \u043c\u0435\u043d\u044e"
                    }
                    aria-controls="mobile-sidebar"
                    aria-expanded={isMobileMenuOpen}
                    onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
                    className="inline-flex h-9 w-9 items-center justify-center rounded-md border border-border/60 bg-card/70 transition-colors hover:bg-card"
                  >
                    {isMobileMenuOpen ? (
                      <X className="h-5 w-5" />
                    ) : (
                      <Menu className="h-5 w-5" />
                    )}
                  </button>
                </div>
              </div>

              <Sheet open={isMobileMenuOpen} onOpenChange={setIsMobileMenuOpen}>
                <SheetContent
                  id="mobile-sidebar"
                  data-testid="mobile-sidebar-sheet"
                  side="left"
                  className="w-[320px] max-w-[86vw] border-border/70 p-0 sm:max-w-[86vw]"
                >
                  <SheetHeader className="sr-only">
                    <SheetTitle>
                      {"\u041d\u0430\u0432\u0438\u0433\u0430\u0446\u0438\u044f"}
                    </SheetTitle>
                    <SheetDescription>
                      {
                        "\u041c\u043e\u0431\u0438\u043b\u044c\u043d\u0430\u044f \u0431\u043e\u043a\u043e\u0432\u0430\u044f \u043f\u0430\u043d\u0435\u043b\u044c Arrancador."
                      }
                    </SheetDescription>
                  </SheetHeader>
                  <Sidebar
                    mobile
                    hidden={false}
                    showToggle={false}
                    reserveTopInset={false}
                    enableToggleShortcut={false}
                  />
                </SheetContent>
              </Sheet>

              <section
                className="kepler-desktop-content-surface flex min-h-0 min-w-0 flex-1 flex-col"
                style={
                  {
                    "--kepler-content-padding-top": "0",
                    "--kepler-content-padding-inline": "0",
                    "--kepler-content-padding-bottom": "0",
                    "--kepler-content-radius-top-left": sidebarConfig.hidden
                      ? "0px"
                      : "16px",
                    "--kepler-content-radius-bottom-left": "0px",
                    "--kepler-content-border-color": "var(--border)",
                    "--kepler-content-border-left-color": sidebarConfig.hidden
                      ? "transparent"
                      : "var(--border)",
                  } as CSSProperties
                }
              >
                <main className="min-w-0 flex-1 overflow-auto pt-14 lg:pt-0">
                  <div className="arrancador-page-shell">
                    <Outlet />
                  </div>
                </main>
              </section>
            </div>
          </div>
        </div>
      </ToastProvider>
    </GamesProvider>
  );
}

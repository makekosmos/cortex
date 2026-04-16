import { Menu, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Outlet, useLocation } from "react-router-dom";

import Spotlight from "@/components/Spotlight";
import { Sidebar } from "@/components/Sidebar";
import { ToastProvider } from "@/components/ToastProvider";
import { cn } from "@/lib/utils";
import { GamesProvider } from "@/store/GamesContext";

export default function Layout() {
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
  const location = useLocation();

  useEffect(() => {
    setIsMobileMenuOpen(false);
  }, [location.pathname]);

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

  return (
    <GamesProvider>
      <ToastProvider>
        <div className="flex h-screen overflow-hidden bg-background text-foreground">
          <div className="fixed top-0 left-0 right-0 z-50 flex h-14 items-center justify-between border-b border-border/70 bg-background/90 px-4 backdrop-blur-xl lg:hidden">
            <span className="text-sm font-[510] tracking-[-0.01em]">Arrancador</span>
            <div className="flex items-center gap-2">
              <Spotlight triggerClassName="h-9 px-3" />
              <button
                type="button"
                aria-label={isMobileMenuOpen ? "Закрыть меню" : "Открыть меню"}
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

          <div
            id="mobile-sidebar"
            className={cn(
              "fixed inset-0 z-40 shrink-0 transition-transform duration-300 lg:static lg:z-auto lg:translate-x-0",
              isMobileMenuOpen ? "translate-x-0" : "-translate-x-full",
            )}
          >
            <button
              type="button"
              aria-label="Закрыть меню"
              className={cn(
                "absolute inset-0 bg-black/82 backdrop-blur-sm transition-opacity duration-300 lg:hidden",
                isMobileMenuOpen
                  ? "opacity-100"
                  : "pointer-events-none opacity-0",
              )}
              onClick={() => setIsMobileMenuOpen(false)}
            />
            <Sidebar />
          </div>

          <div className="fixed top-4 right-4 z-50 hidden lg:block">
            <Spotlight triggerClassName="h-9 px-3" />
          </div>

          <main className="min-w-0 flex-1 overflow-auto pt-14 lg:pt-0">
            <div className="mx-auto min-h-full max-w-[1520px] px-4 pb-4 pt-4 lg:px-6 lg:pt-6">
              <Outlet />
            </div>
          </main>
        </div>
      </ToastProvider>
    </GamesProvider>
  );
}

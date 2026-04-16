import { X } from "lucide-react";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import type { ReactNode } from "react";
import { subscribeAppEvent } from "@/lib/browser";
import { cn } from "@/lib/utils";

export type ToastTone = "info" | "success" | "warning" | "error";

export interface ToastInput {
  title: string;
  description?: string;
  tone?: ToastTone;
  durationMs?: number;
}

interface Toast extends ToastInput {
  id: string;
}

interface ToastContextValue {
  notify: (toast: ToastInput) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

const toneStyles: Record<ToastTone, string> = {
  info: "border-border/70 bg-card/90",
  success: "border-emerald-500/25 bg-emerald-500/8",
  warning: "border-amber-400/25 bg-amber-400/8",
  error: "border-red-500/25 bg-red-500/8",
};

const toneAccent: Record<ToastTone, string> = {
  info: "text-foreground",
  success: "text-emerald-400",
  warning: "text-amber-300",
  error: "text-red-400",
};

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [announcement, setAnnouncement] = useState<Toast | null>(null);
  const counter = useRef(0);

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }, []);

  const notify = useCallback(
    (toast: ToastInput) => {
      const id = `${Date.now()}-${counter.current++}`;
      const entry: Toast = {
        id,
        tone: "info",
        durationMs: 4200,
        ...toast,
      };

      setToasts((prev) => [...prev, entry]);
      setAnnouncement(entry);

      if (entry.durationMs && entry.durationMs > 0) {
        window.setTimeout(() => removeToast(id), entry.durationMs);
      }
    },
    [removeToast],
  );

  useEffect(() => {
    return subscribeAppEvent<{ game_id: string; game_name: string }>(
      "game:save-path-missing",
      (payload) => {
        const name = payload?.game_name || "";
        notify({
          tone: "warning",
          title: "Сохранения не найдены",
          description: `Для "${name}" не найден путь к сохранениям. Укажите его в настройках игры, чтобы работали бэкапы.`,
        });
      },
    );
  }, [notify]);

  return (
    <ToastContext.Provider value={{ notify }}>
      {children}
      <div
        aria-live={announcement?.tone === "error" ? "assertive" : "polite"}
        aria-atomic="true"
        className="sr-only"
      >
        {announcement
          ? [announcement.title, announcement.description]
              .filter(Boolean)
              .join(". ")
          : null}
      </div>
      <div className="fixed bottom-4 right-4 z-50 flex w-full max-w-sm flex-col gap-2 px-4 lg:px-0">
        {toasts.map((toast) => {
          const tone = toast.tone ?? "info";
          return (
            <div
              key={toast.id}
              role={tone === "error" ? "alert" : "status"}
              aria-live={tone === "error" ? "assertive" : "polite"}
              aria-atomic="true"
              className={cn(
                "pointer-events-auto rounded-xl border bg-card/90 px-4 py-3 shadow-[0_18px_40px_rgba(0,0,0,0.32)] backdrop-blur-xl",
                toneStyles[tone],
              )}
            >
              <div className="flex items-start gap-3">
                <div className="flex-1">
                  <div className={cn("text-sm font-[510]", toneAccent[tone])}>
                    {toast.title}
                  </div>
                  {toast.description ? (
                    <div className="mt-1 text-xs leading-relaxed text-muted-foreground">
                      {toast.description}
                    </div>
                  ) : null}
                </div>
                <button
                  type="button"
                  onClick={() => removeToast(toast.id)}
                  className="text-muted-foreground transition-colors hover:text-foreground"
                  aria-label="Закрыть уведомление"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error("useToast must be used within ToastProvider");
  }
  return context;
}

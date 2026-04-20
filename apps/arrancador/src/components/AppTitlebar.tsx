import { Maximize2, Minimize2, PanelLeft, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { TitlebarHistoryControls } from "@/components/TitlebarHistoryControls";
import { useLanguage } from "@/components/language-provider";
import { cn } from "@/lib/utils";
import {
  closeWindow,
  getFallbackWindowChromePlatform,
  minimizeWindow,
  toggleMaximizeWindow,
} from "@/lib/window-chrome";

type AppTitlebarProps = {
  sidebarHidden: boolean;
  onToggleSidebar: () => void;
  canGoBack: boolean;
  canGoForward: boolean;
  onBack: () => void;
  onForward: () => void;
};

type WindowControlsOverlayLike = {
  visible?: boolean;
  addEventListener?: (
    type: "geometrychange",
    listener: EventListenerOrEventListenerObject,
  ) => void;
  removeEventListener?: (
    type: "geometrychange",
    listener: EventListenerOrEventListenerObject,
  ) => void;
};

function getWindowControlsOverlayVisible() {
  if (typeof navigator === "undefined") {
    return false;
  }

  const overlay = (
    navigator as Navigator & { windowControlsOverlay?: WindowControlsOverlayLike }
  ).windowControlsOverlay;

  return Boolean(overlay?.visible);
}

export function AppTitlebar({
  sidebarHidden,
  onToggleSidebar,
  canGoBack,
  canGoForward,
  onBack,
  onForward,
}: AppTitlebarProps) {
  const { language } = useLanguage();
  const platform = getFallbackWindowChromePlatform();
  const [hasNativeWindowControls, setHasNativeWindowControls] = useState(
    getWindowControlsOverlayVisible,
  );

  useEffect(() => {
    if (typeof navigator === "undefined") {
      return;
    }

    const overlay = (
      navigator as Navigator & { windowControlsOverlay?: WindowControlsOverlayLike }
    ).windowControlsOverlay;

    if (!overlay?.addEventListener || !overlay.removeEventListener) {
      setHasNativeWindowControls(false);
      return;
    }

    const syncOverlayState = () => {
      setHasNativeWindowControls(Boolean(overlay.visible));
    };

    syncOverlayState();
    overlay.addEventListener("geometrychange", syncOverlayState);
    return () => overlay.removeEventListener?.("geometrychange", syncOverlayState);
  }, []);

  const sidebarLabel =
    language === "ru"
      ? sidebarHidden
        ? "\u041f\u043e\u043a\u0430\u0437\u0430\u0442\u044c \u0431\u043e\u043a\u043e\u0432\u0443\u044e \u043f\u0430\u043d\u0435\u043b\u044c"
        : "\u0421\u043a\u0440\u044b\u0442\u044c \u0431\u043e\u043a\u043e\u0432\u0443\u044e \u043f\u0430\u043d\u0435\u043b\u044c"
      : sidebarHidden
        ? "Show sidebar"
        : "Hide sidebar";

  const controlsCopy = useMemo(
    () =>
      language === "ru"
        ? {
            back: "\u041d\u0430\u0437\u0430\u0434",
            forward: "\u0412\u043f\u0435\u0440\u0451\u0434",
            minimize: "\u0421\u0432\u0435\u0440\u043d\u0443\u0442\u044c \u043e\u043a\u043d\u043e",
            maximize:
              "\u0420\u0430\u0437\u0432\u0435\u0440\u043d\u0443\u0442\u044c \u043e\u043a\u043d\u043e",
            close: "\u0417\u0430\u043a\u0440\u044b\u0442\u044c \u043e\u043a\u043d\u043e",
          }
        : {
            back: "Back",
            forward: "Forward",
            minimize: "Minimize window",
            maximize: "Maximize window",
            close: "Close window",
          },
    [language],
  );

  const showFallbackWindowControls =
    platform === "windows" && !hasNativeWindowControls;

  return (
    <header
      data-testid="app-titlebar"
      className={cn(
        "kepler-titlebar",
        platform === "mac" && "app-titlebar--mac",
        platform === "windows" && "app-titlebar--windows",
      )}
    >
      <div className="kepler-titlebar__leading">
        <button
          type="button"
          data-testid="desktop-sidebar-toggle"
          className="arrancador-titlebar-button"
          aria-label={sidebarLabel}
          title={sidebarLabel}
          onClick={onToggleSidebar}
        >
          <PanelLeft className="h-4 w-4" />
        </button>

        <TitlebarHistoryControls
          backDisabled={!canGoBack}
          forwardDisabled={!canGoForward}
          backTitle={controlsCopy.back}
          forwardTitle={controlsCopy.forward}
          onBack={onBack}
          onForward={onForward}
        />
      </div>

      <div className="kepler-titlebar__center" />

      <div className="kepler-titlebar__trailing">
        {showFallbackWindowControls && (
          <div
            className="arrancador-window-controls"
            data-testid="fallback-window-controls"
          >
            <button
              type="button"
              className="arrancador-window-controls__button"
              aria-label={controlsCopy.minimize}
              title={controlsCopy.minimize}
              onClick={() => void minimizeWindow()}
            >
              <Minimize2 className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="arrancador-window-controls__button"
              aria-label={controlsCopy.maximize}
              title={controlsCopy.maximize}
              onClick={() => void toggleMaximizeWindow()}
            >
              <Maximize2 className="h-4 w-4" />
            </button>
            <button
              type="button"
              className="arrancador-window-controls__button arrancador-window-controls__button--danger"
              aria-label={controlsCopy.close}
              title={controlsCopy.close}
              onClick={() => void closeWindow()}
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        )}
      </div>
    </header>
  );
}

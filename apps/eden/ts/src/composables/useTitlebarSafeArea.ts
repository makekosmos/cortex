import { watch } from "vue";

import { useEdenStore } from "@/store/eden";
import { useLayoutStore } from "@/store/layout";

export function useTitlebarSafeArea() {
  const layout = useLayoutStore();
  const eden = useEdenStore();

  watch(
    [
      () => eden.isInitializing,
      () => eden.vaultPath,
      () => eden.activeScreen,
      () => layout.widgetSidebarHidden,
      () => layout.widgetSidebarWidth,
    ],
    ([isInitializing, vaultPath, activeScreen, widgetSidebarHidden, widgetSidebarWidth]) => {
      if (isInitializing || !vaultPath || activeScreen === "settings") {
        document.documentElement.style.setProperty("--titlebar-left-safe-area", "0px");
        return;
      }

      const leftArea = widgetSidebarHidden ? 0 : widgetSidebarWidth;

      document.documentElement.style.setProperty(
        "--titlebar-left-safe-area",
        `${leftArea}px`,
      );
    },
    { immediate: true },
  );
}

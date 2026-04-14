import { watchEffect } from "vue";

import { useEdenStore } from "@/store/eden";
import { useLayoutStore } from "@/store/layout";

export function useTitlebarSafeArea() {
  const layout = useLayoutStore();
  const eden = useEdenStore();

  watchEffect(() => {
    if (eden.isInitializing || !eden.vaultPath || eden.activeScreen === "settings") {
      document.documentElement.style.setProperty("--titlebar-left-safe-area", "0px");
      return;
    }

    const leftArea =
      (layout.vaultSidebarHidden ? 0 : layout.vaultSidebarWidth) +
      (layout.widgetSidebarHidden ? 0 : layout.widgetSidebarWidth);

    document.documentElement.style.setProperty(
      "--titlebar-left-safe-area",
      `${leftArea}px`,
    );
  });
}

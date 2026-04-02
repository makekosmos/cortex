import { watchEffect } from "vue";
import { useLayoutStore } from "@/store/layout";

export function useTitlebarSafeArea() {
  const layout = useLayoutStore();

  watchEffect(() => {
    const leftArea =
      (layout.vaultSidebarHidden ? 0 : layout.vaultSidebarWidth) +
      (layout.widgetSidebarHidden ? 0 : layout.widgetSidebarWidth);
    document.documentElement.style.setProperty("--titlebar-left-safe-area", `${leftArea}px`);
  });
}

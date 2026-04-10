import { shallowRef, computed } from "vue";

// Module-level singleton — shared across all component instances
const sidebarHidden = shallowRef(false);

// Initialize from persisted config on module load
try {
  const raw = localStorage.getItem("delphi-sidebar-config");
  if (raw) {
    const cfg = JSON.parse(raw) as { hidden?: boolean };
    sidebarHidden.value = cfg.hidden ?? false;
  }
} catch {
  // ignore
}

export function setSidebarHidden(hidden: boolean) {
  sidebarHidden.value = hidden;
}

export function useSidebarState() {
  const wrapClass = "mx-auto w-full";

  // Animate max-width so the zen ↔ normal transition is smooth.
  const wrapStyle = computed(() => ({
    maxWidth: sidebarHidden.value ? "var(--bringhurst-wide)" : "100%",
    transition: "max-width 0.35s cubic-bezier(0.22, 1, 0.36, 1)",
  }));

  return { sidebarHidden, wrapClass, wrapStyle };
}

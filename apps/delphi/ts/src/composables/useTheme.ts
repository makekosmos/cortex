import { ref, computed, watch } from "vue";

type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "vite-ui-theme";

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const isDark = theme === "dark" || (theme === "system" && prefersDark);
  root.classList.toggle("dark", isDark);
}

// Module-level shared state (singleton)
const theme = ref<Theme>(
  (localStorage.getItem(STORAGE_KEY) as Theme) || "system",
);

// Apply theme immediately on module load and reactively on every change.
// This runs at import time — no component needs to call useTheme() first.
applyTheme(theme.value);

watch(theme, (value) => {
  applyTheme(value);
  localStorage.setItem(STORAGE_KEY, value);
});

// Listen for system theme changes
const mq = window.matchMedia("(prefers-color-scheme: dark)");
mq.addEventListener("change", () => {
  if (theme.value === "system") applyTheme("system");
});

export function useTheme() {
  const resolvedTheme = computed<"light" | "dark">(() => {
    if (theme.value === "system") {
      return mq.matches ? "dark" : "light";
    }
    return theme.value;
  });

  function setTheme(t: Theme) {
    theme.value = t;
  }

  function toggleTheme() {
    theme.value = theme.value === "dark" ? "light" : "dark";
  }

  return { theme, setTheme, toggleTheme, resolvedTheme };
}

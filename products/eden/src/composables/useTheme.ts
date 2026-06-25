import { ref, watch } from "vue";

type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "vite-ui-theme";

function readStoredTheme(fallback: Theme): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  return stored === "light" || stored === "dark" || stored === "system" ? stored : fallback;
}

function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  const isDark = theme === "dark" || (theme === "system" && prefersDark);

  root.classList.toggle("dark", isDark);
  root.style.colorScheme = isDark ? "dark" : "light";
}

const theme = ref<Theme>(readStoredTheme("dark"));

applyTheme(theme.value);

watch(theme, (value) => {
  applyTheme(value);
  localStorage.setItem(STORAGE_KEY, value);
});

const mq = window.matchMedia("(prefers-color-scheme: dark)");

mq.addEventListener("change", () => {
  if (theme.value === "system") {
    applyTheme("system");
  }
});

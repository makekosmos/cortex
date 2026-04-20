import { computed, onMounted, shallowRef } from "vue";

export type Theme = "dark" | "light" | "system";

const storageKey = "arrancador-theme";
const theme = shallowRef<Theme>("dark");

function readStoredTheme(): Theme {
  if (typeof window === "undefined") {
    return "dark";
  }

  const stored = window.localStorage.getItem(storageKey);
  if (stored === "dark" || stored === "light" || stored === "system") {
    return stored;
  }
  return "dark";
}

function applyTheme(nextTheme: Theme) {
  if (typeof window === "undefined") {
    return;
  }

  const root = window.document.documentElement;
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  root.classList.remove("light", "dark");

  if (nextTheme === "system") {
    root.classList.add(media.matches ? "dark" : "light");
    return;
  }

  root.classList.add(nextTheme);
}

export function initializeTheme() {
  theme.value = readStoredTheme();
  applyTheme(theme.value);
}

export function useTheme() {
  onMounted(() => {
    applyTheme(theme.value);
  });

  const setTheme = (nextTheme: Theme) => {
    theme.value = nextTheme;
    if (typeof window !== "undefined") {
      window.localStorage.setItem(storageKey, nextTheme);
    }
    applyTheme(nextTheme);
  };

  return {
    theme: computed(() => theme.value),
    setTheme,
  };
}

import { onMounted } from "vue";

export function usePlatform() {
  onMounted(async () => {
    if (!window.api?.getPlatform) return;

    const platform = await window.api.getPlatform();

    if (platform === "darwin") {
      document.documentElement.classList.add("platform-mac");
    } else if (platform === "win32") {
      document.documentElement.classList.add("platform-windows");
    }
  });
}

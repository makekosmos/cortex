import { openDashboardWindow } from "./dashboard-window";
import { openExtension } from "./extension-host";

export function scheduleBenchmarkOpenAllExtensions(): void {
  setTimeout(() => {
    for (const id of ["delphi", "arrancador", "eden"]) {
      void openExtension(id).catch((e) => console.error(`bench open ${id} failed:`, e));
    }
    try {
      openDashboardWindow();
    } catch (e) {
      console.error("bench open dashboard failed:", e);
    }
  }, 5000);
}

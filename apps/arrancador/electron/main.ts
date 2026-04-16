import { runApp } from "./main/index";

void runApp().catch((error) => {
  console.error("Electron app failed to start:", error);
  process.exitCode = 1;
});

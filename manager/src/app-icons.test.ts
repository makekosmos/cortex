import { expect, test } from "bun:test";
import { appIcon } from "./app-icons";

test("prefers bundled Kosmos icons and preserves external fallbacks", () => {
  for (const [id, file] of Object.entries({
    agenda: "agenda.png",
    arcadia: "arcadia.png",
    dictation: "dictation.png",
    memoria: "memoria.png",
    shell: "kosmos.png",
  })) {
    expect(appIcon(`com.kosmos.${id}`, "https://old/icon.png")).toEndWith(file);
  }
  expect(appIcon("external", "https://example.com/icon.png")).toBe(
    "https://example.com/icon.png",
  );
  expect(appIcon("local", null, "C:\\Apps\\icon.png")).toBe(
    "file:///C:/Apps/icon.png",
  );
});

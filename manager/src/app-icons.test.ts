import { expect, test } from "../../test-support/node-test.mjs";
import { appIcon } from "./app-icons";

test("prefers bundled Kosmos icons and preserves external fallbacks", () => {
  for (const [id, file] of Object.entries({
    agenda: "agenda.png",
    arcadia: "arcadia.png",
    dictation: "dictation.png",
    memoria: "memoria.png",
    focus: "ordo.png",
    shell: "kosmos.png",
  })) {
    expect(appIcon(`com.kosmos.${id}`, "https://old/icon.png")).toEndWith(file);
  }
  expect(appIcon("external", "https://example.com/icon.png")).toBe("https://example.com/icon.png");
  for (const id of [
    "com.kosmos.bigfrontend",
    "com.kosmos.greatfrontend",
    "com.kosmos.hevy",
    "com.kosmos.huawei-health",
    "com.kosmos.leetcode",
    "com.kosmos.codewars",
    "com.kosmos.toggl",
  ])
    expect(appIcon(id)).toMatch(/\.svg$/);
  expect(appIcon("local", null, "C:\\Apps\\icon.png")).toBe("file:///C:/Apps/icon.png");
  expect(appIcon("local", null, "/tmp/My Icon#1.png")).toBe("file:///tmp/My%20Icon%231.png");
});

import { describe, expect, test } from "bun:test";
import type { CommandRecord } from "../../shell/shared/ipc-types";
import { dedupeCommandsById } from "../../shell/src/lib/launcherCommands";

describe("launcher commands", () => {
  test("dedupes commands by id before Vue renders keyed lists", () => {
    // Regression: 2026-06-05. Duplicate v-for keys can crash Vue patching.
    const commands: CommandRecord[] = [
      { id: "app:steam", title: "Steam", category: "open", kind: "app" },
      { id: "app:steam", title: "Steam duplicate", category: "open", kind: "app" },
      { id: "app:discord", title: "Discord", category: "open", kind: "app" },
    ];

    expect(dedupeCommandsById(commands).map((command) => command.title)).toEqual([
      "Steam",
      "Discord",
    ]);
  });
});

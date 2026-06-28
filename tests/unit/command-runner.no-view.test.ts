import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, readFileSync, rmSync } from "node:fs";
import path from "node:path";
import { runCommandNoView } from "../../platform/desktop/electron/command-host/command-runner";
import { writeFixture } from "./command-runner.test";

const fixtureRoot = "command-runner-no-view-test";
const root = path.resolve(".tmp", fixtureRoot);

afterEach(() => {
  if (existsSync(root)) rmSync(root, { recursive: true, force: true });
});

describe("Command no-view command runner", () => {
  test("runs trusted no-view command with storage, cache, preferences, feedback, and clipboard", async () => {
    const fixture = writeFixture(fixtureRoot);

    await runCommandNoView({
      extensionId: "copy-tool",
      commandName: "copy",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      clipboard: { writeText: (text) => fixture.copied.push(text) },
      feedback: (event) => fixture.feedback.push(`${event.kind}:${event.title}`),
    });

    expect(fixture.copied).toEqual(["Hello world"]);
    expect(fixture.feedback).toEqual(["toast:Done"]);
    expect(
      JSON.parse(
        readFileSync(path.join(fixture.userDataDir, "raycast-local-storage.json"), "utf8"),
      ),
    ).toEqual({
      launchType: "userInitiated",
      arguments: "{}",
      fallbackText: "",
      launchContext: "null",
    });
    expect(
      JSON.parse(
        readFileSync(path.join(fixture.userDataDir, "raycast-cache", "copy.json"), "utf8"),
      ),
    ).toEqual({
      prefix: "Hello",
    });
  });

  test("refuses to execute user-installed command code in main process", async () => {
    const fixture = writeFixture(fixtureRoot);
    await expect(
      runCommandNoView({
        extensionId: "copy-tool",
        commandName: "copy",
        extensionDir: fixture.extensionDir,
        userDataDir: fixture.userDataDir,
        source: "user",
      }),
    ).rejects.toThrow("refusing to execute user-installed command");
  });

  test("bridges confirmAlert calls from trusted commands to the host adapter", async () => {
    const fixture = writeFixture(fixtureRoot);
    const prompts: unknown[] = [];

    await runCommandNoView({
      extensionId: "copy-tool",
      commandName: "alert",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      clipboard: { writeText: (text) => fixture.copied.push(text) },
      confirmAlert: async (options) => {
        prompts.push(options);
        return true;
      },
    });

    expect(prompts).toEqual([
      {
        title: "Continue?",
        message: "This is a confirmation",
        primaryAction: { title: "Continue" },
        dismissAction: { title: "Cancel" },
      },
    ]);
    expect(fixture.copied).toEqual(["confirmed"]);
  });

  test("bridges launchCommand calls from trusted commands to the host adapter", async () => {
    const fixture = writeFixture(fixtureRoot);
    const launches: unknown[] = [];

    await runCommandNoView({
      extensionId: "copy-tool",
      commandName: "chain",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      launchCommand: async (options) => launches.push(options),
    });

    expect(launches).toEqual([
      {
        extensionName: "copy-tool",
        name: "copy",
        arguments: { from: "chain" },
        context: { source: "test" },
        fallbackText: "fallback",
      },
    ]);
  });

  test("bridges clipboard read and clear from trusted commands", async () => {
    const fixture = writeFixture(fixtureRoot);
    let clipboardText = "Seed";

    await runCommandNoView({
      extensionId: "copy-tool",
      commandName: "clipboard",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      clipboard: {
        writeText: (text) => {
          clipboardText = text;
        },
        readText: () => clipboardText,
        clear: () => {
          clipboardText = "";
        },
      },
    });

    expect(clipboardText).toBe("");
    expect(
      JSON.parse(
        readFileSync(path.join(fixture.userDataDir, "raycast-local-storage.json"), "utf8"),
      ),
    ).toEqual({
      clipboard: JSON.stringify({ before: "Seed", afterCopy: "Seed copied", afterClear: "" }),
    });
  });

  test("bridges system utilities from trusted commands to the host adapter", async () => {
    const fixture = writeFixture(fixtureRoot);

    await runCommandNoView({
      extensionId: "copy-tool",
      commandName: "files",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      system: {
        open: async (target) => fixture.system.push(`open:${target}`),
        showInFinder: async (target) => fixture.system.push(`show:${target}`),
        trash: async (target) => fixture.system.push(`trash:${target}`),
      },
    });

    expect(fixture.system).toEqual([
      "open:https://example.com",
      "show:C:/Temp/example.txt",
      "trash:C:/Temp/delete.txt",
    ]);
  });

  test("bridges LocalStorage.allItems from trusted commands", async () => {
    const fixture = writeFixture(fixtureRoot);

    await runCommandNoView({
      extensionId: "copy-tool",
      commandName: "storage",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
    });

    expect(
      JSON.parse(
        readFileSync(path.join(fixture.userDataDir, "raycast-local-storage.json"), "utf8"),
      ),
    ).toEqual({
      alpha: "one",
      beta: "two",
      snapshot: JSON.stringify({ alpha: "one", beta: "two" }),
    });
  });

  test("passes launch props into command default export", async () => {
    const fixture = writeFixture(fixtureRoot);

    await runCommandNoView({
      extensionId: "copy-tool",
      commandName: "copy",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      launch: {
        launchType: "launchCommand",
        arguments: { query: "hello" },
        fallbackText: "hello",
        launchContext: { from: "chain" },
      },
    });

    expect(
      JSON.parse(
        readFileSync(path.join(fixture.userDataDir, "raycast-local-storage.json"), "utf8"),
      ),
    ).toEqual({
      launchType: "launchCommand",
      arguments: JSON.stringify({ query: "hello" }),
      fallbackText: "hello",
      launchContext: JSON.stringify({ from: "chain" }),
    });
  });
});

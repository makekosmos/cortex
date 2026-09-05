import { beforeEach, expect, mock, test } from "bun:test";

const openExtension = mock(async () => {});
const commandRuntimeContext = mock(() => ({
  dir: "C:\\extensions\\custom",
  source: "dev" as const,
}));
const runCommandNoView = mock(async () => {});
const openCommandViewCommand = mock(async () => {});
const assertLegacyLaunchAllowed = mock((_dataDir: string, id: string) => {
  if (id === "eden") throw new Error("legacy launch denied");
});

mock.module("electron", () => ({
  clipboard: {},
  dialog: { showMessageBox: async () => ({ response: 0 }) },
  shell: {
    openExternal: async () => {},
    openPath: async () => "",
    showItemInFolder: () => {},
    trashItem: async () => {},
  },
}));
mock.module("./data-dir", () => ({ keplerDataDir: () => "C:\\Kosmos-test" }));
mock.module("./legacy-migration-journal", () => ({ assertLegacyLaunchAllowed }));
mock.module("./extension-host", () => ({
  commandRuntimeContext,
  extensionUserDataDir: (id: string) => `C:\\Kosmos-test\\extensions-data\\${id}`,
  findDeclaredCommand: () => null,
  openExtension,
}));
mock.module("./command-host/command-runner", () => ({ runCommandNoView }));
mock.module("./command-host/view-host", () => ({ openCommandViewCommand }));

const { launchCommandDeclaredCommand } = await import("./main-command-host");

type CommandMode = "open" | "command-view" | "command-no-view" | "command-menu-bar";

function declared(mode: CommandMode, extensionId: string) {
  return {
    id: `${extensionId}:command`,
    title: "Command",
    category: "action" as const,
    kind: "command" as const,
    appName: "Extension",
    extensionId,
    mode,
    commandName: mode === "open" ? undefined : "command",
  };
}

beforeEach(() => {
  openExtension.mockClear();
  commandRuntimeContext.mockClear();
  runCommandNoView.mockClear();
  openCommandViewCommand.mockClear();
  assertLegacyLaunchAllowed.mockClear();
});

test("committed legacy identities are denied before every command route", async () => {
  const modes: CommandMode[] = ["open", "command-view", "command-no-view", "command-menu-bar"];

  for (const mode of modes) {
    await expect(launchCommandDeclaredCommand(declared(mode, "eden"))).rejects.toThrow(
      "legacy launch denied",
    );
  }

  expect(assertLegacyLaunchAllowed).toHaveBeenCalledTimes(modes.length);
  expect(openExtension).not.toHaveBeenCalled();
  expect(commandRuntimeContext).not.toHaveBeenCalled();
  expect(runCommandNoView).not.toHaveBeenCalled();
  expect(openCommandViewCommand).not.toHaveBeenCalled();
});

test("non-legacy command routes keep their existing downstream behavior", async () => {
  await expect(launchCommandDeclaredCommand(declared("open", "custom-extension"))).resolves.toBe(
    true,
  );
  await expect(
    launchCommandDeclaredCommand(declared("command-view", "custom-extension")),
  ).resolves.toBe(true);
  await expect(
    launchCommandDeclaredCommand(declared("command-menu-bar", "custom-extension")),
  ).resolves.toBe(true);
  await expect(
    launchCommandDeclaredCommand(declared("command-no-view", "custom-extension")),
  ).resolves.toBe(true);

  expect(assertLegacyLaunchAllowed).toHaveBeenCalledTimes(4);
  expect(openExtension).toHaveBeenCalledTimes(1);
  expect(openCommandViewCommand).toHaveBeenCalledTimes(2);
  expect(runCommandNoView).toHaveBeenCalledTimes(1);
});

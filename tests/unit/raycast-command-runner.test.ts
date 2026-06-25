import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  runRaycastNoViewCommand,
  runRaycastViewCommand,
} from "../../platform/desktop/electron/raycast/command-runner";

const root = path.resolve(".tmp", "raycast-command-runner-test");

afterEach(() => {
  if (existsSync(root)) rmSync(root, { recursive: true, force: true });
});

function writeFixture(): {
  extensionDir: string;
  userDataDir: string;
  copied: string[];
  feedback: string[];
  system: string[];
} {
  const extensionDir = path.join(root, "extension", "copy-tool");
  const userDataDir = path.join(root, "data", "copy-tool");
  const dist = path.join(extensionDir, "dist");
  mkdirSync(dist, { recursive: true });
  mkdirSync(userDataDir, { recursive: true });
  writeFileSync(
    path.join(extensionDir, "package.json"),
    JSON.stringify(
      {
        name: "copy-tool",
        title: "Copy Tool",
        commands: [
          { name: "copy", title: "Copy", mode: "no-view" },
          { name: "alert", title: "Alert", mode: "no-view" },
          { name: "chain", title: "Chain", mode: "no-view" },
          { name: "clipboard", title: "Clipboard", mode: "no-view" },
          { name: "files", title: "Files", mode: "no-view" },
          { name: "storage", title: "Storage", mode: "no-view" },
          { name: "search", title: "Search", mode: "view" },
          { name: "create", title: "Create", mode: "view" },
          { name: "gallery", title: "Gallery", mode: "view" },
          { name: "jsx", title: "JSX", mode: "view" },
          { name: "inspect", title: "Inspect", mode: "view" },
          { name: "nav", title: "Navigation", mode: "view" },
          { name: "status", title: "Status", mode: "menu-bar" },
        ],
        preferences: [{ name: "prefix", default: "Hello" }],
        kosmos: {
          commands: {
            copy: { entry: "dist/copy.mjs" },
            alert: { entry: "dist/alert.mjs" },
            chain: { entry: "dist/chain.mjs" },
            clipboard: { entry: "dist/clipboard.mjs" },
            files: { entry: "dist/files.mjs" },
            storage: { entry: "dist/storage.mjs" },
            search: { entry: "dist/search.mjs" },
            create: { entry: "dist/create.mjs" },
            gallery: { entry: "dist/gallery.mjs" },
            jsx: { entry: "dist/jsx.mjs" },
            inspect: { entry: "dist/inspect.mjs" },
            nav: { entry: "dist/nav.mjs" },
            status: { entry: "dist/status.mjs" },
          },
        },
      },
      null,
      2,
    ),
    "utf8",
  );
  writeFileSync(
    path.join(dist, "copy.mjs"),
    `
import { Clipboard, LocalStorage, Cache, getPreferenceValues, showToast } from "@raycast/api";

export default async function Command(props) {
  const prefs = getPreferenceValues();
  await LocalStorage.setItem("launchType", props.launchType);
  await LocalStorage.setItem("arguments", JSON.stringify(props.arguments));
  await LocalStorage.setItem("fallbackText", props.fallbackText ?? "");
  await LocalStorage.setItem("launchContext", JSON.stringify(props.launchContext ?? null));
  const cache = new Cache({ namespace: "copy" });
  await cache.set("prefix", String(prefs.prefix));
  await Clipboard.copy(String(prefs.prefix) + " world");
  await showToast({ title: "Done" });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "chain.mjs"),
    `
import { launchCommand } from "@raycast/api";

export default async function Command() {
  await launchCommand({
    extensionName: "copy-tool",
    name: "copy",
    arguments: { from: "chain" },
    context: { source: "test" },
    fallbackText: "fallback",
  });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "clipboard.mjs"),
    `
import { Clipboard, LocalStorage } from "@raycast/api";

export default async function Command() {
  const before = await Clipboard.readText();
  await Clipboard.copy(before + " copied");
  const afterCopy = await Clipboard.read();
  await Clipboard.clear();
  const afterClear = await Clipboard.readText();
  await LocalStorage.setItem("clipboard", JSON.stringify({ before, afterCopy, afterClear }));
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "alert.mjs"),
    `
import { Clipboard, confirmAlert } from "@raycast/api";

export default async function Command() {
  const ok = await confirmAlert({
    title: "Continue?",
    message: "This is a confirmation",
    primaryAction: { title: "Continue" },
    dismissAction: { title: "Cancel" },
  });
  await Clipboard.copy(ok ? "confirmed" : "dismissed");
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "files.mjs"),
    `
import { open, showInFinder, trash } from "@raycast/api";

export default async function Command() {
  await open("https://example.com");
  await showInFinder("C:/Temp/example.txt");
  await trash("C:/Temp/delete.txt");
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "storage.mjs"),
    `
import { LocalStorage } from "@raycast/api";

export default async function Command() {
  await LocalStorage.setItem("alpha", "one");
  await LocalStorage.setItem("beta", "two");
  const items = await LocalStorage.allItems();
  await LocalStorage.setItem("snapshot", JSON.stringify(items));
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "search.mjs"),
    `
import { Action, ActionPanel, Detail, List, Toast, showHUD, showToast } from "@raycast/api";

export default function Command() {
  return List({
    searchBarPlaceholder: "Search notes",
    children: [
      List.Section({
        title: "Notes",
        children: [
          List.Item({
            id: "hello",
            title: "Hello",
            subtitle: "First item",
            detail: Detail({ markdown: "# Hello" }),
            actions: ActionPanel({
              children: [
                Action.CopyToClipboard({ content: "Hello" }),
                Action.Paste({ content: "Hello" }),
                Action.Push({ title: "Show Detail", target: Detail({ markdown: "# Pushed" }) }),
                Action.OpenInBrowser({ title: "Open Site", url: "https://example.com" }),
                Action.Open({ title: "Open File", target: "C:/Temp/example.txt" }),
                Action.ShowInFinder({ path: "C:/Temp/example.txt" }),
                Action.Trash({ paths: ["C:/Temp/delete.txt"] }),
                Action({
                  title: "Feedback",
                  onAction: async () => {
                    await showToast({
                      style: Toast.Style.Success,
                      title: "Saved",
                      message: "Feedback works",
                    });
                    await showHUD("Ready");
                  },
                }),
              ],
            }),
          }),
        ],
      }),
      List.EmptyView({
        title: "No notes",
      }),
    ],
  });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "create.mjs"),
    `
import { Action, ActionPanel, Form, LocalStorage } from "@raycast/api";

export default function Command() {
  return Form({
    children: [
      Form.TextField({ id: "title", title: "Title", defaultValue: "Draft" }),
      Form.Checkbox({ id: "flagged", title: "Flagged", defaultValue: true }),
      Form.Dropdown({
        id: "kind",
        title: "Kind",
        defaultValue: "note",
        children: [
          Form.Dropdown.Item({ value: "note", title: "Note" }),
          Form.Dropdown.Item({ value: "task", title: "Task" }),
        ],
      }),
      Form.FilePicker({
        id: "attachments",
        title: "Attachments",
        defaultValue: ["C:/Temp/example.txt"],
        allowMultipleSelection: true,
      }),
    ],
    actions: ActionPanel({
      children: [
        Action.SubmitForm({
          title: "Create",
          onSubmit: async (values) => {
            await LocalStorage.setItem("submitted", JSON.stringify(values));
          },
        }),
      ],
    }),
  });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "gallery.mjs"),
    `
import { Action, ActionPanel, Grid } from "@raycast/api";

export default function Command() {
  return Grid({
    searchBarPlaceholder: "Search games",
    children: [
      Grid.Section({
        title: "Games",
        children: [
          Grid.Item({
            id: "game-one",
            title: "Game One",
            subtitle: "Installed",
            content: { source: "data:image/png;base64,abc" },
            actions: ActionPanel({
              children: [Action.CopyToClipboard({ content: "Game One" })],
            }),
          }),
        ],
      }),
      Grid.EmptyView({ title: "No games" }),
    ],
  });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "jsx.mjs"),
    `
import { Action, ActionPanel, List } from "@raycast/api";
import { jsx, jsxs } from "@raycast/api/jsx-runtime";

export default function Command() {
  return jsx(List, {
    children: jsx(List.Item, {
      id: "jsx-item",
      title: "JSX Item",
      actions: jsx(ActionPanel, {
        children: jsxs(ActionPanel.Section, {
          children: [
            jsx(Action.CopyToClipboard, { content: "JSX" }),
            jsx(Action, { title: "Plain Action" }),
          ],
        }),
      }),
    }),
  });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "inspect.mjs"),
    `
import { Action, ActionPanel, Detail } from "@raycast/api";

export default function Command() {
  return Detail({
    markdown: "# Inspect",
    metadata: Detail.Metadata({
      children: [
        Detail.Metadata.Label({ title: "Status", text: "Ready" }),
        Detail.Metadata.TagList({
          title: "Tags",
          children: [
            Detail.Metadata.TagList.Item({ text: "raycast" }),
            Detail.Metadata.TagList.Item({ text: "metadata" }),
          ],
        }),
      ],
    }),
    actions: ActionPanel({
      children: [
        Action.CopyToClipboard({ title: "Copy Inspect", content: "Inspect" }),
      ],
    }),
  });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "nav.mjs"),
    `
import { Action, ActionPanel, Detail, List, useNavigation } from "@raycast/api";

export default function Command() {
  const navigation = useNavigation();
  return List({
    children: [
      List.Item({
        id: "root",
        title: "Root",
        actions: ActionPanel({
          children: [
            Action({
              title: "Push",
              onAction: () => navigation.push(Detail({ markdown: "# Pushed by hook" })),
            }),
            Action.Pop(),
            Action.PopToRoot(),
          ],
        }),
      }),
    ],
  });
}
`,
    "utf8",
  );
  writeFileSync(
    path.join(dist, "status.mjs"),
    `
import { MenuBarExtra, LocalStorage } from "@raycast/api";

export default function Command() {
  return MenuBarExtra({
    title: "Status",
    children: [
      MenuBarExtra.Section({
        title: "Actions",
        children: [
          MenuBarExtra.Item({
            id: "sync",
            title: "Sync Now",
            subtitle: "Runs from menu",
            onAction: async () => {
              await LocalStorage.setItem("menuAction", "sync");
            },
          }),
        ],
      }),
    ],
  });
}
`,
    "utf8",
  );
  const copied: string[] = [];
  const feedback: string[] = [];
  const system: string[] = [];
  return { extensionDir, userDataDir, copied, feedback, system };
}

describe("Raycast no-view command runner", () => {
  test("runs trusted no-view command with storage, cache, preferences, feedback, and clipboard", async () => {
    const fixture = writeFixture();

    await runRaycastNoViewCommand({
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
    const fixture = writeFixture();
    await expect(
      runRaycastNoViewCommand({
        extensionId: "copy-tool",
        commandName: "copy",
        extensionDir: fixture.extensionDir,
        userDataDir: fixture.userDataDir,
        source: "user",
      }),
    ).rejects.toThrow("refusing to execute user-installed Raycast command");
  });

  test("bridges confirmAlert calls from trusted commands to the host adapter", async () => {
    const fixture = writeFixture();
    const prompts: unknown[] = [];

    await runRaycastNoViewCommand({
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
    const fixture = writeFixture();
    const launches: unknown[] = [];

    await runRaycastNoViewCommand({
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
    const fixture = writeFixture();
    let clipboardText = "Seed";

    await runRaycastNoViewCommand({
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
    const fixture = writeFixture();

    await runRaycastNoViewCommand({
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
    const fixture = writeFixture();

    await runRaycastNoViewCommand({
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
    const fixture = writeFixture();

    await runRaycastNoViewCommand({
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

  test("runs trusted view command and returns a host-renderable snapshot", async () => {
    const fixture = writeFixture();
    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "search",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
    });

    expect(snapshot.type).toBe("List");
    expect(snapshot.children[0]?.type).toBe("List.Section");
    expect(snapshot.children[0]?.children[0]?.type).toBe("List.Item");
    expect(snapshot.children[0]?.children[0]?.children.map((node) => node.type)).toEqual([
      "Detail",
      "ActionPanel",
    ]);
    const actionPanel = snapshot.children[0]?.children[0]?.children.find(
      (node) => node.type === "ActionPanel",
    );
    const pushAction = actionPanel?.children.find((node) => node.type === "Action.Push");
    expect(pushAction?.children[0]?.type).toBe("Detail");
    expect(pushAction?.children[0]?.props.markdown).toBe("# Pushed");
    expect(actionPanel?.children.map((node) => node.type)).toEqual([
      "Action.CopyToClipboard",
      "Action.Paste",
      "Action.Push",
      "Action.OpenInBrowser",
      "Action.Open",
      "Action.ShowInFinder",
      "Action.Trash",
      "Action",
    ]);
  });

  test("bridges feedback from trusted view action callbacks to the host adapter", async () => {
    const fixture = writeFixture();
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const feedback: string[] = [];

    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "search",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      feedback: (event) => feedback.push(`${event.kind}:${event.style ?? "none"}:${event.title}`),
      callbacks: {
        register(callback) {
          const id = `callback:${callbacks.size}`;
          callbacks.set(id, callback);
          return id;
        },
      },
    });

    const feedbackAction = snapshot.children[0]?.children[0]?.children
      .find((node) => node.type === "ActionPanel")
      ?.children.find((node) => node.type === "Action");
    await callbacks.get(String(feedbackAction?.props.__callbackId))?.();

    expect(feedback).toEqual(["toast:success:Saved", "hud:none:Ready"]);
  });

  test("registers trusted form submit callbacks for view commands", async () => {
    const fixture = writeFixture();
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "create",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      callbacks: {
        register(callback) {
          const id = `callback:${callbacks.size}`;
          callbacks.set(id, callback);
          return id;
        },
      },
    });

    expect(snapshot.type).toBe("Form");
    expect(snapshot.children.map((node) => node.type)).toContain("Form.FilePicker");
    const submitAction = snapshot.children
      .find((node) => node.type === "ActionPanel")
      ?.children.find((node) => node.type === "Action.SubmitForm");
    const callbackId = submitAction?.props.__callbackId;
    expect(callbackId).toBe("callback:0");

    await callbacks.get(String(callbackId))?.({
      values: { title: "Saved", flagged: false, kind: "task" },
    });
    expect(
      JSON.parse(
        readFileSync(path.join(fixture.userDataDir, "raycast-local-storage.json"), "utf8"),
      ),
    ).toEqual({
      submitted: JSON.stringify({ title: "Saved", flagged: false, kind: "task" }),
    });
  });

  test("runs trusted grid command and returns sectioned grid snapshot", async () => {
    const fixture = writeFixture();
    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "gallery",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
    });

    expect(snapshot.type).toBe("Grid");
    expect(snapshot.children[0]?.type).toBe("Grid.Section");
    expect(snapshot.children[0]?.children[0]?.type).toBe("Grid.Item");
    expect(snapshot.children[0]?.children[0]?.children[0]?.type).toBe("ActionPanel");
    expect(snapshot.children[1]?.type).toBe("Grid.EmptyView");
  });

  test("bridges @raycast/api/jsx-runtime imports from trusted view commands", async () => {
    const fixture = writeFixture();
    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "jsx",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
    });

    expect(snapshot.type).toBe("List");
    const item = snapshot.children[0];
    expect(item?.type).toBe("List.Item");
    expect(item?.props.id).toBe("jsx-item");
    expect(item?.props.title).toBe("JSX Item");
    const actions = item?.children.find((node) => node.type === "ActionPanel")?.children ?? [];
    expect(actions[0]?.type).toBe("ActionPanel.Section");
    expect(actions[0]?.children.map((node) => node.type)).toEqual([
      "Action.CopyToClipboard",
      "Action",
    ]);
  });

  test("runs trusted detail command and returns metadata snapshot", async () => {
    const fixture = writeFixture();
    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "inspect",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
    });

    expect(snapshot.type).toBe("Detail");
    expect(snapshot.props.markdown).toBe("# Inspect");
    expect(snapshot.children[0]?.type).toBe("Detail.Metadata");
    expect(snapshot.children[1]?.type).toBe("ActionPanel");
    expect(snapshot.children[0]?.children.map((node) => node.type)).toEqual([
      "Detail.Metadata.Label",
      "Detail.Metadata.TagList",
    ]);
    expect(snapshot.children[1]?.children[0]?.type).toBe("Action.CopyToClipboard");
  });

  test("forwards trusted useNavigation operations from action callbacks", async () => {
    const fixture = writeFixture();
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const navigation: string[] = [];
    const pushed: unknown[] = [];
    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "nav",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      navigation: {
        push(target) {
          navigation.push("push");
          pushed.push(target);
        },
        pop() {
          navigation.push("pop");
        },
        popToRoot() {
          navigation.push("root");
        },
      },
      callbacks: {
        register(callback) {
          const id = `callback:${callbacks.size}`;
          callbacks.set(id, callback);
          return id;
        },
      },
    });

    const actions =
      snapshot.children[0]?.children.find((node) => node.type === "ActionPanel")?.children ?? [];
    expect(actions.map((node) => node.type)).toEqual(["Action", "Action.Pop", "Action.PopToRoot"]);

    await callbacks.get(String(actions[0]?.props.__callbackId))?.();
    await callbacks.get(String(actions[1]?.props.__callbackId))?.();
    await callbacks.get(String(actions[2]?.props.__callbackId))?.();

    expect(navigation).toEqual(["push", "pop", "root"]);
    expect((pushed[0] as { type?: unknown } | undefined)?.type).toBe("Detail");
  });

  test("runs trusted menu-bar command and registers item callbacks", async () => {
    const fixture = writeFixture();
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const snapshot = await runRaycastViewCommand({
      extensionId: "copy-tool",
      commandName: "status",
      commandMode: "menu-bar",
      extensionDir: fixture.extensionDir,
      userDataDir: fixture.userDataDir,
      source: "dev",
      callbacks: {
        register(callback) {
          const id = `callback:${callbacks.size}`;
          callbacks.set(id, callback);
          return id;
        },
      },
    });

    expect(snapshot.type).toBe("MenuBarExtra");
    expect(snapshot.props.title).toBe("Status");
    expect(snapshot.children[0]?.type).toBe("MenuBarExtra.Section");
    const item = snapshot.children[0]?.children[0];
    expect(item?.type).toBe("MenuBarExtra.Item");
    expect(item?.props.__callbackId).toBe("callback:0");

    await callbacks.get(String(item?.props.__callbackId))?.();
    expect(
      JSON.parse(
        readFileSync(path.join(fixture.userDataDir, "raycast-local-storage.json"), "utf8"),
      ),
    ).toEqual({
      menuAction: "sync",
    });
  });
});

import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, readFileSync, rmSync } from "node:fs";
import path from "node:path";
import { runCommandView } from "../../platform/desktop/electron/command-host/command-runner";
import { writeFixture } from "./command-runner.test";

const fixtureRoot = "command-runner-view-test";
const root = path.resolve(".tmp", fixtureRoot);

afterEach(() => {
  if (existsSync(root)) rmSync(root, { recursive: true, force: true });
});

describe("Command view command runner", () => {
  test("runs trusted view command and returns a host-renderable snapshot", async () => {
    const fixture = writeFixture(fixtureRoot);
    const snapshot = await runCommandView({
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
    const fixture = writeFixture(fixtureRoot);
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const feedback: string[] = [];

    const snapshot = await runCommandView({
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
    const fixture = writeFixture(fixtureRoot);
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const snapshot = await runCommandView({
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
    const fixture = writeFixture(fixtureRoot);
    const snapshot = await runCommandView({
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
    const fixture = writeFixture(fixtureRoot);
    const snapshot = await runCommandView({
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
    const actionPanel = item?.children.find((node) => node.type === "ActionPanel");
    expect(actionPanel?.type).toBe("ActionPanel");
    const actions = actionPanel?.children;
    expect(actions[0]?.type).toBe("ActionPanel.Section");
    expect(actions[0]?.children.map((node) => node.type)).toEqual([
      "Action.CopyToClipboard",
      "Action",
    ]);
  });

  test("runs trusted detail command and returns metadata snapshot", async () => {
    const fixture = writeFixture(fixtureRoot);
    const snapshot = await runCommandView({
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
    const fixture = writeFixture(fixtureRoot);
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const navigation: string[] = [];
    const pushed: unknown[] = [];
    const snapshot = await runCommandView({
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

    const actionPanel = snapshot.children[0]?.children.find((node) => node.type === "ActionPanel");
    expect(actionPanel?.type).toBe("ActionPanel");
    const actions = actionPanel?.children;
    expect(actions.map((node) => node.type)).toEqual(["Action", "Action.Pop", "Action.PopToRoot"]);

    await callbacks.get(String(actions[0]?.props.__callbackId))?.();
    await callbacks.get(String(actions[1]?.props.__callbackId))?.();
    await callbacks.get(String(actions[2]?.props.__callbackId))?.();

    expect(navigation).toEqual(["push", "pop", "root"]);
    expect((pushed[0] as { type?: unknown } | undefined)?.type).toBe("Detail");
  });

  test("runs trusted menu-bar command and registers item callbacks", async () => {
    const fixture = writeFixture(fixtureRoot);
    const callbacks = new Map<
      string,
      (payload?: Record<string, unknown>) => unknown | Promise<unknown>
    >();
    const snapshot = await runCommandView({
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

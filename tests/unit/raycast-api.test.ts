import { describe, expect, test } from "bun:test";
import {
  Action,
  ActionPanel,
  Cache,
  Clipboard,
  Detail,
  Keyboard,
  List,
  LocalStorage,
  Toast,
  configureRaycastRuntime,
  confirmAlert,
  getPreferenceValues,
  launchCommand,
  open,
  resetRaycastRuntimeForTest,
  showInFinder,
  showHUD,
  showToast,
  trash,
  useNavigation,
  type RaycastRuntimeAdapter,
} from "../../packages/raycast-api/src/index";

function adapter(events: string[]): RaycastRuntimeAdapter {
  const local = new Map<string, string>();
  const cache = new Map<string, string>();
  let clipboardText = "";
  return {
    async showToast(options) {
      events.push(`toast:${options.style}:${options.title}`);
    },
    async showHUD(title) {
      events.push(`hud:${title}`);
    },
    async confirmAlert() {
      return true;
    },
    async clipboardCopy(content) {
      clipboardText = content;
      events.push(`copy:${content}`);
    },
    async clipboardPaste(content) {
      clipboardText = content;
      events.push(`paste:${content}`);
    },
    async clipboardReadText() {
      return clipboardText;
    },
    async clipboardClear() {
      clipboardText = "";
      events.push("clear");
    },
    async systemOpen(target) {
      events.push(`open:${target}`);
    },
    async systemShowInFinder(target) {
      events.push(`show:${target}`);
    },
    async systemTrash(target) {
      events.push(`trash:${target}`);
    },
    async localStorageGetItem(key) {
      return local.get(key);
    },
    async localStorageAllItems() {
      return Object.fromEntries(local.entries());
    },
    async localStorageSetItem(key, value) {
      local.set(key, value);
    },
    async localStorageRemoveItem(key) {
      local.delete(key);
    },
    async localStorageClear() {
      local.clear();
    },
    async cacheGet(namespace, key) {
      return cache.get(`${namespace}:${key}`);
    },
    async cacheSet(namespace, key, value) {
      cache.set(`${namespace}:${key}`, value);
    },
    async cacheRemove(namespace, key) {
      cache.delete(`${namespace}:${key}`);
    },
    async cacheClear(namespace) {
      for (const key of cache.keys()) {
        if (key.startsWith(`${namespace}:`)) cache.delete(key);
      }
    },
    getPreferenceValues() {
      return { token: "secret" };
    },
    async launchCommand(options) {
      events.push(`launch:${options.extensionName ?? "self"}:${options.name}`);
    },
    navigationPush(target) {
      events.push(`push:${String(target)}`);
    },
    navigationPop() {
      events.push("pop");
    },
    navigationPopToRoot() {
      events.push("root");
    },
  };
}

describe("@raycast/api shim", () => {
  test("exports runtime-backed feedback, storage, clipboard, command, and navigation APIs", async () => {
    const events: string[] = [];
    configureRaycastRuntime(adapter(events));

    await showToast({ style: Toast.Style.Success, title: "Done" });
    await showHUD("Copied");
    const confirmed = await confirmAlert({ title: "Continue?" });
    await Clipboard.copy("hello");
    expect(await Clipboard.readText()).toBe("hello");
    await Clipboard.paste("world");
    expect(await Clipboard.read()).toBe("world");
    await Clipboard.clear();
    expect(await Clipboard.readText()).toBe("");
    await open("https://example.com");
    await showInFinder("C:/Temp/example.txt");
    await trash("C:/Temp/delete.txt");
    await LocalStorage.setItem("k", "v");
    await LocalStorage.setItem("second", "value");
    const cache = new Cache({ namespace: "notes" });
    await cache.set("a", "b");
    await launchCommand({ extensionName: "tools", name: "copy" });
    useNavigation().push("detail");
    useNavigation().pop();
    useNavigation().popToRoot();

    expect(await LocalStorage.getItem("k")).toBe("v");
    expect(await LocalStorage.allItems()).toEqual({ k: "v", second: "value" });
    expect(await cache.get("a")).toBe("b");
    expect(confirmed).toBe(true);
    expect(getPreferenceValues<{ token: string }>().token).toBe("secret");
    expect(events).toEqual([
      "toast:success:Done",
      "hud:Copied",
      "copy:hello",
      "paste:world",
      "clear",
      "open:https://example.com",
      "show:C:/Temp/example.txt",
      "trash:C:/Temp/delete.txt",
      "launch:tools:copy",
      "push:detail",
      "pop",
      "root",
    ]);

    resetRaycastRuntimeForTest();
  });

  test("creates serializable command primitives for List, Detail, ActionPanel, and Actions", () => {
    const detail = Detail({ markdown: "# Hello" });
    const listItemDetail = List.Item.Detail({
      markdown: "# List Detail",
      metadata: List.Item.Detail.Metadata({
        children: [List.Item.Detail.Metadata.Label({ title: "Status", text: "Ready" })],
      }),
    });
    const actionPanel = ActionPanel({
      children: [
        Action.CopyToClipboard({ content: "Hello" }),
        Action.Pop({ shortcut: { modifiers: ["cmd"], key: "arrowLeft" } }),
        Action.PopToRoot(),
        ActionPanel.Submenu({
          title: "More",
          children: [Action.Push({ title: "Open", target: detail })],
        }),
      ],
    });
    const list = List({
      searchBarPlaceholder: "Search notes",
      children: [
        List.Section({
          title: "Notes",
          children: [
            List.Item({
              id: "hello",
              title: "Hello",
              detail: listItemDetail,
              actions: actionPanel,
            }),
          ],
        }),
      ],
    });

    expect(list.type).toBe("List");
    expect(list.props.searchBarPlaceholder).toBe("Search notes");
    const section = list.props.children?.[0];
    expect(
      typeof section === "object" && section !== null && "type" in section ? section.type : "",
    ).toBe("List.Section");
    const item =
      typeof section === "object" && section !== null && "props" in section
        ? section.props.children?.[0]
        : null;
    const itemDetail =
      typeof item === "object" && item !== null && "props" in item ? item.props.detail : null;
    expect(itemDetail?.type).toBe("List.Item.Detail");
    expect(itemDetail?.props.metadata?.type).toBe("Detail.Metadata");
    expect(itemDetail?.props.metadata?.props.children?.[0]?.type).toBe("Detail.Metadata.Label");
    expect(actionPanel.props.children?.map((node) => node?.type)).toEqual([
      "Action.CopyToClipboard",
      "Action.Pop",
      "Action.PopToRoot",
      "ActionPanel.Submenu",
    ]);
    expect(Keyboard.Shortcut.Common.Copy).toEqual({ modifiers: ["cmd"], key: "c" });
    expect(Keyboard.Shortcut.Common.Open).toEqual({ modifiers: ["cmd"], key: "o" });
  });
});

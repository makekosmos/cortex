import { describe, expect, test } from "bun:test";
import {
  Action,
  ActionPanel,
  Detail,
  Keyboard,
  List,
} from "../../packages/raycast-api/src/index";
import { normalizeCommandNode } from "../../platform/desktop/electron/command-host/view-model";
import {
  actionNodes,
  actionSections,
  actionShortcut,
  actionSubmenuActions,
  matchesActionShortcut,
} from "../../platform/desktop/src/command-host/model";
import {
  detailMarkdown,
  detailMetadataItems,
} from "../../platform/desktop/src/command-host/model-detail";
import {
  listEmptyActions,
  listEmptyMessage,
  listDropdown,
  listFiltering,
  listIsLoading,
  listItems,
  listPlaceholder,
  listSearchCallbackNode,
  listSearchText,
  listSections,
  listSelectedItemId,
  listSelectionCallbackNode,
} from "../../platform/desktop/src/command-host/model-list";

describe("Command list view model", () => {
  test("normalizes List.Item detail/actions props into host-renderable children", () => {
    const root = List({
      searchBarPlaceholder: "Search notes",
      searchBarAccessory: List.Dropdown({
        defaultValue: "all",
        children: [
          List.Dropdown.Item({ value: "all", title: "Все" }),
          List.Dropdown.Section({
            title: "Тип",
            children: [
              List.Dropdown.Item({ value: "notes", title: "Заметки" }),
              List.Dropdown.Item({ value: "tasks", title: "Задачи" }),
            ],
          }),
        ],
      }),
      children: [
        List.Section({
          title: "Заметки",
          children: [
            List.Item({
              id: "hello",
              title: "Hello",
              subtitle: "First",
              icon: { source: "data:image/png;base64,abc" },
              keywords: ["note"],
              accessories: [{ text: "2 задачи" }, { tag: "Сегодня" }],
              detail: List.Item.Detail({
                markdown: "# Hello",
                metadata: List.Item.Detail.Metadata({
                  children: [List.Item.Detail.Metadata.Label({ title: "Тип", text: "Заметка" })],
                }),
              }),
              actions: ActionPanel({
                children: [
                  Action.CopyToClipboard({ content: "Hello" }),
                  Action.Paste({ content: "Hello" }),
                  Action.Push({ title: "Открыть", target: Detail({ markdown: "# Pushed" }) }),
                  Action.OpenInBrowser({ url: "https://example.com" }),
                  Action.Open({ target: "C:/Temp/example.txt" }),
                  Action.ShowInFinder({ path: "C:/Temp/example.txt" }),
                  Action.Trash({ paths: ["C:/Temp/delete.txt"] }),
                  Action({ title: "Произвольное", onAction: () => {} }),
                  Action.LaunchCommand({
                    title: "Запустить copy",
                    name: "copy",
                    arguments: { from: "list" },
                  }),
                ],
              }),
            }),
          ],
        }),
        List.EmptyView({
          title: "Пусто",
          actions: ActionPanel({
            children: [
              Action.CopyToClipboard({ title: "Скопировать пустое", content: "empty-list" }),
            ],
          }),
        }),
        List.Item({
          id: "loose",
          title: "Loose",
          actions: ActionPanel({
            children: [Action.CopyToClipboard({ content: "Loose" })],
          }),
        }),
      ],
    });

    const snapshot = normalizeCommandNode(root);
    expect(snapshot?.type).toBe("List");
    expect(snapshot ? listIsLoading(snapshot) : false).toBe(false);
    expect(snapshot ? listPlaceholder(snapshot) : "").toBe("Search notes");
    expect(snapshot ? listEmptyMessage(snapshot) : "").toBe("Пусто");
    expect(
      actionNodes(snapshot ? listEmptyActions(snapshot) : null).map((node) => node.type),
    ).toEqual(["Action.CopyToClipboard"]);
    expect(snapshot ? listDropdown(snapshot)?.defaultValue : "").toBe("all");
    expect(
      snapshot ? listDropdown(snapshot)?.sections.map((section) => section.title) : [],
    ).toEqual([null, "Тип"]);
    expect(snapshot ? listDropdown(snapshot)?.options.map((option) => option.value) : []).toEqual([
      "all",
      "notes",
      "tasks",
    ]);

    const items = snapshot ? listItems(snapshot) : [];
    expect(items).toHaveLength(2);
    expect(items[0]?.title).toBe("Hello");
    expect(items[0]?.icon).toBe("data:image/png;base64,abc");
    expect(items[0]?.keywords).toEqual(["note"]);
    expect(items[0]?.accessories).toEqual(["2 задачи", "Сегодня"]);
    expect(items[1]?.title).toBe("Loose");
    const sections = snapshot ? listSections(snapshot) : [];
    expect(sections.map((section) => section.title)).toEqual(["Заметки", null]);
    expect(sections.map((section) => section.items.map((item) => item.id))).toEqual([
      ["hello"],
      ["loose"],
    ]);
    expect(detailMarkdown(items[0]?.detail ?? null)).toBe("# Hello");
    expect(detailMetadataItems(items[0]?.detail ?? null)).toEqual([
      { id: "metadata:0", type: "label", title: "Тип", text: "Заметка", href: null },
    ]);
    const actions = actionNodes(items[0]?.actions ?? null);
    expect(actions.map((node) => node.type)).toEqual([
      "Action.CopyToClipboard",
      "Action.Paste",
      "Action.Push",
      "Action.OpenInBrowser",
      "Action.Open",
      "Action.ShowInFinder",
      "Action.Trash",
      "Action",
      "Action.LaunchCommand",
    ]);
    expect(actions[0]?.props.__callbackId).toBeUndefined();
    expect(actions.find((node) => node.type === "Action.Push")?.children[0]?.props.markdown).toBe(
      "# Pushed",
    );
    expect(actions.find((node) => node.type === "Action.OpenInBrowser")?.props.url).toBe(
      "https://example.com",
    );
    expect(actions.find((node) => node.type === "Action.Open")?.props.target).toBe(
      "C:/Temp/example.txt",
    );
    expect(actions.find((node) => node.type === "Action.ShowInFinder")?.props.path).toBe(
      "C:/Temp/example.txt",
    );
    expect(actions.find((node) => node.type === "Action.Trash")?.props.paths).toEqual([
      "C:/Temp/delete.txt",
    ]);
    expect(actions.find((node) => node.type === "Action.LaunchCommand")?.props.name).toBe("copy");
    expect(actions.find((node) => node.type === "Action.LaunchCommand")?.props.arguments).toEqual({
      from: "list",
    });
  });

  test("normalizes List controlled props and callbacks into host props", () => {
    const callbacks = new Map<string, (payload?: Record<string, unknown>) => unknown>();
    let searched = "";
    let selected: string | null = null;
    const root = List({
      searchText: "черновик",
      selectedItemId: "two",
      filtering: false,
      onSearchTextChange: (text) => {
        searched = text;
      },
      onSelectionChange: (id) => {
        selected = id;
      },
      children: [
        List.Item({ id: "one", title: "Первый" }),
        List.Item({ id: "two", title: "Второй" }),
      ],
    });

    const snapshot = normalizeCommandNode(root, {
      register(callback) {
        const id = `callback:${callbacks.size}`;
        callbacks.set(id, callback);
        return id;
      },
    });

    expect(snapshot?.type).toBe("List");
    expect(snapshot ? listSearchText(snapshot) : "").toBe("черновик");
    expect(snapshot ? listSelectedItemId(snapshot) : null).toBe("two");
    expect(snapshot ? listFiltering(snapshot) : true).toBe(false);
    expect(snapshot ? listSearchCallbackNode(snapshot)?.props.__onSearchTextChangeId : null).toBe(
      "callback:0",
    );
    expect(snapshot ? listSelectionCallbackNode(snapshot)?.props.__onSelectionChangeId : null).toBe(
      "callback:1",
    );

    callbacks.get("callback:0")?.({ text: "готово" });
    callbacks.get("callback:1")?.({ id: null });
    expect(searched).toBe("готово");
    expect(selected).toBe(null);
  });

  test("groups ActionPanel.Section actions for host rendering", () => {
    const root = ActionPanel({
      children: [
        Action({ title: "Корневое" }),
        Action.Pop({ shortcut: { modifiers: ["cmd"], key: "arrowLeft" } }),
        Action.PopToRoot({ title: "Домой" }),
        ActionPanel.Submenu({
          title: "Ещё",
          children: [Action.CopyToClipboard({ title: "Скопировать", content: "Проект" })],
        }),
        ActionPanel.Section({
          title: "Файлы",
          children: [
            Action.Open({ title: "Открыть", target: "C:\\Temp\\note.txt" }),
            Action.ShowInFinder({ title: "Показать", path: "C:\\Temp\\note.txt" }),
          ],
        }),
      ],
    });

    const snapshot = normalizeCommandNode(root);
    const sections = actionSections(snapshot);

    expect(sections.map((section) => section.title)).toEqual([null, "Файлы"]);
    expect(sections.map((section) => section.actions.map((action) => action.type))).toEqual([
      ["Action", "Action.Pop", "Action.PopToRoot", "ActionPanel.Submenu"],
      ["Action.Open", "Action.ShowInFinder"],
    ]);
    expect(actionNodes(snapshot).map((action) => action.type)).toEqual([
      "Action",
      "Action.Pop",
      "Action.PopToRoot",
      "Action.CopyToClipboard",
      "Action.Open",
      "Action.ShowInFinder",
    ]);
    expect(
      actionSubmenuActions(sections[0]?.actions[3] ?? snapshot).map((action) => action.type),
    ).toEqual(["Action.CopyToClipboard"]);
    expect(actionShortcut(actionNodes(snapshot)[1])?.label).toBe("Cmd arrowLeft");
  });

  test("normalizes and matches Action shortcuts by KeyboardEvent code", () => {
    const root = ActionPanel({
      children: [
        Action.CopyToClipboard({
          title: "Скопировать",
          content: "Проект",
          shortcut: Keyboard.Shortcut.Common.Copy,
        }),
      ],
    });

    const snapshot = normalizeCommandNode(root);
    const shortcut = actionShortcut(actionNodes(snapshot)[0]);

    expect(shortcut?.label).toBe("Cmd C");
    expect(shortcut?.code).toBe("KeyC");
    expect(
      matchesActionShortcut(shortcut, {
        altKey: false,
        code: "KeyC",
        ctrlKey: false,
        key: "с",
        metaKey: true,
        shiftKey: false,
      }),
    ).toBe(true);
    expect(
      matchesActionShortcut(shortcut, {
        altKey: false,
        code: "KeyC",
        ctrlKey: true,
        key: "c",
        metaKey: true,
        shiftKey: true,
      }),
    ).toBe(false);
  });
});

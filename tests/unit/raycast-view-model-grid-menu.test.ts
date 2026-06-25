import { describe, expect, test } from "bun:test";
import {
  Action,
  ActionPanel,
  Grid,
  List,
  MenuBarExtra,
} from "../../packages/raycast-api/src/index";
import { normalizeRaycastNode } from "../../platform/desktop/electron/raycast/view-model";
import { actionNodes } from "../../platform/desktop/src/raycast-host/model";
import {
  gridDropdown,
  gridEmptyActions,
  gridEmptyMessage,
  gridFiltering,
  gridItems,
  gridIsLoading,
  gridPlaceholder,
  gridSearchCallbackNode,
  gridSearchText,
  gridSections,
  gridSelectedItemId,
  gridSelectionCallbackNode,
} from "../../platform/desktop/src/raycast-host/model-grid";
import { listIsLoading } from "../../platform/desktop/src/raycast-host/model-list";
import { menuBarSections, menuBarTitle } from "../../platform/desktop/src/raycast-host/model-menu";

describe("Raycast grid and menu view model", () => {
  test("normalizes Grid sections, items, images, and actions into host model", () => {
    const root = Grid({
      searchBarPlaceholder: "Поиск игр",
      searchBarAccessory: Grid.Dropdown({
        defaultValue: "all",
        children: [
          Grid.Dropdown.Item({ value: "all", title: "Все" }),
          Grid.Dropdown.Section({
            title: "Статус",
            children: [
              Grid.Dropdown.Item({ value: "installed", title: "Установлено" }),
              Grid.Dropdown.Item({ value: "backlog", title: "Бэклог" }),
            ],
          }),
        ],
      }),
      children: [
        Grid.Section({
          title: "Игры",
          children: [
            Grid.Item({
              id: "one",
              title: "Первая",
              subtitle: "Steam",
              content: { source: "data:image/png;base64,abc" },
              keywords: ["game"],
              actions: ActionPanel({
                children: [Action.CopyToClipboard({ content: "one" })],
              }),
            }),
          ],
        }),
        Grid.EmptyView({
          title: "Нет элементов",
          actions: ActionPanel({
            children: [
              Action.CopyToClipboard({ title: "Скопировать пустую сетку", content: "empty-grid" }),
            ],
          }),
        }),
      ],
    });

    const snapshot = normalizeRaycastNode(root);
    expect(snapshot?.type).toBe("Grid");
    expect(snapshot ? gridIsLoading(snapshot) : false).toBe(false);
    expect(snapshot ? gridPlaceholder(snapshot) : "").toBe("Поиск игр");
    expect(snapshot ? gridEmptyMessage(snapshot) : "").toBe("Нет элементов");
    expect(
      actionNodes(snapshot ? gridEmptyActions(snapshot) : null).map((node) => node.type),
    ).toEqual(["Action.CopyToClipboard"]);
    expect(snapshot ? gridDropdown(snapshot)?.defaultValue : "").toBe("all");
    expect(
      snapshot ? gridDropdown(snapshot)?.sections.map((section) => section.title) : [],
    ).toEqual([null, "Статус"]);
    expect(snapshot ? gridDropdown(snapshot)?.options.map((option) => option.value) : []).toEqual([
      "all",
      "installed",
      "backlog",
    ]);

    const sections = snapshot ? gridSections(snapshot) : [];
    expect(sections.map((section) => section.title)).toEqual(["Игры"]);
    const items = snapshot ? gridItems(snapshot) : [];
    expect(items).toHaveLength(1);
    expect(items[0]?.image).toBe("data:image/png;base64,abc");
    expect(actionNodes(items[0]?.actions ?? null).map((node) => node.type)).toEqual([
      "Action.CopyToClipboard",
    ]);
  });

  test("normalizes Grid controlled props and callbacks into host props", () => {
    const callbacks = new Map<string, (payload?: Record<string, unknown>) => unknown>();
    let searched = "";
    let selected: string | null = null;
    const root = Grid({
      searchText: "игра",
      selectedItemId: "outer-wilds",
      filtering: false,
      onSearchTextChange: (text) => {
        searched = text;
      },
      onSelectionChange: (id) => {
        selected = id;
      },
      children: [
        Grid.Item({ id: "disco", title: "Disco Elysium" }),
        Grid.Item({ id: "outer-wilds", title: "Outer Wilds" }),
      ],
    });

    const snapshot = normalizeRaycastNode(root, {
      register(callback) {
        const id = `callback:${callbacks.size}`;
        callbacks.set(id, callback);
        return id;
      },
    });

    expect(snapshot?.type).toBe("Grid");
    expect(snapshot ? gridSearchText(snapshot) : "").toBe("игра");
    expect(snapshot ? gridSelectedItemId(snapshot) : null).toBe("outer-wilds");
    expect(snapshot ? gridFiltering(snapshot) : true).toBe(false);
    expect(snapshot ? gridSearchCallbackNode(snapshot)?.props.__onSearchTextChangeId : null).toBe(
      "callback:0",
    );
    expect(snapshot ? gridSelectionCallbackNode(snapshot)?.props.__onSelectionChangeId : null).toBe(
      "callback:1",
    );

    callbacks.get("callback:0")?.({ text: "космос" });
    callbacks.get("callback:1")?.({ id: "disco" });
    expect(searched).toBe("космос");
    expect(selected).toBe("disco");
  });

  test("normalizes MenuBarExtra sections, submenus, and callbacks into host model", () => {
    const callbacks = new Map<string, (payload?: Record<string, unknown>) => unknown>();
    const root = MenuBarExtra({
      title: "Статус",
      children: [
        MenuBarExtra.Section({
          title: "Синхронизация",
          children: [
            MenuBarExtra.Item({
              id: "sync",
              title: "Синхронизировать",
              subtitle: "Сейчас",
              onAction: () => callbacks.set("ran", () => true),
            }),
            MenuBarExtra.Submenu({
              id: "more",
              title: "Ещё",
              children: [MenuBarExtra.Item({ id: "logs", title: "Логи" })],
            }),
          ],
        }),
      ],
    });

    const snapshot = normalizeRaycastNode(root, {
      register(callback) {
        const id = `callback:${callbacks.size}`;
        callbacks.set(id, callback);
        return id;
      },
    });

    expect(snapshot?.type).toBe("MenuBarExtra");
    expect(snapshot ? menuBarTitle(snapshot) : "").toBe("Статус");
    const sections = snapshot ? menuBarSections(snapshot) : [];
    expect(sections.map((section) => section.title)).toEqual(["Синхронизация"]);
    expect(sections[0]?.items.map((item) => [item.id, item.title, item.subtitle])).toEqual([
      ["sync", "Синхронизировать", "Сейчас"],
      ["more", "Ещё", null],
    ]);
    expect(sections[0]?.items[1]?.children.map((item) => item.title)).toEqual(["Логи"]);
    expect(sections[0]?.items[0]?.node.props.__callbackId).toBe("callback:0");
    callbacks.get("callback:0")?.();
    expect(callbacks.get("ran")?.()).toBe(true);
  });

  test("extracts loading state from List and Grid roots", () => {
    const listSnapshot = normalizeRaycastNode(List({ isLoading: true }));
    const gridSnapshot = normalizeRaycastNode(Grid({ isLoading: true }));

    expect(listSnapshot ? listIsLoading(listSnapshot) : false).toBe(true);
    expect(gridSnapshot ? gridIsLoading(gridSnapshot) : false).toBe(true);
  });
});

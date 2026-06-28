import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { textProp, type CommandMenuBarItemModel, type CommandMenuBarSectionModel } from "./model";

export function menuBarTitle(snapshot: CommandSnapshotNode): string {
  return textProp(snapshot.props.title) ?? textProp(snapshot.props.tooltip) ?? "Menu Bar";
}

export function menuBarIsLoading(snapshot: CommandSnapshotNode): boolean {
  return snapshot.props.isLoading === true;
}

export function menuBarSections(snapshot: CommandSnapshotNode): CommandMenuBarSectionModel[] {
  const sections: CommandMenuBarSectionModel[] = [];
  let looseItems: CommandSnapshotNode[] = [];
  let itemIndex = 0;

  function menuItemModel(node: CommandSnapshotNode): CommandMenuBarItemModel {
    const title = textProp(node.props.title) ?? "Без названия";
    const id = textProp(node.props.id) ?? `${itemIndex}:${title}`;
    itemIndex += 1;
    return {
      id,
      title,
      subtitle: textProp(node.props.subtitle),
      node,
      children: node.children
        .filter(
          (child) => child.type === "MenuBarExtra.Item" || child.type === "MenuBarExtra.Submenu",
        )
        .map(menuItemModel),
    };
  }

  function flushLooseItems(): void {
    if (looseItems.length === 0) return;
    sections.push({
      id: `menubar-section:root:${sections.length}`,
      title: null,
      items: looseItems.map(menuItemModel),
    });
    looseItems = [];
  }

  for (const child of snapshot.children) {
    if (child.type === "MenuBarExtra.Section") {
      flushLooseItems();
      sections.push({
        id: textProp(child.props.id) ?? `menubar-section:${sections.length}`,
        title: textProp(child.props.title),
        items: child.children
          .filter(
            (item) => item.type === "MenuBarExtra.Item" || item.type === "MenuBarExtra.Submenu",
          )
          .map(menuItemModel),
      });
    } else if (child.type === "MenuBarExtra.Item" || child.type === "MenuBarExtra.Submenu") {
      looseItems.push(child);
    }
  }
  flushLooseItems();

  return sections;
}

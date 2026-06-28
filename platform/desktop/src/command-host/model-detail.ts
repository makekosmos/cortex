import type { CommandSnapshotNode } from "../../shared/command-ipc";
import { collectNodes, findFirst, textProp } from "./model";
import type { CommandMetadataItemModel } from "./model";

export function detailMarkdown(node: CommandSnapshotNode | null): string {
  if (!node) return "";
  const markdown = textProp(node.props.markdown);
  if (markdown) return markdown;
  return node.children
    .filter((child) => child.type !== "Detail.Metadata" && child.type !== "ActionPanel")
    .map((child) => child.text ?? detailMarkdown(child))
    .join("\n");
}

function detailMetadata(node: CommandSnapshotNode | null): CommandSnapshotNode | null {
  return node ? findFirst(node, "Detail.Metadata") : null;
}

export function detailActions(node: CommandSnapshotNode | null): CommandSnapshotNode | null {
  return node ? findFirst(node, "ActionPanel") : null;
}

export function detailMetadataItems(node: CommandSnapshotNode | null): CommandMetadataItemModel[] {
  const metadata = detailMetadata(node);
  if (!metadata) return [];
  const items: CommandMetadataItemModel[] = [];
  let index = 0;

  for (const child of metadata.children) {
    const id = textProp(child.props.id) ?? `metadata:${index++}`;
    if (child.type === "Detail.Metadata.Separator") {
      items.push({ id, type: "separator" });
      continue;
    }
    if (child.type === "Detail.Metadata.TagList") {
      const tags = collectNodes(child, "Detail.Metadata.TagList.Item")
        .map((item) => textProp(item.props.text) ?? textProp(item.props.title))
        .filter((tag): tag is string => tag !== null);
      items.push({
        id,
        type: "tags",
        title: textProp(child.props.title) ?? "",
        tags,
      });
      continue;
    }
    if (child.type === "Detail.Metadata.Link") {
      items.push({
        id,
        type: "link",
        title: textProp(child.props.title) ?? "",
        text: textProp(child.props.text) ?? textProp(child.props.target) ?? "",
        href: textProp(child.props.target),
      });
      continue;
    }
    if (child.type === "Detail.Metadata.Label") {
      items.push({
        id,
        type: "label",
        title: textProp(child.props.title) ?? "",
        text: textProp(child.props.text) ?? "",
        href: null,
      });
    }
  }

  return items;
}

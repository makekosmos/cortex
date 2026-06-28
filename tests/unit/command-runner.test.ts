import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

export function writeFixture(rootName: string): {
  extensionDir: string;
  userDataDir: string;
  copied: string[];
  feedback: string[];
  system: string[];
} {
  const root = path.resolve(".tmp", rootName);
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

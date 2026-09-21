import { app, shell } from "electron";
import fs from "node:fs";
import path from "node:path";
import { SAFE_ID } from "./host-api";

export type ShortcutApp = {
  id: string;
  name: string;
  enabled: boolean;
  revoked?: boolean;
  iconPath?: string;
};
type Entry = { id: string; file: string };
const INDEX = ".kosmos-desktop-host-shortcuts.json";
const WINDOWS_INVALID = /[<>:"/\\|?*]/u;
const WINDOWS_RESERVED = /^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\..*)?$/iu;

function quoteWindowsArg(value: string): string {
  return `"${value.replaceAll('"', '\\"')}"`;
}
export function shortcutArgs(
  appPath: string | undefined,
  id: string,
  defaultApp = process.defaultApp,
): string {
  const prefix = defaultApp && appPath ? `${quoteWindowsArg(path.win32.resolve(appPath))} ` : "";
  return `${prefix}--open-app=${id}`;
}

function dir(): string {
  return (
    process.env.KOSMOS_SHORTCUT_DIR ||
    path.join(app.getPath("appData"), "Microsoft", "Windows", "Start Menu", "Programs", "Kosmos")
  );
}
function readIndex(root: string): Entry[] {
  const resolvedRoot = path.resolve(root);
  try {
    const value = JSON.parse(fs.readFileSync(path.join(root, INDEX), "utf8"));
    return Array.isArray(value)
      ? value
          .filter(
            (e): e is Entry =>
              e?.id?.constructor === String &&
              SAFE_ID.test(e.id) &&
              e?.file?.constructor === String &&
              path.dirname(path.resolve(e.file)) === resolvedRoot &&
              path.extname(e.file).toLowerCase() === ".lnk",
          )
          .map((e) => ({ ...e, file: path.resolve(e.file) }))
      : [];
  } catch {
    return [];
  }
}

function shortcutBaseName(name: string, id: string): string {
  let base = [...name]
    .map((character) =>
      character.charCodeAt(0) < 32 || WINDOWS_INVALID.test(character) ? " " : character,
    )
    .join("")
    .replace(/\s+/g, " ")
    .trim();
  base = base
    .replace(/[. ]+$/g, "")
    .slice(0, 240)
    .replace(/[. ]+$/g, "");
  if (!base) base = id;
  return WINDOWS_RESERVED.test(base) ? `_${base}` : base;
}

function shortcutFiles(apps: ShortcutApp[], root: string): Map<string, string> {
  const files = new Map<string, string>();
  const used = new Set<string>();
  const eligible = apps
    .filter((item) => item.enabled && !item.revoked && SAFE_ID.test(item.id))
    .map((item) => ({ item, base: shortcutBaseName(item.name, item.id) }))
    .sort((a, b) => {
      const left = `${a.base.toLowerCase()}\0${a.item.id}`;
      const right = `${b.base.toLowerCase()}\0${b.item.id}`;
      return left < right ? -1 : left > right ? 1 : 0;
    });
  for (const { item, base } of eligible) {
    let candidate = base;
    for (let suffix = 2; used.has(candidate.toLowerCase()); suffix += 1)
      candidate = `${base} (${suffix})`;
    used.add(candidate.toLowerCase());
    files.set(item.id, path.join(root, `${candidate}.lnk`));
  }
  return files;
}

export function reconcileShortcuts(apps: ShortcutApp[], executable = process.execPath): void {
  if (process.platform !== "win32" && !process.env.KOSMOS_SHORTCUT_DIR) return;
  const root = dir();
  fs.mkdirSync(root, { recursive: true });
  const previous = readIndex(root);
  const files = shortcutFiles(apps, root);
  const next: Entry[] = [];
  for (const old of previous) {
    const current = files.get(old.id);
    if (!current || path.resolve(current) !== path.resolve(old.file)) {
      try {
        fs.unlinkSync(old.file);
      } catch {
        /* already absent */
      }
    }
  }
  const appPath = process.defaultApp
    ? process.argv
        .slice(1)
        .find((argument) => !argument.startsWith("-") && /\.(?:c?m?js)$/iu.test(argument))
    : undefined;
  for (const item of apps) {
    if (!item.enabled || item.revoked || !SAFE_ID.test(item.id)) continue;
    const file = files.get(item.id);
    if (!file) continue;
    const shortcut = {
      target: executable,
      args: shortcutArgs(appPath, item.id),
      description: `Kosmos: ${item.name}`,
    };
    const shortcutWithIcon =
      item.iconPath && fs.existsSync(item.iconPath)
        ? { ...shortcut, icon: item.iconPath, iconIndex: 0 }
        : shortcut;
    if (shell.writeShortcutLink(file, "create", shortcutWithIcon)) next.push({ id: item.id, file });
  }
  const index = path.join(root, INDEX);
  const temp = path.join(root, `${INDEX}.tmp`);
  fs.writeFileSync(temp, JSON.stringify(next), "utf8");
  // Windows rename does not replace an existing destination; this index is
  // Host-owned, so replacing it is safe and keeps reconciliation repeatable.
  if (process.platform === "win32") fs.rmSync(index, { force: true });
  fs.renameSync(temp, index);
}

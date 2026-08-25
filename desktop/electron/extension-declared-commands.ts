import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { listExtensions, loadExtensionManifest, resolveExtensionDir } from "./extension-manifest";
import { isRecord, isString } from "../src/shared/runtimeGuards";

/**
 * Manifest-declared command, обогащённая resolved icon data:URI + metadata
 * extension'а. Этот формат уже близок к `CommandRecord` из ipc-types.ts.
 */
export interface DeclaredCommand {
  id: string;
  title: string;
  subtitle?: string;
  category: "open" | "action";
  kind: "app" | "command";
  appName: string;
  icon?: string;
  /** Source extension id (для exec). */
  extensionId: string;
  /** Hash-route из manifest (если задан). */
  route?: string;
  /** Mode: `open`, `action`, or command runner. */
  mode: "open" | "action" | "command-view" | "command-no-view" | "command-menu-bar";
  /** Command name for `kind: "command-extension"` packages. */
  commandName?: string;
}

interface IconCacheEntry {
  uri: string | null;
  mtimeMs: number;
}

const iconDataUriCache = new Map<string, IconCacheEntry>();
const declaredCommandIdRe = /^[a-z0-9][a-z0-9:_-]*$/;

function iconFileAsDataUri(iconPath: string): string {
  const buf = readFileSync(iconPath);
  const ext = path.extname(iconPath).toLowerCase();
  const mime =
    ext === ".svg"
      ? "image/svg+xml"
      : ext === ".jpg" || ext === ".jpeg"
        ? "image/jpeg"
        : ext === ".webp"
          ? "image/webp"
          : "image/png";
  return `data:${mime};base64,${buf.toString("base64")}`;
}

/**
 * Возвращает icon extension'а как `data:image/png;base64,...` URI, или undefined
 * если у extension'а нет icon (нет поля в manifest или файл отсутствует).
 * Кеширует по mtime файла — повторные вызовы дешёвые, обновление файла
 * автоматически перечитывается.
 */
function extensionIconDataUri(id: string): string | undefined {
  const manifest = loadExtensionManifest(id);
  if (!manifest || !manifest.icon) return;
  const dir = resolveExtensionDir(id);
  if (!dir) return;
  const iconPath = path.resolve(path.join(dir, manifest.icon));
  if (!iconPath.startsWith(path.resolve(dir) + path.sep) && iconPath !== path.resolve(dir)) return;
  if (!existsSync(iconPath)) return;
  const stat = statSync(iconPath);
  const cached = iconDataUriCache.get(id);
  if (cached && cached.mtimeMs === stat.mtimeMs) {
    return cached.uri ?? undefined;
  }
  try {
    const uri = iconFileAsDataUri(iconPath);
    iconDataUriCache.set(id, { uri, mtimeMs: stat.mtimeMs });
    return uri;
  } catch (e) {
    console.error(`[kepler-shell] failed to read icon for ${id}:`, e);
    iconDataUriCache.set(id, { uri: null, mtimeMs: stat.mtimeMs });
    return;
  }
}

/**
 * Резолвит произвольный icon-path из манифеста (относительно extension dir)
 * в data:URI. Используется для `commands[].icon`. Возвращает undefined
 * если файл отсутствует / выходит за пределы extension dir / unreadable.
 */
function readManifestIconAsDataUri(id: string, iconRel: string): string | undefined {
  if (!iconRel || !isString(iconRel)) return;
  const dir = resolveExtensionDir(id);
  if (!dir) return;
  const resolved = path.resolve(path.join(dir, iconRel));
  const dirResolved = path.resolve(dir);
  if (!resolved.startsWith(dirResolved + path.sep) && resolved !== dirResolved) return;
  if (!existsSync(resolved)) return;
  try {
    return iconFileAsDataUri(resolved);
  } catch {
    return;
  }
}

/**
 * Сканирует все установленные extension'ы (включая dev tree), читает
 * `manifest.commands[]`, билдит DeclaredCommand[]. Idempotent / lightweight —
 * можно дёргать на каждом `kepler:commands:list`.
 */
export function loadDeclaredCommands(): DeclaredCommand[] {
  const out: DeclaredCommand[] = [];
  for (const manifest of listExtensions()) {
    if (!Array.isArray(manifest.commands)) continue;
    for (const cmd of manifest.commands) {
      if (!isRecord(cmd)) continue;
      if (!isString(cmd.id) || !declaredCommandIdRe.test(cmd.id)) {
        console.warn(
          `[kepler-shell] extension '${manifest.id}' command id invalid: ${JSON.stringify(cmd.id)} — skipped`,
        );
        continue;
      }
      if (!isString(cmd.title) || cmd.title.trim().length === 0) {
        console.warn(
          `[kepler-shell] extension '${manifest.id}' command '${cmd.id}' missing title — skipped`,
        );
        continue;
      }
      const fullId = `${manifest.id}:${cmd.id}`;
      const icon = cmd.icon
        ? readManifestIconAsDataUri(manifest.id, cmd.icon)
        : extensionIconDataUri(manifest.id);
      out.push({
        id: fullId,
        title: cmd.title,
        subtitle: cmd.subtitle ?? manifest.name,
        category: cmd.mode === "action" || cmd.mode === "command-no-view" ? "action" : "open",
        kind: cmd.kind ?? "command",
        appName: manifest.name,
        icon,
        extensionId: manifest.id,
        route: cmd.route,
        mode: cmd.mode ?? "open",
        commandName: manifest.kind === "command-extension" ? cmd.id : undefined,
      });
    }
  }
  return out;
}

/**
 * Lookup конкретной DeclaredCommand по полному id (`<ext>:<cmd>`).
 * Используется `kepler:commands:invoke` для resolve'а нужного route +
 * extension id перед openExtension/commands.invoke.
 */
export function findDeclaredCommand(fullId: string): DeclaredCommand | null {
  for (const cmd of loadDeclaredCommands()) {
    if (cmd.id === fullId) return cmd;
  }
  return null;
}

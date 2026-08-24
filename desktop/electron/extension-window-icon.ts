import { existsSync } from "node:fs";
import path from "node:path";

export function resolveExtensionWindowIcon(
  extensionDir: string,
  icon: string | undefined,
  kosmosIcon?: string,
): string | undefined {
  if (kosmosIcon && existsSync(kosmosIcon)) return kosmosIcon;
  if (!icon) return;
  const root = path.resolve(extensionDir);
  const candidate = path.resolve(root, icon);
  if (!candidate.startsWith(`${root}${path.sep}`) || !existsSync(candidate)) return;
  return candidate;
}

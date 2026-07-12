import type { AgentsDiffFile } from "@kosmos/ark/agents";

export type ChangeTreeRow =
  | { key: string; kind: "directory"; name: string; depth: number }
  | { key: string; kind: "file"; name: string; depth: number; file: AgentsDiffFile };

export function buildChangeTree(files: AgentsDiffFile[]): ChangeTreeRow[] {
  const rows: ChangeTreeRow[] = [];
  const seenDirectories = new Set<string>();
  for (const file of [...files].sort((a, b) => a.path.localeCompare(b.path))) {
    const parts = file.path.replaceAll("\\", "/").split("/").filter(Boolean);
    for (let depth = 0; depth < parts.length - 1; depth += 1) {
      const key = parts.slice(0, depth + 1).join("/");
      if (seenDirectories.has(key)) continue;
      seenDirectories.add(key);
      rows.push({ key: `directory:${key}`, kind: "directory", name: parts[depth]!, depth });
    }
    rows.push({
      key: `file:${file.path}`,
      kind: "file",
      name: parts.at(-1) ?? file.path,
      depth: Math.max(0, parts.length - 1),
      file,
    });
  }
  return rows;
}

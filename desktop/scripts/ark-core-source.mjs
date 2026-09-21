import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const ARK_CORE_SOURCE = "core/crates/ark-core";
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
export const ARK_CORE_SOURCE_DIR = path.join(repoRoot, ARK_CORE_SOURCE);

function hashTree(dir) {
  const digest = createHash("sha256");
  const walk = (current) => {
    for (const entry of readdirSync(current, { withFileTypes: true }).sort((a, b) =>
      a.name.localeCompare(b.name),
    )) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile()) {
        digest.update(path.relative(dir, full));
        digest.update("\0");
        digest.update(readFileSync(full));
        digest.update("\0");
      }
    }
  };
  walk(dir);
  return digest.digest("hex");
}

let cachedSourceId;
// Content hash of the in-tree ark-core crate. Keyed on file contents rather
// than a commit so dirty worktrees cannot be served stale cache entries.
export function arkCoreSourceId() {
  if (!cachedSourceId) cachedSourceId = `sha256:${hashTree(ARK_CORE_SOURCE_DIR)}`;
  return cachedSourceId;
}

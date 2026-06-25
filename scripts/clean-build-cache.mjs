import { readdir, rm, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = new Set(process.argv.slice(2));
const dryRun = args.has("--dry-run");
const includeAll = args.has("--all");

const alwaysClean = [
  ".tmp/cargo-*",
  "target-*",
  "shell/release",
  "shell/target",
  "platform/desktop/release",
  "crates/ark-core/rust/.tmp",
  "crates/ark-core/rust/target",
  "services/kepler-backend/target",
];

const allClean = ["target", ".bun_cache"];

function escapeRegex(value) {
  return value.replace(/[|\\{}()[\]^$+?.]/g, "\\$&");
}

function globToRegex(pattern) {
  return new RegExp(`^${pattern.split("*").map(escapeRegex).join("[^/]*")}$`);
}

async function pathExists(targetPath) {
  try {
    await stat(targetPath);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}

async function expandPattern(pattern) {
  const normalized = pattern.replaceAll("\\", "/");

  if (!normalized.includes("*")) {
    const absolute = path.join(root, normalized);
    return (await pathExists(absolute)) ? [absolute] : [];
  }

  const slash = normalized.lastIndexOf("/");
  const parentPattern = slash === -1 ? "." : normalized.slice(0, slash);
  const basenamePattern = slash === -1 ? normalized : normalized.slice(slash + 1);

  if (parentPattern.includes("*")) {
    throw new Error(`Nested wildcard parents are not supported: ${pattern}`);
  }

  const parent = path.join(root, parentPattern);
  const entries = await stat(parent).then(
    async () => readdir(parent, { withFileTypes: true }),
    (error) => {
      if (error?.code === "ENOENT") {
        return [];
      }
      throw error;
    },
  );
  const regex = globToRegex(basenamePattern);

  return entries
    .filter((entry) => entry.isDirectory() && regex.test(entry.name))
    .map((entry) => path.join(parent, entry.name));
}

const patterns = includeAll ? [...alwaysClean, ...allClean] : alwaysClean;
const targets = [...new Set((await Promise.all(patterns.map(expandPattern))).flat())].sort((a, b) =>
  a.localeCompare(b),
);

if (targets.length === 0) {
  console.log("No build cache directories found.");
  process.exit(0);
}

const failures = [];

for (const target of targets) {
  const relative = path.relative(root, target) || ".";
  if (dryRun) {
    console.log(`would remove ${relative}`);
    continue;
  }

  console.log(`removing ${relative}`);
  try {
    await rm(target, { force: true, recursive: true });
  } catch (error) {
    failures.push({ relative, error });
    console.error(`failed to remove ${relative}: ${error.message}`);
  }
}

if (failures.length > 0) {
  console.error(
    `Failed to remove ${failures.length} build cache director${failures.length === 1 ? "y" : "ies"}.`,
  );
  process.exit(1);
}

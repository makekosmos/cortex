import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";

const root = new URL("../", import.meta.url);
const generated = "core/ark/packages/ark/src/generated";

if (!existsSync(new URL(`${generated}/`, root))) {
  throw new Error(`Generated bindings directory is missing: ${generated}`);
}

const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" });
const expectedExportPath = "../../../core/ark/packages/ark/src/generated/";
const exportSources = git("grep", "-l", "ts(export", "--", "crates/ark-core/src")
  .trim()
  .split(/\r?\n/)
  .filter(Boolean);
for (const sourcePath of exportSources) {
  const source = readFileSync(new URL(sourcePath, root), "utf8");
  const exportPaths = [...source.matchAll(/export_to\s*=\s*"([^"]+)"/g)];
  if (!exportPaths.length || exportPaths.some((match) => match[1] !== expectedExportPath)) {
    throw new Error(`${sourcePath} does not export into tracked bindings: ${generated}`);
  }
}
const before = git("diff", "--", generated);
if (before) {
  throw new Error(
    `Generated bindings have unstaged changes; stage or resolve them first:\n${before}`,
  );
}

execFileSync(
  "cargo",
  ["test", "--manifest-path", "crates/ark-core/Cargo.toml", "--features", "ts-rs", "--quiet"],
  { cwd: root, stdio: "inherit" },
);

execFileSync("bun", ["x", "oxfmt", generated], { cwd: root, stdio: "inherit" });

const diff = git("diff", "--", generated);
if (diff) {
  console.error("Generated bindings are stale. Regenerate them, then commit the diff:");
  console.error(diff);
  process.exitCode = 1;
}

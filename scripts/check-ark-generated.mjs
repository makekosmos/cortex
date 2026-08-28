import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";

const root = new URL("../", import.meta.url);
const generated = "core/ark/packages/ark/src/generated";

if (!existsSync(new URL(`${generated}/`, root))) {
  throw new Error(`Generated bindings directory is missing: ${generated}`);
}

const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" });
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

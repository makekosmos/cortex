import { spawnSync } from "node:child_process";

const extensions = /\.(c|m)?(js|ts)x?$|\.vue$|\.json$|\.ya?ml$/;
const base = process.env.FORMAT_BASE;
const staged = process.argv.includes("--staged");
const args = base
  ? /^0+$/.test(base)
    ? ["ls-tree", "-r", "--name-only", "HEAD"]
    : ["diff", "--name-only", "--diff-filter=ACMR", `${base}...HEAD`]
  : staged
    ? ["diff", "--cached", "--name-only", "--diff-filter=ACMR"]
    : ["diff", "--name-only", "--diff-filter=ACMR", "HEAD"];
const gitEnv = { ...process.env };
for (const key of [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_COMMON_DIR",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_CEILING_DIRECTORIES",
  "GIT_PREFIX",
])
  delete gitEnv[key];
const changed = spawnSync("git", args, { encoding: "utf8", env: gitEnv });
if (changed.error || changed.status !== 0) process.exit(changed.status ?? 1);

const files = changed.stdout.split(/\r?\n/).filter((file) => extensions.test(file));

if (files.length === 0) process.exit(0);

const result = spawnSync("oxfmt", ["--check", ...files], {
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(result.status ?? 1);

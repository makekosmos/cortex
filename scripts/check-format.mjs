import { spawnSync } from "node:child_process";

const extensions = /\.(c|m)?(js|ts)x?$|\.vue$|\.json$/;
const base = process.env.FORMAT_BASE;
const args = base ? ["diff", "--name-only", `${base}...HEAD`] : ["diff", "--name-only", "HEAD"];
const files = spawnSync("git", args, { encoding: "utf8" })
  .stdout.split(/\r?\n/)
  .filter((file) => extensions.test(file));

if (files.length === 0) process.exit(0);

const result = spawnSync("oxfmt", ["--check", ...files], { stdio: "inherit" });
process.exit(result.status ?? 1);

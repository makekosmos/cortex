import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

const root = path.resolve(import.meta.dirname, "..");
const scriptDirs = [
  "scripts",
  "desktop/scripts",
  "manager/scripts",
  "host/scripts",
  "runtime/scripts",
];

// Node >=18.20.2 (CVE-2024-27980) throws EINVAL when spawn/spawnSync/execFile
// targets a .cmd/.bat without `shell`. Plain "pnpm" (no .exe) also fails to
// resolve on Windows without a shell. These calls must always carry `shell`.
const spawnRe = /\b(?:spawnSync|spawn|execFileSync|execFile)\s*\(/g;
const cmdBindingRe =
  /(?:const|let|var)\s+([A-Za-z_$][\w$]*)\s*=\s*[^;]*["'`](?:[^"'`]*\.cmd|pnpm)["'`]/g;

function* callSites(source) {
  for (const match of source.matchAll(spawnRe)) {
    let depth = 0;
    let end = match.index + match[0].length - 1;
    for (; end < source.length; end++) {
      if (source[end] === "(") depth++;
      else if (source[end] === ")" && --depth === 0) break;
    }
    yield { text: source.slice(match.index, end + 1), index: match.index };
  }
}

function firstArg(callText) {
  const open = callText.indexOf("(");
  let depth = 0;
  for (let i = open; i < callText.length; i++) {
    const ch = callText[i];
    if (ch === "(") depth++;
    else if (ch === ")") depth--;
    else if (ch === "," && depth === 1) return callText.slice(open + 1, i).trim();
  }
  return callText.slice(open + 1, -1).trim();
}

function lineOf(source, index) {
  return source.slice(0, index).split("\n").length;
}

for (const dir of scriptDirs) {
  const absDir = path.join(root, dir);
  if (!existsSync(absDir)) continue;
  // *.test.mjs files may contain deliberately unsafe spawns (EINVAL probes)
  // and any real misuse fails at test runtime; only production scripts need
  // static enforcement.
  for (const file of readdirSync(absDir).filter(
    (name) => name.endsWith(".mjs") && !name.endsWith(".test.mjs"),
  )) {
    const relative = `${dir}/${file}`;
    const source = readFileSync(path.join(absDir, file), "utf8");

    test(`${relative} spawns .cmd/.bat/pnpm only with shell`, () => {
      const cmdVars = new Set();
      for (const match of source.matchAll(cmdBindingRe)) cmdVars.add(match[1]);
      for (const call of callSites(source)) {
        const command = firstArg(call.text);
        const needsShell =
          /["'`][^"'`]*\.(?:cmd|bat)["'`]/.test(command) ||
          /^["'`]pnpm["'`]$/.test(command) ||
          [...cmdVars].some((name) => new RegExp(`\\b${name}\\b`).test(command));
        if (needsShell) {
          const line = source.split("\n")[lineOf(source, call.index) - 1];
          if (!line.includes("spawn-shell-contract: allow"))
            assert.match(
              call.text,
              /\bshell\b/,
              `${relative}:${lineOf(source, call.index)} spawns ${command} without shell`,
            );
        }
      }
    });

    test(`${relative} never runs "pnpm run <file>.mjs"`, () => {
      for (const call of callSites(source)) {
        const command = firstArg(call.text);
        if (!/pnpm/.test(command)) continue;
        assert.doesNotMatch(
          call.text,
          /["'`]run["'`]\s*,\s*["'`][^"'`]*\.mjs["'`]/,
          `${relative}:${lineOf(source, call.index)} passes a file path to "pnpm run"; use process.execPath`,
        );
      }
    });
  }
}

test(
  "Node throws EINVAL for .cmd spawns without shell",
  { skip: process.platform !== "win32" },
  async () => {
    const dir = await mkdtemp(path.join(os.tmpdir(), "spawn-shell-"));
    const shim = path.join(dir, "probe.cmd");
    writeFileSync(shim, "@exit /b 0\r\n");
    // Deliberately unsafe: proves the EINVAL contract this file guards.
    const denied = spawnSync(shim, { encoding: "utf8" }); // spawn-shell-contract: allow
    assert.equal(denied.error?.code, "EINVAL");
    const allowed = spawnSync(shim, { encoding: "utf8", shell: true });
    assert.equal(allowed.status, 0);
  },
);

test("pnpm runs through the fixed spawn pattern from a package cwd", async (t) => {
  const pnpm = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
  const dir = await mkdtemp(path.join(os.tmpdir(), "spawn-shell-"));
  writeFileSync(
    path.join(dir, "package.json"),
    JSON.stringify({ scripts: { probe: 'node -e "process.exit(0)"' } }),
  );
  const run = spawnSync(pnpm, ["run", "probe"], {
    cwd: dir,
    encoding: "utf8",
    shell: pnpm.endsWith(".cmd"),
  });
  if (run.error?.code === "ENOENT") return t.skip("pnpm is not on PATH");
  assert.equal(run.status, 0, run.stderr || run.stdout);
  const exec = spawnSync(pnpm, ["exec", "node", "--version"], {
    cwd: dir,
    encoding: "utf8",
    shell: pnpm.endsWith(".cmd"),
  });
  assert.equal(exec.status, 0, exec.stderr || exec.stdout);
});

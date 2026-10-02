// KOS-309 regression: NSIS `SetOutPath` also changes the installer process'
// own working directory, and Windows refuses to rename or delete a
// directory that is some process' CWD. 0.10.2 staged into resources.next
// via `SetOutPath "$INSTDIR\resources.next"` and then could not rename it.
//
// For every `Rename`/`RMDir` in the script, this test fails when the most
// recent `SetOutPath` on that code path is the target itself or a child of
// it — the target is then pinned by our own process.
//
// Known limits (stated so nobody over-trusts it):
//  - linear scan per control-flow path; `IfFileExists`/`Goto` branches are
//    scanned in file order with the CWD carried through — a Goto jumping
//    *back* over a SetOutPath would fool it, but the script has none.
//  - `!insertmacro` bodies are expanded inline at the insertion point.
//  - `Call`-ed functions are not expanded (none of them SetOutPath today;
//    a future one that does would need this test extended).
//  - at `Section "Uninstall"` the CWD is seeded as $INSTDIR: the
//    uninstaller exe lives there, so Explorer/Apps&Features start it with
//    $INSTDIR as CWD.
import { expect, test } from "../test-support/node-test.mjs";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const buildDir = path.dirname(fileURLToPath(import.meta.url));
const installer = readFileSync(path.join(buildDir, "installer.nsi"), "utf8");

// Collect `!macro Name … !macroend` bodies for inline expansion.
const macros = new Map();
{
  const re = /!macro (\w+)\b([\s\S]*?)!macroend/g;
  for (const m of installer.matchAll(re)) macros.set(m[1], m[2]);
}
expect(macros.has("StopProductProcesses")).toBeTruthy();

// Strip comments and macro-definition blocks, then expand insertmacros.
function* relevantLines(text, depth = 0) {
  expect(depth).toBeLessThan(4);
  // At top level, `!macro … !macroend` blocks are definitions — they run
  // nowhere by themselves and are scanned where `!insertmacro` puts them.
  let inMacroDef = false;
  for (const raw of text.split("\n")) {
    // Strip a `;` comment only outside a quoted string — cheap check: no
    // '"' before the ';'.
    const semi = raw.indexOf(";");
    const line = (semi >= 0 && !raw.slice(0, semi).includes('"') ? raw.slice(0, semi) : raw).trim();
    if (depth === 0) {
      if (/^!macro /.test(line)) {
        inMacroDef = true;
        continue;
      }
      if (inMacroDef) {
        if (line === "!macroend") inMacroDef = false;
        continue;
      }
    }
    const insert = line.match(/^!insertmacro (\w+)/);
    if (insert) {
      // Only script-local macros are expanded — MUI/NSIS built-ins
      // (MUI_PAGE_*, MUI_LANGUAGE, …) set no out-path we control.
      const body = macros.get(insert[1]);
      if (body) yield* relevantLines(body, depth + 1);
      continue;
    }
    yield line;
  }
}

function normalizeNsisPath(p) {
  return p.replace(/^"|"$/g, "").replaceAll("/", "\\").replace(/\\+$/, "").toLowerCase();
}

// The target is pinned when the CWD is the target itself or sits inside it —
// Windows refuses to rename/delete a directory that is some process' CWD.
function isPinnedBy(cwd, target) {
  if (!cwd) return false;
  const t = normalizeNsisPath(target);
  const c = normalizeNsisPath(cwd);
  return c === t || c.startsWith(t + "\\");
}

const violations = [];
// Unknown initial CWD — the first SetOutPath wins before any check can fire.
let cwd = null;
let scope = "top";
for (const line of relevantLines(installer)) {
  const section = line.match(/^Section "(\w+)"/);
  if (section) {
    scope = section[1];
    // The uninstaller exe is written into $INSTDIR — its initial CWD.
    if (section[1] === "Uninstall") cwd = '"$INSTDIR"';
    continue;
  }
  const out = line.match(/^SetOutPath\s+(.+)$/);
  if (out) {
    cwd = out[1];
    continue;
  }
  const targets = [];
  const rename = line.match(/^Rename\s+("[^"]+"|[^\s]+)\s+("[^"]+"|[^\s]+)/);
  if (rename) targets.push(rename[1], rename[2]);
  const rmdir = line.match(/^RMDir\s+(?:\/r\s+)?("[^"]+"|[^\s]+)/);
  if (rmdir) targets.push(rmdir[1]);
  for (const target of targets) {
    if (isPinnedBy(cwd, target)) {
      violations.push(`[${scope}] ${line.trim()} — CWD is ${cwd}`);
    }
  }
}

test("no Rename/RMDir target is the installer's own working directory", () => {
  expect(violations).toEqual([]);
});

test("the payload swap runs with the CWD parked on $INSTDIR", () => {
  // The KOS-309 fix, stated positively: after staging into resources.next
  // the script must move its CWD back out before the renames.
  const macro = macros.get("StopProductProcesses");
  const stageAt = macro.indexOf('SetOutPath "$INSTDIR\\resources.next"');
  const backAt = macro.lastIndexOf('SetOutPath "$INSTDIR"');
  expect(stageAt).toBeGreaterThan(-1);
  expect(backAt).toBeGreaterThan(stageAt);
  expect(uninstallerCwd()).toBe(true);
});

function uninstallerCwd() {
  const uninstall = installer.split('Section "Uninstall"')[1] ?? "";
  return uninstall.includes('SetOutPath "$TEMP"');
}

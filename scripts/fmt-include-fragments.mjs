// Format .rs files reached only via include!(): rustfmt/cargo-fmt never opens
// them, so we wrap each fragment in a synthetic context matching its splice
// indentation, run `rustfmt`, and strip the wrapper again.
//   --check   verify only (exit 1 + list files needing changes)
//   (default) write formatted output back
import { readFileSync, writeFileSync, mkdtempSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join, posix } from "node:path";
import { execSync } from "node:child_process";

const MODE = process.argv.includes("--check") ? "check" : "write";

// collect include targets; vendor/ is third-party and out of scope
const all = execSync('git ls-files "*.rs"', { encoding: "utf8" })
  .split("\n")
  .filter((f) => f && !f.startsWith("vendor/") && !f.includes("/vendor/"));
const targets = new Set();
const unresolvable = [];
for (const f of all) {
  const src = readFileSync(f, "utf8");
  for (const m of src.matchAll(/include!\s*\(\s*(?:"([^"]+)"|([^)]+))\)/g)) {
    if (m[1] === undefined) {
      // non-literal include! (concat!/env!-style) — the gate cannot resolve
      // it, so it must fail loudly instead of silently skipping the file
      unresolvable.push(`${f}: include!(${m[2].trim()})`);
      continue;
    }
    targets.add(posix.normalize(posix.join(posix.dirname(f), m[1])));
  }
}
if (unresolvable.length) {
  console.error("include! calls this tool cannot resolve (extend it or rewrite the call):");
  for (const u of unresolvable) console.error(`  ${u}`);
  process.exit(1);
}

function rustfmt(text, tmp) {
  const file = join(tmp, "frag.rs");
  writeFileSync(file, text);
  const r = spawnSync(
    "rustfmt",
    ["--edition", "2021", "--config", "skip_children=true", "--emit", "stdout", file],
    { encoding: "utf8" },
  );
  if (r.status !== 0) return null;
  // output is "<path>:\n\n<formatted>" — drop the header line(s)
  const out = r.stdout;
  const idx = out.indexOf(".rs:\n");
  if (idx === -1) return null;
  return out.slice(idx + 5).replace(/^\n/, "");
}

const tmp = mkdtempSync(join(tmpdir(), "fmtinc-"));
const bad = [],
  unformattable = [],
  missing = [];
for (const t of [...targets].sort()) {
  let src;
  try {
    src = readFileSync(t, "utf8");
  } catch {
    missing.push(t);
    continue;
  }
  const first = src.split("\n").find((l) => l.trim());
  const base = first ? first.match(/^ */)[0].length : 0;
  const body = src.endsWith("\n") ? src : src + "\n";
  const stripWs = (ls) => {
    while (ls.length && !ls[ls.length - 1].trim()) ls.pop();
    return ls;
  };
  const tryStmt = () => {
    const w = rustfmt("fn __fmt_wrap() {\n" + body + "}\n", tmp);
    if (w === null) return null;
    const ls = stripWs(w.split("\n"));
    if (!ls[0].startsWith("fn __fmt_wrap()") || ls[ls.length - 1].trim() !== "}") return null;
    return ls.slice(1, -1).join("\n") + "\n";
  };
  const tryExpr = () => {
    const w = rustfmt("fn __fmt_wrap() {\n    let __x = " + body + ";\n}\n", tmp);
    if (w === null) return null;
    const ls = stripWs(w.split("\n"));
    if (!ls[0].startsWith("fn __fmt_wrap()") || ls[ls.length - 1].trim() !== "}") return null;
    const inner = ls.slice(1, -1);
    // unwrap `    let __x = <expr>;`
    if (!inner[0].startsWith("    let __x = ")) return null;
    inner[0] = "    " + inner[0].slice("    let __x = ".length);
    const last = inner[inner.length - 1];
    if (/^\s*};?\s*$/.test(last)) inner[inner.length - 1] = last.replace(/};?\s*$/, "}");
    return inner.join("\n") + "\n";
  };

  let out = null;
  if (base === 0) {
    out = rustfmt(body, tmp) ?? tryStmt() ?? tryExpr();
  } else {
    // nested context: wrap in `mod` for item fragments
    const mods = Math.max(1, Math.round(base / 4));
    let wrap = body;
    for (let i = 0; i < mods; i++) wrap = "mod __w {\n" + wrap + "\n}\n";
    const wout = rustfmt(wrap, tmp);
    if (wout !== null) {
      const ls = stripWs(wout.split("\n"));
      out = ls.slice(mods, ls.length - mods).join("\n") + "\n";
    } else {
      out = tryStmt() ?? tryExpr();
    }
  }
  if (out === null) {
    unformattable.push(t);
    continue;
  }
  if (out !== src) {
    bad.push(t);
    if (MODE === "write") writeFileSync(t, out);
  }
}
rmSync(tmp, { recursive: true, force: true });
console.log(
  `targets=${targets.size} changed=${bad.length} unformattable=${unformattable.length} missing=${missing.length}`,
);
unformattable.forEach((f) => console.log("UNFORMATTABLE", f));
missing.forEach((f) => console.error("MISSING TARGET", f));
if (missing.length) process.exit(1);
if (MODE === "check") {
  bad.forEach((f) => console.log("NEEDS FMT", f));
  process.exit(bad.length || unformattable.length ? 1 : 0);
}
process.exit(unformattable.length ? 1 : 0);

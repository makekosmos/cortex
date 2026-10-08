// package.json edits that only touch "scripts" select the checks exercising
// the edited entries. Everything else in a manifest (dependencies, engines,
// packageManager, ...) still requires the full check, as does
// any revision that cannot be read or parsed.

// Every script in both manifests is asserted Bun-free by
// scripts/package-manager-migration.test.mjs, and the manifest itself is
// oxfmt-formatted, so any manifest edit runs both.
const MANIFEST_CONTRACT = ["package-manager", "format"];

// Script name → checks that run it (or read it as a contract). An empty list
// marks a script that `pnpm run check` never runs, so the full check would not
// exercise it either. Scripts missing here (gate plumbing such as prepare and
// check:plan, the aggregators check, check:fast and test:static, lint/format
// entrypoints, and any newly added script) fail closed.
const SCRIPT_CHECKS = {
  "package.json": {
    "check:brand": ["brand"],
    "check:test-skips": ["test-skips"],
    "check:core-pin": ["core-pin"],
    rustfmt: ["rustfmt"],
    clippy: ["clippy"],
    "test:rust": ["test:rust"],
    "check:manager-gpui": ["manager-gpui"],
    "test:desktop-contracts": ["desktop-contracts"],
    "test:release-bom": ["release-bom"],
    "test:package-manager": ["package-manager"],
    "build:desktop": [],
    "build:installer:local": [],
    dev: [],
    "test:backend": [],
    "test:static": [],
    "test:desktop-tooling": [],
  },
  "desktop/package.json": {
    "test:runtime-staging": ["runtime-staging"],
    // desktop/scripts/release-pipeline.test.mjs asserts their step order.
    build: ["release-bom"],
    "package:dir": ["release-bom"],
    "build:backend": [],
    "build:backend:dev": [],
    "build:native:macos": ["release-bom"],
    "build:package-components": [],
    "verify:channel": [],
  },
};

export function isScriptsManifest(path) {
  return Object.hasOwn(SCRIPT_CHECKS, path);
}

const isRecord = (value) =>
  value !== null && value !== undefined && Object.getPrototypeOf(value) === Object.prototype;

function parse(source) {
  try {
    const value = JSON.parse(source);
    return isRecord(value) ? value : null;
  } catch {
    return null;
  }
}

function changedScripts(before, after) {
  const names = new Set([...Object.keys(before), ...Object.keys(after)]);
  return [...names].filter((name) => before[name] !== after[name]);
}

// Returns { checks } for a scripts-only edit with known scripts, otherwise
// { full: reason }. `before`/`after` are the raw manifest revisions.
export function manifestChecks(path, before, after) {
  const table = SCRIPT_CHECKS[path];
  if (!table) return { full: `${path} is not a scripts-mapped manifest` };
  const old = parse(before);
  const next = parse(after);
  if (!old || !next) return { full: `${path} could not be parsed` };
  const keys = new Set([...Object.keys(old), ...Object.keys(next)]);
  keys.delete("scripts");
  for (const key of keys)
    if (JSON.stringify(old[key]) !== JSON.stringify(next[key]))
      return { full: `${path} changes "${key}"` };
  const oldScripts = old.scripts ?? {};
  const nextScripts = next.scripts ?? {};
  if (!isRecord(oldScripts) || !isRecord(nextScripts))
    return { full: `${path} has a non-object "scripts"` };
  const checks = new Set(MANIFEST_CONTRACT);
  for (const name of changedScripts(oldScripts, nextScripts)) {
    if (!Object.hasOwn(table, name)) return { full: `${path} script "${name}" is unmapped` };
    for (const check of table[name]) checks.add(check);
  }
  return { checks: [...checks] };
}

// Documents that tests read as contracts, mapped to the checks that read
// them. check-plan.test.mjs scans test sources so a new reader cannot go
// unnoticed. Any other .md/.mdx/.txt document selects no checks.
export const DOC_CONTRACTS = {
  // package-manager-migration.test.mjs: command docs must not require Bun.
  "README.md": ["package-manager"],
};

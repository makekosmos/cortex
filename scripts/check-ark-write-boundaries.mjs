import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));

const scanRoots = [
  "apps/arrancador/electron",
  "apps/dashboard/electron",
  "apps/eden/ts/main",
];

const ignoredParts = new Set([
  "node_modules",
  "dist",
  "dist-electron",
  "out",
  "release",
  ".e2e",
  ".tmp",
]);

const arkTables = [
  "objects",
  "object_types",
  "object_links",
  "tracked_apps",
  "usage_sessions",
  "usage_events",
  "sync_kv",
  "sync_tombstones",
];

const writePattern = new RegExp(
  String.raw`\b(?:INSERT(?:\s+OR\s+\w+)?\s+INTO|UPDATE|DELETE\s+FROM)\s+(?:["'\`])?(?:${arkTables.join("|")})(?:["'\`])?\b`,
  "i",
);

function walk(dir, files = []) {
  if (!fs.existsSync(dir)) {
    return files;
  }

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (ignoredParts.has(entry.name)) {
      continue;
    }

    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(fullPath, files);
    } else if (/\.(?:ts|tsx|js|mjs|cjs)$/.test(entry.name)) {
      files.push(fullPath);
    }
  }

  return files;
}

const findings = [];

for (const root of scanRoots) {
  for (const filePath of walk(path.join(repoRoot, root))) {
    const text = fs.readFileSync(filePath, "utf8");
    const lines = text.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (writePattern.test(line)) {
        findings.push({
          file: path.relative(repoRoot, filePath).replaceAll(path.sep, "/"),
          line: index + 1,
          text: line.trim(),
        });
      }
    });
  }
}

if (findings.length > 0) {
  console.error("Direct app writes to ARK tables are not allowed:");
  for (const finding of findings) {
    console.error(`${finding.file}:${finding.line}: ${finding.text}`);
  }
  process.exit(1);
}

console.log("ARK write boundary guard passed.");

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));

// Multi-scan invariant guard. Запускается через `bun run ark:guard:writes`
// и в lefthook pre-commit. Каждая отдельная проверка возвращает массив
// findings; общий exit code = 1 если есть хотя бы одна.
//
// Что ловим:
//   1. SQL INSERT/UPDATE/DELETE для ARK таблиц в app TS service code
//      (forbidden.md → ARK writes).
//   2. `app.getPath('userData')` вне shell/electron/instance.ts —
//      нарушает single-source-of-truth для slot-based изоляции
//      (forbidden.md → Instance slots).
//   3. `path.join(..., "Kosmos" | "Kepler", ...)` в shell main process кроме
//      instance.ts / data-dir.ts (тот же запрет, разный синтаксис).
//   4. `KOSMOS_DATA_DIR=...APPDATA...` в tests/e2e/* вне helpers/launch.ts —
//      форсирует тесты в real user data dir (forbidden.md → Тесты).

const ignoredParts = new Set([
  "node_modules",
  "dist",
  "dist-electron",
  "out",
  "release",
  ".e2e",
  ".tmp",
]);

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

function relRepo(p) {
  return path.relative(repoRoot, p).replaceAll(path.sep, "/");
}

// --- Check 1: direct SQL writes ---------------------------------------------

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

function checkDirectSqlWrites() {
  const roots = [
    "extensions/arrancador/src",
    "extensions/dashboard/src",
    "extensions/delphi/src",
    "extensions/eden/src",
    "extensions/horologion/src",
    "shell/electron",
  ];
  const findings = [];
  for (const root of roots) {
    for (const filePath of walk(path.join(repoRoot, root))) {
      const text = fs.readFileSync(filePath, "utf8");
      text.split(/\r?\n/).forEach((line, index) => {
        if (writePattern.test(line)) {
          findings.push({
            file: relRepo(filePath),
            line: index + 1,
            text: line.trim(),
            kind: "sql-write",
          });
        }
      });
    }
  }
  return findings;
}

// --- Check 2: app.getPath('userData') outside instance.ts -------------------

const userDataPattern = /\bapp\.getPath\s*\(\s*["']userData["']\s*\)/;

function checkUserDataAccess() {
  const root = path.join(repoRoot, "shell/electron");
  const allowlist = new Set([
    "shell/electron/instance.ts",
  ]);
  const findings = [];
  for (const filePath of walk(root)) {
    const rel = relRepo(filePath);
    if (allowlist.has(rel)) continue;
    const text = fs.readFileSync(filePath, "utf8");
    text.split(/\r?\n/).forEach((line, index) => {
      // Игнор строк-комментариев (// ...) — это упоминание паттерна, не использование.
      if (/^\s*(?:\/\/|\*)/.test(line)) return;
      if (userDataPattern.test(line)) {
        findings.push({
          file: rel,
          line: index + 1,
          text: line.trim(),
          kind: "user-data-access",
        });
      }
    });
  }
  return findings;
}

// --- Check 3: path.join Kosmos/Kepler outside whitelist ---------------------

const pathJoinBrandPattern = /\bpath\.join\s*\([^)]*["'](?:Kosmos|Kepler)["']/;

function checkBrandPathJoin() {
  const root = path.join(repoRoot, "shell/electron");
  const allowlist = new Set([
    "shell/electron/instance.ts",
    "shell/electron/data-dir.ts",
  ]);
  const findings = [];
  for (const filePath of walk(root)) {
    const rel = relRepo(filePath);
    if (allowlist.has(rel)) continue;
    const text = fs.readFileSync(filePath, "utf8");
    text.split(/\r?\n/).forEach((line, index) => {
      if (/^\s*(?:\/\/|\*)/.test(line)) return;
      if (pathJoinBrandPattern.test(line)) {
        findings.push({
          file: rel,
          line: index + 1,
          text: line.trim(),
          kind: "brand-path-join",
        });
      }
    });
  }
  return findings;
}

// --- Check 4: KOSMOS_DATA_DIR pointing to APPDATA in tests ------------------

// Ловит: KOSMOS_DATA_DIR: ... process.env.APPDATA / %APPDATA% / "AppData" вместе
// в пределах ~5 строк. Простой grep — оба токена на одной строке.
const kosmosDataAppdataPattern = /KOSMOS_DATA_DIR[\s\S]{0,200}(?:%APPDATA%|process\.env\.APPDATA|AppData[\\/]Roaming|getPath\(["']appData["']\))/;

function checkTestsAppdataDataDir() {
  const root = path.join(repoRoot, "tests/e2e");
  const allowlist = new Set([
    "tests/e2e/helpers/launch.ts",
  ]);
  const findings = [];
  for (const filePath of walk(root)) {
    const rel = relRepo(filePath);
    if (allowlist.has(rel)) continue;
    const text = fs.readFileSync(filePath, "utf8");
    // multi-line match: kosmosDataAppdataPattern может пересекать строки.
    if (kosmosDataAppdataPattern.test(text)) {
      const match = text.match(kosmosDataAppdataPattern);
      const before = text.slice(0, match.index);
      const lineNum = before.split(/\r?\n/).length;
      findings.push({
        file: rel,
        line: lineNum,
        text: match[0].split(/\r?\n/)[0].trim(),
        kind: "tests-appdata-data-dir",
      });
    }
  }
  return findings;
}

// --- Aggregate --------------------------------------------------------------

const all = [
  ...checkDirectSqlWrites(),
  ...checkUserDataAccess(),
  ...checkBrandPathJoin(),
  ...checkTestsAppdataDataDir(),
];

const KIND_LABELS = {
  "sql-write": "Direct SQL write to ARK table",
  "user-data-access":
    "app.getPath('userData') outside instance.ts — use resolveInstance() / keplerDataDir()",
  "brand-path-join":
    "path.join(..., 'Kosmos'|'Kepler', ...) outside instance.ts / data-dir.ts",
  "tests-appdata-data-dir":
    "KOSMOS_DATA_DIR pointing at real user APPDATA in tests/e2e — forbidden.md → Тесты",
};

if (all.length > 0) {
  const byKind = new Map();
  for (const f of all) {
    if (!byKind.has(f.kind)) byKind.set(f.kind, []);
    byKind.get(f.kind).push(f);
  }
  for (const [kind, list] of byKind) {
    console.error(`\n=== ${KIND_LABELS[kind] ?? kind} ===`);
    for (const f of list) {
      console.error(`${f.file}:${f.line}: ${f.text}`);
    }
  }
  console.error(`\nARK write boundary guard found ${all.length} violation(s).`);
  process.exit(1);
}

console.log("ARK write boundary guard passed.");

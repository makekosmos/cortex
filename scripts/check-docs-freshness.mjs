#!/usr/bin/env node
/*
 * check-docs-freshness.mjs
 *
 * Сверяет содержимое docs-site/ с реальностью репозитория:
 *
 *  1. Упомянутые пути файлов/папок (apps/<x>/..., packages/<x>/..., scripts/<x>.<ext>,
 *     docs/<x>.md, services/<x>/..., .agent/...) — реально существуют?
 *  2. Команды `bun run <name>` — реально объявлены в каком-то package.json?
 *  3. Внутренние ссылки `/section/page` — реально соответствуют файлу
 *     `docs-site/<section>/<page>.md` (или `docs-site/<section>/index.md`)?
 *
 * Падает с exit code 1 если есть stale references.
 * Запускай: bun run docs:check
 */

import { readFile, readdir, stat } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const DOCS = path.join(ROOT, "docs-site");

// ─────────────────────────────────────────────────────────────────────────────
// сбор всех .md в docs-site
// ─────────────────────────────────────────────────────────────────────────────

async function walk(dir, files = []) {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const e of entries) {
    const full = path.join(dir, e.name);
    if (e.isDirectory()) {
      if (e.name === "node_modules" || e.name === ".vitepress" || e.name === "public") continue;
      await walk(full, files);
    } else if (e.name.endsWith(".md")) {
      files.push(full);
    }
  }
  return files;
}

// ─────────────────────────────────────────────────────────────────────────────
// сбор всех `bun run <name>` команд из package.json'ов
// ─────────────────────────────────────────────────────────────────────────────

async function collectScripts() {
  const scripts = new Set();
  async function walkPkg(dir) {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const e of entries) {
      if (e.name === "node_modules" || e.name.startsWith(".")) continue;
      const full = path.join(dir, e.name);
      if (e.isDirectory()) {
        await walkPkg(full);
      } else if (e.name === "package.json") {
        try {
          const pkg = JSON.parse(await readFile(full, "utf8"));
          for (const s of Object.keys(pkg.scripts || {})) scripts.add(s);
        } catch (_) {
          /* ignore */
        }
      }
    }
  }
  await walkPkg(ROOT);
  return scripts;
}

// ─────────────────────────────────────────────────────────────────────────────
// extractors
// ─────────────────────────────────────────────────────────────────────────────

// inline-code `apps/foo/bar.ts` или `apps/foo/`
// Только пути от корня репо. Относительные внутри приложений (main/, electron/, src/)
// сюда не попадают — их не проверяем, потому что они контекст-зависимые.
const PATH_RE = /`((?:apps|packages|services|scripts|docs|\.agent|docs-site)\/[A-Za-z0-9._\-\/]+)`/g;

// bun run <name> или bun run --cwd <path> <name>
// "<name>" не должен содержать `<` (template-плейсхолдер) или `--` (флаг)
const BUN_CMD_RE = /\bbun run (?:--cwd \S+ )?([a-z][a-z0-9:-]*)\b/g;

// Игнор runtime-артефактов (создаются командами, не должны лежать в репо)
const IGNORE_PATH_PARTS = [
  ".e2e/",
  ".tmp/",
  "/dist/",
  "/build/",
  "/target/",
  "/release/",
  "/coverage/",
  "node_modules/",
];

// Пути, упомянутые в доке намеренно как удалённые / будущие
const KNOWN_NONEXISTENT = new Set([
  "apps/delphi/ts/sidecar", // намеренно удалён, упомянут в DELPHI-LEGACY-DB-DECISION
  "apps/eden/kotlin",       // планируется
]);

// внутренние markdown-ссылки `/section/page` (с возможным якорем)
const INTERNAL_LINK_RE = /\]\((\/[a-z0-9\-/]+)(?:#[a-z0-9-]+)?\)/gi;

// ─────────────────────────────────────────────────────────────────────────────
// проверки
// ─────────────────────────────────────────────────────────────────────────────

function pathExists(p) {
  // Пути могут быть с / или \, приводим к OS
  const norm = p.replace(/\//g, path.sep);
  return existsSync(path.join(ROOT, norm));
}

async function checkInternalLink(link) {
  // /agents/ → docs-site/agents/index.md
  // /apps/eden → docs-site/apps/eden.md (или index.md)
  const cleaned = link.replace(/\/$/, "");
  const asFile = path.join(DOCS, cleaned + ".md");
  const asIndex = path.join(DOCS, cleaned, "index.md");
  if (existsSync(asFile) || existsSync(asIndex) || existsSync(path.join(DOCS, cleaned))) {
    return true;
  }
  // спец-случай: /llms.txt
  if (cleaned === "/llms.txt" && existsSync(path.join(DOCS, "public", "llms.txt"))) {
    return true;
  }
  return false;
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

async function main() {
  console.log("→ docs:check");

  const mdFiles = await walk(DOCS);
  const scripts = await collectScripts();

  const issues = [];

  for (const file of mdFiles) {
    const rel = path.relative(ROOT, file).replace(/\\/g, "/");
    const text = await readFile(file, "utf8");

    // 1. пути в backticks
    for (const m of text.matchAll(PATH_RE)) {
      const p = m[1];
      // нормализуем — убираем хвостовые `/`
      const clean = p.replace(/\/+$/, "");
      // Skip явные template-плейсхолдеры с <...> и UPPER_CASE токенами
      if (clean.includes("<") || clean.includes(">")) continue;
      if (/\$\{/.test(clean)) continue;
      if (/\b[A-Z_]{2,}\b/.test(clean)) continue;
      // skip dynamic placeholders
      if (clean.includes("/name/") || clean.includes("/app/")) continue;
      // skip runtime артефакты
      if (IGNORE_PATH_PARTS.some((part) => ("/" + clean).includes(part))) continue;
      // skip намеренно удалённые / будущие
      if (KNOWN_NONEXISTENT.has(clean)) continue;

      if (!pathExists(clean)) {
        issues.push({
          file: rel,
          kind: "path",
          ref: clean,
          msg: `путь не существует: ${clean}`,
        });
      }
    }

    // 2. bun run <script>
    for (const m of text.matchAll(BUN_CMD_RE)) {
      const cmd = m[1];
      if (!scripts.has(cmd)) {
        issues.push({
          file: rel,
          kind: "cmd",
          ref: `bun run ${cmd}`,
          msg: `скрипт не найден ни в одном package.json: bun run ${cmd}`,
        });
      }
    }

    // 3. внутренние ссылки
    for (const m of text.matchAll(INTERNAL_LINK_RE)) {
      const link = m[1];
      // skip llms.txt (он public)
      if (link === "/llms.txt") continue;
      // skip external-like
      if (link.startsWith("//")) continue;
      const ok = await checkInternalLink(link);
      if (!ok) {
        issues.push({
          file: rel,
          kind: "link",
          ref: link,
          msg: `битая внутренняя ссылка: ${link}`,
        });
      }
    }
  }

  if (issues.length === 0) {
    console.log("  ✓ всё свежо, stale references не найдено");
    return;
  }

  // группируем по файлам
  const byFile = new Map();
  for (const it of issues) {
    if (!byFile.has(it.file)) byFile.set(it.file, []);
    byFile.get(it.file).push(it);
  }

  console.error(`\n  ✗ найдено ${issues.length} stale references:\n`);
  for (const [file, list] of byFile) {
    console.error(`  ${file}`);
    for (const it of list) {
      console.error(`    [${it.kind}] ${it.msg}`);
    }
    console.error("");
  }

  process.exit(1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

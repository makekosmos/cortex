import path from "node:path";
import { readFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import {
  walk,
  collectScripts,
  checkInternalLink,
  checkRepoPathReference,
  PATH_RE,
  PATH_LINK_RE,
  BUN_CMD_RE,
  INTERNAL_LINK_RE,
  ROOT,
  DOCS,
  ROOT_DOCS,
  KNOWN_RETIRED_COMMANDS,
} from "./check-docs-freshness-lib.mjs";

async function main() {
  console.log("→ docs:check");

  const mdFiles = [
    ...(await walk(DOCS)),
    ...ROOT_DOCS.map((rel) => path.join(ROOT, rel)).filter((file) => existsSync(file)),
  ];
  const scripts = await collectScripts();

  const issues = [];

  for (const file of mdFiles) {
    const rel = path.relative(ROOT, file).replace(/\\/g, "/");
    const text = await readFile(file, "utf8");

    // 1. пути в backticks
    for (const m of text.matchAll(PATH_RE)) {
      checkRepoPathReference({ issues, rel, p: m[1] });
    }

    // 1b. пути в markdown-ссылках
    for (const m of text.matchAll(PATH_LINK_RE)) {
      checkRepoPathReference({ issues, rel, p: m[1] });
    }

    // 2. bun run <script>
    for (const m of text.matchAll(BUN_CMD_RE)) {
      const cmd = m[1];
      if (!scripts.has(cmd) && !KNOWN_RETIRED_COMMANDS.has(cmd)) {
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

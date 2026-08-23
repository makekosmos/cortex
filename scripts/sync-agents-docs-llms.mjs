import { readDoc, stripFrontmatter, rewriteLinks, write } from "./sync-agents-docs-lib.mjs";

const LLMS_SECTIONS = [
  { title: "О документе для агента", file: "agents/index.md" },
  { title: "Архитектура", file: "concepts/architecture.md" },
  { title: "Модель данных ARK", file: "concepts/ark-objects.md" },
  { title: "Синхронизация", file: "concepts/sync.md" },
  { title: "Граница записи в ARK", file: "concepts/write-boundary.md" },
  { title: "Read-only SQL", file: "concepts/readonly-sql.md" },
  { title: "Proof loop", file: "concepts/proof-loop.md" },
  { title: "Изоляция тестовых БД", file: "concepts/test-isolation.md" },
  { title: "Запреты и гварды", file: "agents/forbidden.md" },
  { title: "Чек-листы по областям", file: "agents/checklists.md" },
  { title: "Шаблоны спецификаций", file: "agents/spec-templates.md" },
  { title: "Поддержка документации", file: "agents/docs-maintenance.md" },
  { title: "Сжатый чек-лист правил", file: "reference/rules.md" },
  { title: "Smoke-матрица", file: "reference/smoke-matrix.md" },
  { title: "Глоссарий", file: "reference/glossary.md" },
  { title: "Команды и скрипты", file: "reference/commands.md" },
  { title: "Журнал решений (ADR)", file: "reference/decisions.md" },
];

async function buildLlmsTxtManifest() {
  // Manifest по спеке https://llmstxt.org/ — title, optional summary, разделы
  // со ссылками. Относительные URL (`/concepts/X`) — агенты resolve'ят от
  // origin'а сайта.
  const lines = [
    "# Kosmos",
    "",
    "> Local-first монорепо для личной экосистемы: ARK runtime (Rust + SQLite + FTS5) + Electron-приложения (Kepler launcher + Vue extensions: Eden, Delphi, Arrancador, Akasha; Focus Session встроен в shell) + Android sync.",
    "",
    "Эти ссылки — ключевой контекст для агентов, работающих над репо. Читай по порядку для полной картины архитектуры и правил. Если нужен весь контекст одним fetch'ом — [/full-llms.txt](/full-llms.txt).",
    "",
    "## Концепты",
    "",
  ];
  for (const s of LLMS_SECTIONS) {
    if (!s.file.startsWith("concepts/")) continue;
    const slug = s.file.replace(/\.md$/, "");
    lines.push(`- [${s.title}](/${slug})`);
  }
  lines.push("", "## Для агентов", "");
  for (const s of LLMS_SECTIONS) {
    if (!s.file.startsWith("agents/")) continue;
    const slug = s.file.replace(/\.md$/, "");
    lines.push(`- [${s.title}](/${slug})`);
  }
  lines.push("", "## Справочник", "");
  for (const s of LLMS_SECTIONS) {
    if (!s.file.startsWith("reference/")) continue;
    const slug = s.file.replace(/\.md$/, "");
    lines.push(`- [${s.title}](/${slug})`);
  }
  lines.push(
    "",
    "## Optional",
    "",
    "- [Roadmap Kepler](/apps/kepler-roadmap)",
    "- [Что нового](/whats-new/) — release notes по-человечески",
    "- [/full-llms.txt](/full-llms.txt) — все материалы выше inline в одном файле",
    "",
  );

  await write("docs-site/public/llms.txt", lines.join("\n"));
}

async function buildLlmsTxtFull() {
  const contents = ["## Contents", ""];
  LLMS_SECTIONS.forEach((s, i) => {
    contents.push(`${i}. ${s.title} — section ## (источник: docs-site/${s.file})`);
  });
  contents.push("");

  const head = [
    "# Kosmos — full-llms.txt",
    "",
    "Полный inline-текст правил и контекста репозитория Kosmos.",
    "Источник правды — docs-site/. Этот файл генерируется скриптом scripts/sync-agents-docs.mjs.",
    "",
    "Структура: каждый раздел — отдельный концепт или область. Читай по порядку для полной картины.",
    "Если нужен только manifest со ссылками — `/llms.txt`.",
    "",
    "---",
    "",
    contents.join("\n"),
    "---",
    "",
  ].join("\n");

  let body = head;
  for (const s of LLMS_SECTIONS) {
    const md = await readDoc(s.file);
    if (!md) continue;
    body += `\n## ${s.title}\n\n`;
    body += rewriteLinks(stripFrontmatter(md)).trim();
    body += "\n\n";
  }

  await write("docs-site/public/full-llms.txt", body);
}

async function buildLlmsTxt() {
  await buildLlmsTxtManifest();
  await buildLlmsTxtFull();
}

// ─────────────────────────────────────────────────────────────────────────────
// main
// ─────────────────────────────────────────────────────────────────────────────

// Прогоняем oxfmt по только что сгенерированным .md, иначе pre-commit hook
// (`oxfmt --check`) падает: генератор пишет «как есть», а CI/hook ожидают
export { buildLlmsTxt };

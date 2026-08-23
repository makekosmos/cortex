import { spawnSync } from "node:child_process";
import {
  ROOT,
  MARK,
  readDoc,
  stripFrontmatter,
  rewriteLinks,
  write,
  header,
} from "./sync-agents-docs-lib.mjs";
import { buildLlmsTxt } from "./sync-agents-docs-llms.mjs";

async function buildRootAgents() {
  const agentCore = await readDoc("agents/claude-md-core.md");

  const agentsBody =
    header("AGENTS.md — Kosmos", [
      "",
      "Этот файл намеренно компактный: он грузится в агентский контекст по умолчанию.",
      "Детальные правила живут в `docs-site/` и подгружаются по ссылкам только когда задача касается области.",
      "Полный inline-reference для редких случаев — `docs-site/public/full-llms.txt`.",
    ]) +
    "\n" +
    rewriteLinks(stripFrontmatter(agentCore)).trim() +
    "\n";

  await write("AGENTS.md", agentsBody);

  // CLAUDE.md — slim version. Anthropic best-practices: < 200 строк, иначе
  // Claude начинает игнорировать правила (https://code.claude.com/docs/en/best-practices).
  // CLAUDE.md загружается каждую сессию каждый turn. Source — единственный
  // curated файл `agents/claude-md-core.md`; per-app forbidden и situational
  // знания живут в `docs-site/` и подгружаются Claude'ом по необходимости.
  // Full detail stays in docs-site/ and full-llms.txt; the generated root files
  // are boot context, not an inline encyclopedia.
  const claudeCore = await readDoc("agents/claude-md-core.md");

  const claudeHeader = [
    "# CLAUDE.md",
    "",
    MARK,
    "",
    "Контекст работы над монорепо Kosmos для Claude Code.",
    "Источник — `docs-site/agents/claude-md-core.md`. Регенерация — `bun run docs:sync`.",
    "",
    "Компактный cross-agent контекст — `AGENTS.md` в корне. Глубокие правила — страницы под `docs-site/`.",
    "",
    "---",
    "",
  ].join("\n");

  await write(
    "CLAUDE.md",
    claudeHeader + "\n" + rewriteLinks(stripFrontmatter(claudeCore)).trim() + "\n",
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// per-app / per-package / per-service AGENTS.md
// ─────────────────────────────────────────────────────────────────────────────

const TARGETS = [
  {
    dest: "incubator/mobile/delphi/AGENTS.md",
    title: "Delphi Android",
    sourceDocs: [
      "docs-site/apps/index.md#android-kotlin",
      "docs-site/apps/ark-service.md",
      "docs-site/apps/delphi.md",
    ],
    sections: [
      {
        title: "Scope",
        lines: [
          "This file applies only to `incubator/mobile/delphi/` Android code.",
          "Desktop Delphi lives in `products/delphi/`; do not apply desktop ARK or Vue assumptions here unless the task explicitly says so.",
          "Android Delphi is package `com.kazui.delphi` and consumes data from the separate `incubator/mobile/ark-service` APK.",
        ],
      },
      {
        title: "Must Read",
        lines: [
          "`docs-site/apps/index.md#android-kotlin` — current Android stack snapshot.",
          "`docs-site/apps/ark-service.md` — ContentProvider / Room ownership model.",
          "`docs-site/agents/checklists.md` — Android checklist before handoff.",
        ],
      },
      {
        title: "Invariants",
        lines: [
          "Android Delphi does not own the Room DB directly; data access goes through `com.kosmos.ark.data` ContentProvider from `incubator/mobile/ark-service`.",
          "The Android stack is currently isolated from desktop ARK: no desktop `ark.db`, no `ark-core-rpc`, no working desktop<->Android sync assumption.",
          "If `ark-service` is absent, Delphi must keep the install-required UX instead of crashing or silently writing elsewhere.",
          "Changes to Android schema/provider contracts must be coordinated with `incubator/mobile/ark-service/`.",
        ],
      },
      {
        title: "Commands",
        lines: [
          "`cd incubator/mobile/delphi; .\\gradlew build` — Android Delphi build.",
          "`cd incubator/mobile/ark-service; .\\gradlew build` — provider build when provider contract changes.",
        ],
      },
    ],
  },
  {
    dest: "core/ark/crates/ark-core/AGENTS.md",
    title: "ark-core",
    sourceDocs: [
      "docs-site/packages/ark-core.md",
      "docs-site/concepts/ark-objects.md",
      "docs-site/concepts/sync.md",
      "docs-site/concepts/write-boundary.md",
      "docs-site/concepts/db-resilience.md",
    ],
    sections: [
      {
        title: "Scope",
        lines: [
          "`core/ark/crates/ark-core/` is the shared Rust + SQLite runtime and `ark-core-rpc` sidecar.",
          "Electron callers use newline-delimited JSON-RPC; Android/Swift integration goes through UniFFI surfaces.",
          "Full RPC/entity reference lives in `docs-site/packages/ark-core.md`; do not inline it here.",
        ],
      },
      {
        title: "Must Read",
        lines: [
          "`docs-site/packages/ark-core.md` — package contract and verification expectations.",
          "`docs-site/concepts/sync.md` — sync protocol, HLC, peer rules.",
          "`docs-site/concepts/write-boundary.md` — allowed write paths.",
          "`docs-site/concepts/db-resilience.md` — Mutex poison recovery, backups, integrity checks.",
        ],
      },
      {
        title: "Invariants",
        lines: [
          "Production Rust must not use `Mutex::lock().unwrap()`; recover poison with `unwrap_or_else(|e| e.into_inner())`.",
          "Schema evolution is additive only: no destructive migrations; prefer idempotent `CREATE TABLE IF NOT EXISTS` / additive indexes.",
          "Every direct writer to syncable data must update sync state through the appropriate `record_local_*` / version-vector path.",
          "Sync wire messages stay `snake_case`; do not weaken self-peer or routable-address filtering.",
          "`ark-core-rpc` stdout is protocol output: keep framing newline-delimited JSON and avoid noisy logs there.",
        ],
      },
      {
        title: "Commands",
        lines: [
          "`cargo test --manifest-path core/ark/crates/ark-core/rust/Cargo.toml` — core tests.",
          "`cargo build --manifest-path core/ark/crates/ark-core/rust/Cargo.toml --bin ark-core-rpc` — sidecar build.",
          "`bun run ark:guard:writes` — after data-layer/write-boundary changes.",
          "`bun run ark:smoke` — after substantial runtime changes.",
        ],
      },
    ],
  },
];

// TL;DR версия `reference/rules.md` — 5-7 строк по ключевым правилам.
// Полный текст всегда доступен в `docs-site/reference/rules.md`.
const RULES_TLDR = [
  "## Сжатые правила репозитория (TL;DR)",
  "",
  "- **ARK writes** — только через `@kosmos/ark` (TS) или `ark_core::db` (Rust). Прямые SQL writes в `objects` / `object_types` / `object_links` / `tracked_apps` / `usage_sessions` / `usage_events` / `sync_kv` запрещены.",
  "- **Read-only SQL** — renderer никогда не открывает SQLite; read-only fallback в Electron main отделён от write paths и не ходит в user DB из тестов.",
  "- **Тестовая изоляция** — только `.tmp`, `.e2e`, `.agent/tasks/<TASK_ID>/smoke/` или OS temp. User data dir в автотестах — отказ на ревью.",
  "- **Proof loop** — substantial-правки идут через `.agent/tasks/<DATE>-<slug>/`: spec → реализация → evidence → (problems → fix → reverify). Каждый AC = `PASS`.",
  "- **Sync state** — direct writers в синхронизируемые таблицы обязаны вызывать `ark_core::db::bump_sync_version_vector`.",
  "- **Tooling** — `bun run ark:guard:writes` перед PR в data-слой; `bun run ark:smoke` перед нетривиальным PR.",
  "",
  "Полный текст: `docs-site/reference/rules.md`.",
  "",
].join("\n");

async function buildPerAreaAgents() {
  for (const t of TARGETS) {
    const body =
      header(`AGENTS.md — ${t.title}`, [
        "",
        "Компактный локальный boot context. Подробности читай в source docs ниже по необходимости.",
      ]) +
      renderAreaContext(t) +
      "\n" +
      RULES_TLDR;

    await write(t.dest, body);
  }
}

function renderAreaContext(target) {
  const lines = ["## Source Docs", "", ...target.sourceDocs.map((doc) => `- \`${doc}\``), ""];

  for (const section of target.sections) {
    lines.push(`## ${section.title}`, "");
    for (const line of section.lines) {
      lines.push(`- ${line}`);
    }
    lines.push("");
  }

  return lines.join("\n");
}

// ─────────────────────────────────────────────────────────────────────────────
// llms.txt + full-llms.txt
//
// Следуем спеку llmstxt.org:
//   /llms.txt       — manifest, только ссылки на ключевые страницы. Лёгкий,
//                     агент fetch'ит этот файл сначала, дальше fetch'ит
//                     отдельные страницы по необходимости.
//   /full-llms.txt  — полный inline-текст всех материалов. Тяжёлый, для случаев
//                     когда агенту нужен весь контекст одним fetch'ом
//                     (например, прогревом).
// ─────────────────────────────────────────────────────────────────────────────

function formatGenerated() {
  const files = [
    "AGENTS.md",
    "CLAUDE.md",
    "incubator/mobile/delphi/AGENTS.md",
    "core/ark/crates/ark-core/AGENTS.md",
  ];
  const r = spawnSync("bunx", ["oxfmt", ...files], {
    stdio: "inherit",
    cwd: ROOT,
  });
  if (r.status !== 0) {
    console.error(`oxfmt failed (exit ${r.status})`);
    process.exit(r.status ?? 1);
  }
}

async function main() {
  console.log("→ sync agents docs");
  await buildRootAgents();
  await buildPerAreaAgents();
  await buildLlmsTxt();
  formatGenerated();
  console.log("done.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});

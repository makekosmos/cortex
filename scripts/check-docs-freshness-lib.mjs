import { readFile, readdir } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const DOCS = path.join(ROOT, "docs-site");
const ROOT_DOCS = ["README.md", "TODO.md", "STATUS.md"];

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
        } catch {
          /* ignore */
        }
      }
    }
  }
  const scriptRoots = [
    ROOT,
    ...new Set(
      SIBLING_REPO_PATHS.map(([, sibling]) =>
        path.resolve(ROOT, sibling.split("/").join(path.sep)),
      ),
    ),
  ];
  for (const scriptRoot of scriptRoots) {
    if (existsSync(scriptRoot)) await walkPkg(scriptRoot);
  }
  return scripts;
}

// ─────────────────────────────────────────────────────────────────────────────
// extractors
// ─────────────────────────────────────────────────────────────────────────────

// inline-code `apps/foo/bar.ts` или `apps/foo/`
// Только пути от корня репо. Относительные внутри приложений (main/, electron/, src/)
// сюда не попадают — их не проверяем, потому что они контекст-зависимые.
const PATH_RE =
  /`((?:apps|packages|services|scripts|docs|\.agent|docs-site|crates|shell|extensions|mobile|legacy|core|platform|products|incubator)\/[A-Za-z0-9._\-/]+)`/g;

// markdown links to repo-local paths: [label](./core/ark/packages/ark/README.md)
const PATH_LINK_RE =
  /\]\((?:\.\/)?((?:apps|packages|services|scripts|docs|\.agent|docs-site|crates|shell|extensions|mobile|legacy|core|platform|products|incubator)\/[A-Za-z0-9._\-/]+)(?:#[A-Za-z0-9._\-/]+)?\)/g;

// bun run <name> или bun run --cwd <path> <name>
// "<name>" не должен содержать `<` (template-плейсхолдер) или `--` (флаг)
const BUN_CMD_RE = /\bbun run (?:--cwd \S+ )?([a-z][a-z0-9:-]*)\b/g;

// Игнор runtime-артефактов (создаются командами, не должны лежать в репо)
const IGNORE_PATH_PARTS = [
  ".e2e/",
  ".tmp/",
  "/dist/",
  "/dist-electron/",
  "/build/",
  "/target/",
  "/release/",
  "/coverage/",
  "node_modules/",
];

// Пути, упомянутые в доке намеренно как удалённые / будущие
const KNOWN_NONEXISTENT = new Set([
  "apps/digital-cave", // TBD — имя зарезервировано, см. docs-site/apps/digital-cave.md
  "apps/kerux", // TBD — имя зарезервировано, см. docs-site/apps/kerux.md
  "apps/kosmos-shell", // намеренный «не возвращаемся» anti-pattern (был удалён Phase B1)
  "services/kosmos-backend", // намеренный anti-pattern (был удалён Phase B1)
  "apps/eden", // удалён в Phase 6.0.A (Eden теперь extension)
  "apps/eden/ts",
  "apps/eden/kotlin",
  "platform/runtime/src/backup.rs", // Phase 11 backup module — TBD
  "platform/desktop/src/components/ClipboardQuickPanel.vue", // retired clipboard UI
  "platform/desktop/electron/clipboard-history-store.ts", // retired clipboard history
  "platform/desktop/electron/clipboard-history.ts", // retired clipboard history
]);

// Commands from the retired monorepo shell/extension pipeline. Keep this list
// explicit: new commands still must exist in an active sibling package.json.
const KNOWN_RETIRED_COMMANDS = new Set([
  "build:extensions",
  "build:extensions:only",
  "build:extensions:changed",
  "build:extensions:vue",
  "build:extension",
  "dev:extensions",
  "dev:extensions:only",
  "ext:install",
  "ext:uninstall",
  "ext:build",
  "ext:publish",
  "ext:publish-all",
  "ext:catalog",
  "desktop:typecheck",
  "shell:build",
  "shell:typecheck",
  "products:build",
  "visual:regression",
  "visual:launcher",
  "visual:eden",
  "test:e2e:debug",
]);

// внутренние markdown-ссылки `/section/page` (с возможным якорем)
const SIBLING_REPO_PATHS = [
  ["core/ark/packages/ark", "../arca-sdk"],
  ["packages/visuals", "../imago"],
  ["products/eden", "../memoria"],
  ["products/delphi", "../agenda"],
  ["products/daedalus", "../incubator/daedalus"],
  ["incubator/arrancador", "../incubator/arrancador"],
  ["incubator/akasha", "../incubator/akasha"],
  ["incubator/mobile", "../incubator/mobile"],
  ["platform/desktop", "../cortex/desktop"],
  ["platform/runtime", "../cortex/runtime"],
  ["platform/native-services", "../cortex/native-services"],
  ["services/relay-reference", "../relay-reference"],
];

const INTERNAL_LINK_RE = /\]\((\/[a-z0-9\-/]+)(?:#[a-z0-9-]+)?\)/gi;

// ─────────────────────────────────────────────────────────────────────────────
// проверки
// ─────────────────────────────────────────────────────────────────────────────

function pathExists(p) {
  // Пути могут быть с / или \, приводим к OS
  const norm = p.replaceAll("\\", "/").replace(/\/+$/, "");
  const sibling = SIBLING_REPO_PATHS.find(
    ([legacy]) => norm === legacy || norm.startsWith(`${legacy}/`),
  );
  if (sibling) {
    const suffix = norm.slice(sibling[0].length).replace(/^[/\\]/, "");
    return existsSync(path.join(ROOT, sibling[1], suffix));
  }
  return existsSync(path.join(ROOT, norm));
}

function shouldSkipPath(clean) {
  // Skip явные template-плейсхолдеры с <...> и UPPER_CASE токенами
  if (clean.includes("<") || clean.includes(">")) return true;
  if (/\$\{/.test(clean)) return true;
  if (/\b[A-Z_]{2,}\b/.test(clean)) return true;
  // skip dynamic placeholders
  if (clean.includes("/name/") || clean.includes("/app/")) return true;
  // skip runtime артефакты
  if (IGNORE_PATH_PARTS.some((part) => ("/" + clean).includes(part))) return true;
  // skip намеренно удалённые / будущие
  if (KNOWN_NONEXISTENT.has(clean)) return true;
  return false;
}

function checkRepoPathReference({ issues, rel, p }) {
  // нормализуем — убираем хвостовые `/`
  const clean = p.replace(/\/+$/, "");
  if (shouldSkipPath(clean)) return;

  if (!pathExists(clean)) {
    issues.push({
      file: rel,
      kind: "path",
      ref: clean,
      msg: `путь не существует: ${clean}`,
    });
  }
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
export {
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
};

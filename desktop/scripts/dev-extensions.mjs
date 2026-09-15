// Orchestrator: запускает Vite dev server для каждого Vue extension'а
// с его уникальным `devPort` из manifest.json. HMR live-reload для extension
// разработки в стиле Raycast.
//
// Запуск:
//   pnpm run dev:extensions
//   # или
//   node scripts/dev-extensions.mjs
//   # optional:
//   node scripts/dev-extensions.mjs --only akasha,eden
//
// SIGINT/SIGTERM пробрасывается всем children. Каждый child запускается из
// `extensions/<id>/` со своим `vite.config.mjs`.

import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { listRepoExtensionEntries } from "./repo-extension-roots.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const shellRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(shellRoot, "..", "..");
// Vite не установлен внутри extension'овских node_modules (там только
// workspace symlinks @kosmos/*). Используем bin'арник из platform/desktop/node_modules.
const viteBin = path.join(shellRoot, "node_modules", "vite", "bin", "vite.js");

function argValue(name) {
  const index = process.argv.indexOf(name);
  if (index >= 0 && process.argv[index + 1] && !process.argv[index + 1].startsWith("--")) {
    return process.argv[index + 1];
  }
  const prefix = `${name}=`;
  const match = process.argv.find((arg) => arg.startsWith(prefix));
  return match ? match.slice(prefix.length) : null;
}

const onlyValue = argValue("--only");
const onlyIds = onlyValue
  ? new Set(
      onlyValue
        .split(",")
        .map((id) => id.trim())
        .filter(Boolean),
    )
  : null;

if (onlyValue !== null && onlyIds?.size === 0) {
  console.error("[dev:extensions] --only requires at least one extension id");
  process.exit(1);
}

const requestedIds = onlyIds ? new Set(onlyIds) : null;

const extensions = listRepoExtensionEntries(repoRoot)
  .filter((entry) => !requestedIds || requestedIds.has(entry.id) || requestedIds.has(entry.folder))
  .map((entry) => {
    const dir = entry.dir;
    const viteConfig = path.join(dir, "vite.config.mjs");
    if (!existsSync(viteConfig)) return null;
    const m = entry.manifest;
    if (m.kind !== "vue" || !m.devPort) return null;
    return { id: entry.id, folder: entry.folder, devPort: m.devPort, dir };
  })
  .filter(Boolean);

if (requestedIds) {
  for (const extension of extensions) {
    requestedIds.delete(extension.id);
    requestedIds.delete(extension.folder);
  }
  if (requestedIds.size > 0) {
    console.error(
      `[dev:extensions] unknown or non-dev Vue extension id(s): ${Array.from(requestedIds).join(
        ", ",
      )}`,
    );
    process.exit(1);
  }
}

if (extensions.length === 0) {
  const suffix = onlyIds ? ` for --only ${Array.from(onlyIds).join(",")}` : "";
  console.error(
    `[dev:extensions] no Vue extensions with devPort found${suffix} — nothing to start`,
  );
  process.exit(1);
}

console.log(`[dev:extensions] starting ${extensions.length} Vite dev servers`);
for (const { id, devPort } of extensions) {
  console.log(`  ${id}: http://localhost:${devPort}/`);
}

const children = extensions.map(({ id, devPort, dir }) =>
  spawn(
    process.execPath,
    [
      viteBin,
      "--port",
      String(devPort),
      "--strictPort",
      "--host",
      "127.0.0.1",
      "--configLoader",
      "native",
    ],
    {
      cwd: dir,
      stdio: ["ignore", "inherit", "inherit"],
      env: { ...process.env, VITE_KEPLER_EXTENSION_ID: id },
    },
  ),
);

let shuttingDown = false;
function killProcessTree(child) {
  if (child.killed) return;
  if (process.platform === "win32" && child.pid) {
    spawnSync("taskkill", ["/pid", String(child.pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    });
    return;
  }
  try {
    child.kill();
  } catch {
    /* already exited */
  }
}

function cleanup() {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const c of children) {
    killProcessTree(c);
  }
  process.exit(0);
}
process.on("SIGINT", cleanup);
process.on("SIGTERM", cleanup);

// Если все child'ы умерли — выходим тоже.
let alive = children.length;
for (const c of children) {
  c.on("exit", () => {
    alive -= 1;
    if (alive === 0 && !shuttingDown) {
      console.error("[dev:extensions] all vite servers exited");
      process.exit(1);
    }
  });
}

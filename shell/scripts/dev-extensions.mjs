// Orchestrator: запускает Vite dev server для каждого Vue extension'а
// с его уникальным `devPort` из manifest.json. HMR live-reload для extension
// разработки в стиле Raycast.
//
// Запуск:
//   bun run dev:extensions
//   # или
//   node scripts/dev-extensions.mjs
//
// SIGINT/SIGTERM пробрасывается всем children. Каждый child запускается из
// `extensions/<id>/` со своим `vite.config.mjs`.

import { spawn } from "node:child_process";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const extensionsRoot = path.resolve(__dirname, "..", "..", "extensions");

const extensions = readdirSync(extensionsRoot, { withFileTypes: true })
  .filter((d) => d.isDirectory())
  .map((d) => {
    const dir = path.join(extensionsRoot, d.name);
    const manifestPath = path.join(dir, "manifest.json");
    const viteConfig = path.join(dir, "vite.config.mjs");
    if (!existsSync(manifestPath) || !existsSync(viteConfig)) return null;
    try {
      const m = JSON.parse(readFileSync(manifestPath, "utf8"));
      if (m.kind !== "vue" || !m.devPort) return null;
      return { id: d.name, devPort: m.devPort, dir };
    } catch {
      return null;
    }
  })
  .filter(Boolean);

if (extensions.length === 0) {
  console.error(
    "[dev:extensions] no Vue extensions with devPort found — nothing to start",
  );
  process.exit(1);
}

console.log(`[dev:extensions] starting ${extensions.length} Vite dev servers`);
for (const { id, devPort } of extensions) {
  console.log(`  ${id}: http://localhost:${devPort}/`);
}

const children = extensions.map(({ id, devPort, dir }) =>
  spawn(
    "bunx",
    [
      "vite",
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
      shell: true,
      env: { ...process.env, VITE_KEPLER_EXTENSION_ID: id },
    },
  ),
);

let shuttingDown = false;
function cleanup() {
  if (shuttingDown) return;
  shuttingDown = true;
  for (const c of children) {
    try {
      c.kill();
    } catch {
      /* already exited */
    }
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

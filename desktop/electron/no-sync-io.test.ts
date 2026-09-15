import { expect, test } from "../test-support/node-test.mjs";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Regression: 2026-06-10, .agent/tasks/2026-06-10-electron-main-async-io.
// Синхронный fs/child_process I/O в IPC/protocol handler'ах вешает все окна shell'а.
// Для каждого файла — список запрещённых синхронных вызовов на его горячих путях.
// Файлы вне списка (focus-widget, dashboard-window и т.п.) осознанно не покрыты:
// их sync-вызовы — мелкие одноразовые JSON state read/write (см. spec § Scope).
const bannedByFile = {
  "diagnostics.ts": [
    "readFileSync",
    "writeFileSync",
    "copyFileSync",
    "rmSync",
    "readdirSync",
    "statSync",
    "spawnSync",
  ],
  "settings-window.ts": ["execFileSync"],
} satisfies Record<string, readonly string[]>;

for (const [file, banned] of Object.entries(bannedByFile)) {
  test(`${file}: нет синхронного I/O на горячих путях`, async () => {
    const source = await readFile(path.join(import.meta.dirname, file), "utf8");
    const found = banned.filter((name) => source.includes(name));
    expect(found).toEqual([]);
  });
}

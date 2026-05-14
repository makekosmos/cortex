// Recovery скрипт после Agent 3 (services/) swap: восстанавливает корректное
// UTF-8 содержимое для файлов, которые PowerShell-агент испортил через
// Get-Content/Set-Content без явного -Encoding utf8 (на Windows PS 5.1 default
// codepage cp1251 → mojibake кириллицы при чтении и UTF-16 BOM при записи).
//
// Алгоритм:
//   1. Для каждого broken file определить OLD path в HEAD (через reverse rename).
//   2. `git show HEAD:<old>` — получить оригинальные UTF-8 байты.
//   3. Применить swap правила (two-pass через placeholder).
//   4. Записать на NEW path как UTF-8 без BOM.
//
// Запуск: bun run scripts/fix-mojibake.mjs

import { execSync } from "node:child_process";
import { writeFileSync } from "node:fs";

// new path → old path в HEAD
const renameMap = {
  "apps/kepler/": "apps/kosmos/",
  "services/kepler-backend/": "services/kosmos-backend/",
  "services/kepler-watcher/": "services/kosmos-watcher/",
  "legacy/usage-tracker/src/kepler_client.rs":
    "services/usage-tracker/src/cosmos_client.rs",
};

function deriveOldPath(newPath) {
  // Точное совпадение для file-rename
  if (renameMap[newPath]) return renameMap[newPath];

  // Префиксная замена для dir-rename
  for (const [newPrefix, oldPrefix] of Object.entries(renameMap)) {
    if (newPath.startsWith(newPrefix)) {
      return oldPrefix + newPath.slice(newPrefix.length);
    }
  }
  return newPath; // файл не был переименован
}

function swap(s) {
  let r = s;
  // Phase 1: kepler/Kepler/KEPLER → placeholder
  r = r.replaceAll("kepler", "\x01");
  r = r.replaceAll("Kepler", "\x02");
  r = r.replaceAll("KEPLER", "\x03");
  // Phase 2: kosmos → kepler
  r = r.replaceAll("kosmos", "kepler");
  r = r.replaceAll("Kosmos", "Kepler");
  r = r.replaceAll("KOSMOS", "KEPLER");
  // Phase 3: cosmos (alternate spelling) → kepler
  r = r.replaceAll("cosmos", "kepler");
  r = r.replaceAll("Cosmos", "Kepler");
  r = r.replaceAll("COSMOS", "KEPLER");
  // Phase 4: placeholder → kosmos
  r = r.replaceAll("\x01", "kosmos");
  r = r.replaceAll("\x02", "Kosmos");
  r = r.replaceAll("\x03", "KOSMOS");
  return r;
}

const broken = [
  "services/kepler-backend/Cargo.toml",
  "services/kepler-backend/src/main.rs",
  "services/kepler-backend/src/lib.rs",
  "services/kepler-backend/src/sync.rs",
  "services/kepler-backend/src/lock_file.rs",
  "services/kepler-backend/src/ws_server.rs",
  "services/kepler-backend/src/singleton.rs",
  "services/kepler-backend/src/protocol_version.rs",
  "services/kepler-backend/src/ark_host.rs",
  "services/kepler-backend/src/auth.rs",
  "services/kepler-watcher/Cargo.toml",
  "services/kepler-watcher/src/main.rs",
  "services/ark-relay-server/Cargo.toml",
  "legacy/usage-tracker/src/main.rs",
  "legacy/usage-tracker/src/spool.rs",
  "legacy/usage-tracker/src/kepler_client.rs",
  "legacy/usage-tracker/Cargo.toml",
  "apps/eden/ts/main/store.ts",
];

let fixed = 0;
let skipped = 0;
const errors = [];

for (const newPath of broken) {
  const oldPath = deriveOldPath(newPath);
  try {
    const headContent = execSync(
      `git show HEAD:"${oldPath}"`,
      { encoding: "utf8", maxBuffer: 10 * 1024 * 1024 },
    );
    const swapped = swap(headContent);
    writeFileSync(newPath, swapped, { encoding: "utf8" });
    console.log(`fixed: ${newPath}  (HEAD: ${oldPath})`);
    fixed += 1;
  } catch (e) {
    errors.push({ path: newPath, error: e.message.split("\n")[0] });
    skipped += 1;
  }
}

console.log("");
console.log(`Summary: fixed ${fixed}, skipped ${skipped}`);
if (errors.length > 0) {
  console.log("Errors:");
  for (const e of errors) {
    console.log(`  ${e.path}: ${e.error}`);
  }
  process.exit(1);
}

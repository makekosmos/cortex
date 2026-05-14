// Применяет token swap (kepler↔kosmos) к файлам после `git checkout --theirs`
// во время merge main → kosmos/phase-1-scaffold. Main написан под старые
// имена; мы прокатываем их через тот же two-pass placeholder swap.
//
// Также обрабатывает special-case: Dropdown.vue добавлен в main по старому
// пути packages/kepler-visuals/, надо перевезти в packages/kosmos-visuals/.

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync } from "node:fs";

function swap(s) {
  let r = s;
  r = r.replaceAll("kepler", "\x01");
  r = r.replaceAll("Kepler", "\x02");
  r = r.replaceAll("KEPLER", "\x03");
  r = r.replaceAll("kosmos", "kepler");
  r = r.replaceAll("Kosmos", "Kepler");
  r = r.replaceAll("KOSMOS", "KEPLER");
  r = r.replaceAll("cosmos", "kepler");
  r = r.replaceAll("Cosmos", "Kepler");
  r = r.replaceAll("COSMOS", "KEPLER");
  r = r.replaceAll("\x01", "kosmos");
  r = r.replaceAll("\x02", "Kosmos");
  r = r.replaceAll("\x03", "KOSMOS");
  return r;
}

const filesToSwap = [
  "apps/delphi/AGENTS.md",
  "apps/delphi/kotlin/AGENTS.md",
  "apps/horologion/AGENTS.md",
  "apps/horologion/package.json",
  "apps/horologion/src/App.vue",
  "apps/horologion/src/main.ts",
  "apps/horologion/src/views/ListView.vue",
  "apps/horologion/src/views/HomeView.vue",
  "apps/horologion/src/views/StopwatchView.vue",
  "docs-site/apps/delphi.md",
  "docs-site/apps/horologion.md",
  "docs-site/apps/index.md",
  "docs-site/public/llms.txt",
  "docs-site/reference/commands.md",
];

for (const f of filesToSwap) {
  if (!existsSync(f)) {
    console.log(`skip (no file): ${f}`);
    continue;
  }
  const c = readFileSync(f, "utf8");
  writeFileSync(f, swap(c), "utf8");
  execSync(`git add "${f}"`);
  console.log(`swapped + added: ${f}`);
}

// Special case: Dropdown.vue добавлен в main под packages/kepler-visuals/,
// должен жить в packages/kosmos-visuals/ (rename HEAD-side).
const dropdownOldPath = "packages/kepler-visuals/components/Dropdown.vue";
const dropdownNewPath = "packages/kosmos-visuals/components/Dropdown.vue";

try {
  const mainContent = execSync(`git show MERGE_HEAD:"${dropdownOldPath}"`, {
    encoding: "utf8",
  });
  const swapped = swap(mainContent);
  writeFileSync(dropdownNewPath, swapped, "utf8");
  execSync(`git add "${dropdownNewPath}"`);
  // Снять conflict-флаг со старого пути (его не существует в HEAD).
  try {
    execSync(`git rm --cached "${dropdownOldPath}"`, { stdio: "pipe" });
  } catch (e) {
    // если файла нет в index — ок
  }
  console.log(`relocated + swapped: ${dropdownOldPath} → ${dropdownNewPath}`);
} catch (e) {
  console.log(`Dropdown.vue handling failed: ${e.message.split("\n")[0]}`);
}

console.log("");
console.log("Done. Run `git status --short` for next steps.");

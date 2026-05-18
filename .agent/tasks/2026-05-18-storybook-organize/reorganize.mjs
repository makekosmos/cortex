// One-shot script: пересортировать stories в категории + добавить layout.
// Categories по domain'у компонента, layout — чтобы preview не растягивался
// на 100vh для маленьких компонентов.

import fs from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..");
const components = path.join(repoRoot, "packages/visuals/components");

const categories = {
  // Окна / Desktop chrome — нужны реальный viewport.
  Window: ["DesktopChrome", "DesktopContentSurface", "Titlebar", "TitlebarHistoryControls"],
  // Inputs / controls.
  Inputs: ["Toggle", "SettingsRow", "Dropdown", "DateChip", "DateTimePicker", "TimeColumn", "Calendar"],
  // Display / data cards.
  Display: ["StatusDot", "EmptyState", "BlocklistCard", "GamePosterCard"],
  // Lists.
  Lists: ["TodoRow"],
  // Overlays / modals / floating UI.
  Overlays: ["Modal", "ContextMenu", "ContextMenuItem", "CommandPalette", "QuickEntryPanel"],
};

// Layout per category: fullscreen для тех что нужен real viewport
// (window chrome, overlays с backdrop), centered для остальных.
const layoutPerCategory = {
  Window: "fullscreen",
  Overlays: "fullscreen",
  Inputs: "centered",
  Display: "centered",
  Lists: "centered",
};

const componentToCategory = {};
for (const [cat, items] of Object.entries(categories)) {
  for (const c of items) componentToCategory[c] = cat;
}

let changed = 0;

for (const file of fs.readdirSync(components)) {
  if (!file.endsWith(".stories.ts")) continue;
  const name = file.replace(".stories.ts", "");
  const cat = componentToCategory[name];
  if (!cat) {
    console.warn(`[skip] no category for ${name}`);
    continue;
  }

  const filePath = path.join(components, file);
  let src = fs.readFileSync(filePath, "utf8");

  // Replace `title: "Components/X"` → `title: "<Category>/X"`.
  const titleRegex = /title:\s*["'`]Components\/([^"'`]+)["'`]/;
  if (titleRegex.test(src)) {
    src = src.replace(titleRegex, `title: "${cat}/${name}"`);
  }

  // Ensure parameters has `layout: "<centered|fullscreen>"` если не задан.
  const desiredLayout = layoutPerCategory[cat];
  const hasLayout = /layout:\s*["'`]/.test(src);
  if (!hasLayout) {
    // Look for `parameters: { ... }` or `parameters: {}` или вообще нет parameters.
    if (/parameters:\s*\{/.test(src)) {
      // Insert layout into existing parameters block.
      src = src.replace(
        /parameters:\s*\{/,
        `parameters: { layout: "${desiredLayout}",`,
      );
    } else {
      // Insert parameters near `tags: ["autodocs"]` или после component:.
      src = src.replace(
        /tags:\s*\["autodocs"\],?/,
        `tags: ["autodocs"],\n  parameters: { layout: "${desiredLayout}" },`,
      );
    }
  }

  fs.writeFileSync(filePath, src, "utf8");
  changed++;
  console.log(`[ok] ${name} → ${cat}, layout=${desiredLayout}`);
}

console.log(`\n${changed} stories updated`);

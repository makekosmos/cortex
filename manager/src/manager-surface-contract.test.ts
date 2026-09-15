import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, test } from "../../test-support/node-test.mjs";
import { resolveWorkspacePaths } from "../../scripts/workspace.mjs";

const read = (name: string) => readFileSync(new URL(`./views/${name}`, import.meta.url), "utf8");
const workspacePaths = resolveWorkspacePaths(
  path.resolve(import.meta.dirname, "../.."),
  process.env,
);

describe("Manager surface contract", () => {
  test("keeps diagnostics out of the dashboard and standalone apps too", () => {
    const root = readFileSync(new URL("./ManagerRoot.vue", import.meta.url), "utf8");
    expect(root).not.toContain('label: "Диагностика"');
    expect(root).not.toContain("DiagnosticsView");
    expect(root).not.toContain('"com.kosmos.dictation"');
    expect(root).not.toContain('"com.kosmos.focus"');
    expect(root).not.toContain("DictationSettingsView");
    expect(root).not.toContain("FocusView");
  });

  test("marketplace is one canonical app grid", () => {
    const source = read("StoreView.vue");
    expect(source).toContain("retiredListingIds");
    expect(source).toContain('"com.kosmos.eden"');
    expect(source).toContain('"com.kosmos.delphi"');
    expect(source).toContain("!retiredListingIds.has(item.id)");
    expect(source).toContain('item.kind === "kosmos-package"');
    expect(source).toContain('"ark-markdown-bridge"');
    expect(source).toContain('"external.obsidian"');
    expect(read("StoreMarketplaceControls.vue")).toContain("<h1>Приложения</h1>");
    expect(read("StoreMarketplaceControls.vue")).not.toContain("Приложения и интеграции");
    expect(source).not.toContain("Рекомендуем");
    expect(source).not.toContain("store-section-title");
    expect(source).toContain("store-grid");
  });

  test("integrations list catalog sources and keeps Huawei visible", () => {
    const connections = read("ConnectionsView.vue");
    const helpers = readFileSync(new URL("./connection-helpers.ts", import.meta.url), "utf8");
    expect(connections).toContain('"getStoreCatalog"');
    expect(connections).toContain('"refreshStoreCatalog"');
    expect(helpers).toContain('listing.kind !== "integration"');
    expect(helpers).toContain('"ark-markdown-bridge"');
    expect(helpers).toContain('"com.kosmos.huawei-health"');
  });

  test("data and titlebar use the shared contracts", () => {
    const data = read("DataView.vue");
    const root = readFileSync(new URL("./ManagerRoot.vue", import.meta.url), "utf8");
    expect(data).toContain("<SettingsList>");
    expect(data).toContain("<SettingsRow");
    expect(data).toContain("const objectCount");
    expect(data).toContain('title="Данных"');
    expect(data).not.toContain("listObjects");
    expect(data).not.toContain("searchObjects");
    expect(data).not.toContain('<p v-if="!summaryTypes.length"');
    expect(root).toContain("#titlebar-leading");
    expect(root).toContain("Kosmos");
    expect(root).toContain("<DesktopChrome");
    expect(root).toContain("kosmos-titlebar-brand");
    expect(readFileSync(new URL("./main.ts", import.meta.url), "utf8")).toContain(
      'import "@kosmos/visuals/css"',
    );
    if (!existsSync(workspacePaths.imago)) return;
    const chrome = readFileSync(
      path.join(workspacePaths.imago, "components/DesktopChrome.vue"),
      "utf8",
    );
    expect(chrome).toContain("grid-rows-[auto_minmax(0,1fr)]");
    expect(chrome).toContain("col-span-full row-start-1");
    expect(chrome).toContain("col-start-1 row-start-2");
    expect(chrome).toContain("sidebar-titlebar-divider");
    expect(chrome).toContain("border-r border-[var(--border-color-low-emphasis)]");
    expect(chrome).toContain("background: var(--kosmos-titlebar-background)");
    const sidebar = readFileSync(path.join(workspacePaths.imago, "components/Sidebar.vue"), "utf8");
    expect(sidebar).toContain("scrollbar-gutter: stable both-edges");
    expect(sidebar).toContain("padding-inline: 0");
    const theme = readFileSync(path.join(workspacePaths.imago, "theme/css-variables.css"), "utf8");
    expect(theme).toContain("--kosmos-titlebar-background: #2a2a2a");
    expect(theme).toContain("letter-spacing: -0.015em");
    expect(readFileSync(new URL("./styles.css", import.meta.url), "utf8")).not.toContain(
      ".manager-chrome .kosmos-desktop-chrome-settings__header",
    );
  });

  test("updates page owns update actions and About stays informational", () => {
    const root = readFileSync(new URL("./ManagerRoot.vue", import.meta.url), "utf8");
    const updates = read("UpdatesView.vue");
    const about = readFileSync(
      new URL("../../desktop/src/views/settings/tabs/AboutTab.vue", import.meta.url),
      "utf8",
    );
    expect(root).toContain('label: "Обновления"');
    expect(root).toContain("UpdatesView");
    expect(updates).toContain("Проверить обновления");
    expect(updates).toContain("claimUpdatesSessionCheck");
    expect(updates).not.toContain("Обновить всё");
    expect(updates).not.toContain("Проверка Desktop и приложений через их штатные каналы.");
    expect(updates).toContain("<SettingsList>");
    expect(updates).toContain('"checkDesktopUpdates"');
    expect(updates).not.toContain("updates-toolbar");
    expect(read("UpdatesRow.vue")).toContain('variant="surface"');
    expect(updates).toContain("Перезапустить и установить");
    expect(about).not.toContain("@click=\"$emit('checkUpdates')\"");
    expect(about).not.toContain("<Button");
  });

  test("marketplace icons are uniform and have no background frame", () => {
    const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
    const card = read("StoreListingCard.vue");
    expect(card).toContain("store-card-icon-frame");
    expect(css).toContain(".store-card-icon");
    expect(css).toMatch(
      /\.store-card-icon-frame\s*\{[^}]*width: 32px;[^}]*height: 32px;[^}]*flex: none;/s,
    );
    expect(css).toMatch(
      /\.store-view \.store-card-icon-frame\s*\{[^}]*width: 48px;[^}]*height: 48px;/s,
    );
    expect(css).toMatch(
      /\.store-card-icon\s*\{[^}]*width: 100%;[^}]*height: 100%;[^}]*object-fit: cover;[^}]*border-radius: var\(--radius-input\);/s,
    );
    expect(css).toMatch(/\.store-card-icon-frame\s*\{(?![^}]*\b(?:background|filter):)[^}]*\}/s);
    expect(css).not.toContain(".store-card-featured .store-card-icon");
    expect(card).toContain(
      "appIcon(props.listing.id, props.listing.icon_url, props.installed?.icon_path)",
    );
  });

  test("updates reuse the Marketplace icon identity palette", () => {
    const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
    const row = read("UpdatesRow.vue");
    expect(row).toContain("SettingsRow");
    expect(row).not.toContain("Установлена");
    expect(row).toContain("Доступна {{ available }}");
    expect(row).toContain('class="store-card-icon-frame"');
    expect(row).toContain('class="store-card-icon"');
    expect(row).toContain('class="store-card-icon-frame"');
    expect(css).toMatch(/\.store-card-icon-frame\s*\{(?![^}]*background:)[^}]*\}/s);
  });

  test("updates keep canonical titles and shared row roots", () => {
    const updates = read("UpdatesView.vue");
    const row = read("UpdatesRow.vue");
    const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
    expect(updates).toContain("installedItem.name ?? listing?.name ?? installedItem.id");
    expect(updates).toContain(
      "appIcon(installedItem.id, listing?.icon_url, installedItem.icon_path)",
    );
    expect(updates).not.toContain("fallbackName");
    expect(row).toMatch(/<template>\s*<SettingsRow[\s\S]*<\/SettingsRow>\s*<\/template>/);
    expect(row).toContain("<template #leading-icon>");
    expect(row).not.toContain('<div class="updates-row">');
    expect(css).not.toContain(".updates-row {");
  });

  test("updates distinguish unavailable packages from an empty installed list", () => {
    const updates = read("UpdatesView.vue");
    expect(updates).toContain("const packagesAvailable = ref(false)");
    expect(updates).toContain("Пакеты недоступны: проверьте соединение и повторите проверку.");
    expect(updates).toContain("packagesAvailable.value = catalog !== null && packages !== null");
    expect(updates).toContain("!actionFor(item)");
    expect(updates).toContain("item.listing?.distribution?.version");
    expect(updates).toContain("actionLabel(item)");
    expect(updates).toContain("updates-initial-store-catalog");
    expect(updates).toContain("createDesktopVersionCache");
  });

  test("exposes local ARK snapshots with accessible manual actions", () => {
    const root = readFileSync(new URL("./ManagerRoot.vue", import.meta.url), "utf8");
    const settings = read("SettingsView.vue");
    const api = readFileSync(new URL("./manager-api.ts", import.meta.url), "utf8");
    const preload = readFileSync(new URL("../electron/preload.ts", import.meta.url), "utf8");
    const main = readFileSync(new URL("../electron/main.ts", import.meta.url), "utf8");
    expect(root).toContain('label: "Настройки"');
    expect(settings).toContain("Резервные копии базы");
    expect(settings).toContain("локальные снимки базы данных ARK");
    expect(settings).toContain("Сделать бэкап сейчас");
    expect(settings).toContain("Открыть папку бэкапов");
    expect(settings).toContain("безопасный Runtime API");
    expect(settings).not.toContain("Восстановить");
    for (const method of ["getDbBackups", "createDbBackup", "openDbBackupsFolder"]) {
      expect(api).toContain(method);
      expect(preload).toContain(method);
      expect(main).toContain(`manager.${method}`);
    }
    expect(api).not.toContain("restoreDbBackup");
    expect(preload).not.toContain("restoreDbBackup");
    expect(main).not.toContain("restoreDbBackup");
  });
});

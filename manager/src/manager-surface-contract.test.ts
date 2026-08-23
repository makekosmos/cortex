import { readFileSync } from "node:fs";
import { describe, expect, test } from "bun:test";

const read = (name: string) => readFileSync(new URL(`./views/${name}`, import.meta.url), "utf8");

describe("Manager surface contract", () => {
  test("marketplace is one canonical app grid", () => {
    const source = read("StoreView.vue");
    for (const id of [
      "com.kosmos.shell",
      "com.kosmos.eden",
      "com.kosmos.agenda",
      "com.kosmos.graph",
      "com.kosmos.dictation",
    ]) {
      expect(source).toContain(id);
    }
    expect(source).not.toContain("Рекомендуем");
    expect(source).not.toContain("store-section-title");
    expect(source).toContain("store-grid");
    expect(source).toContain("icon_url: listing.icon_url ?? app.icon_url");
    expect(source).toContain('name: "Cosmos Graph"');
    expect(source).toContain('name: "Dictation"');
  });

  test("data and titlebar use the shared contracts", () => {
    const data = read("DataView.vue");
    const root = readFileSync(new URL("./ManagerRoot.vue", import.meta.url), "utf8");
    expect(data).toContain("<SettingsList>");
    expect(data).toContain("<SettingsRow");
    expect(root).toContain("#titlebar-leading");
    expect(root).toContain("Cosmos");
    expect(root).toContain("<DesktopChrome");
    expect(root).toContain("titlebar-above-sidebar");
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
    expect(updates).toContain("Обновить всё");
    expect(updates).not.toContain("Проверка Desktop и приложений через их штатные каналы.");
    expect(updates).toContain("<SettingsList>");
    expect(updates).toContain('<Button variant="ghost"');
    expect(updates).toContain("Перезапустить и установить");
    expect(about).not.toContain("@click=\"$emit('checkUpdates')\"");
    expect(about).not.toContain("<Button");
  });

  test("marketplace icon is a rounded square", () => {
    const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
    const card = read("StoreListingCard.vue");
    expect(card).toContain("store-card-icon-frame");
    expect(css).toContain(".store-card-icon");
    expect(css).toMatch(
      /\.store-card-icon-frame\s*\{[^}]*width: 48px;[^}]*height: 48px;[^}]*overflow: hidden;[^}]*border: 2px solid var\(--border-color-strong\);[^}]*border-radius: var\(--radius-input\);[^}]*background: var\(--surface\);/s,
    );
    expect(css).toMatch(
      /\.store-card-icon\s*\{[^}]*width: 100%;[^}]*height: 100%;[^}]*object-fit: cover;[^}]*border-radius: 0;/s,
    );
    for (const app of ["shell", "eden", "delphi", "graph", "dictation"]) {
      expect(css).toContain(`.store-card-icon-frame--com-kosmos-${app}`);
    }
    expect(css).toContain("filter: sepia(1)");
    expect(css).not.toContain(".store-card-featured .store-card-icon");
    expect(card).toContain("https:|http:|file:|data:image\\/|\\/");
  });

  test("updates reuse the Marketplace icon identity palette", () => {
    const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
    const row = read("UpdatesRow.vue");
    expect(row).toContain("SettingsRow");
    expect(row).not.toContain("Установлена");
    expect(row).toContain("Доступна {{ available }}");
    expect(row).toContain('class="store-card-icon-frame"');
    expect(row).toContain('class="store-card-icon"');
    expect(css).toContain(".store-card-icon-frame--desktop");
    expect(css).not.toContain("updates-row-icon-frame--com-kosmos-");
    for (const app of ["shell", "eden", "delphi", "graph", "dictation"])
      expect(row).not.toContain(`updates-row-icon-frame--com-kosmos-${app}`);
  });

  test("updates keep canonical titles and shared row roots", () => {
    const updates = read("UpdatesView.vue");
    const row = read("UpdatesRow.vue");
    const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
    expect(updates).toContain("name: fallbackName");
    expect(updates).not.toContain("listing?.name ?? fallbackName");
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
    expect(updates).toContain(
      ':disabled="!packagesAvailable || item.installedItem?.revoked === true"',
    );
    expect(updates).toContain(
      ":current=\"packagesAvailable ? (item.installedItem?.version ?? '—') : '—'\"",
    );
  });
});

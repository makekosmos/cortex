import { readFileSync } from "node:fs";
import { describe, expect, test } from "bun:test";

const read = (name: string) =>
  readFileSync(new URL(`./views/${name}`, import.meta.url), "utf8");

describe("Manager surface contract", () => {
  test("marketplace is one canonical app grid", () => {
    const source = read("StoreView.vue");
    for (const id of [
      "com.kosmos.shell",
      "com.kosmos.eden",
      "com.kosmos.delphi",
      "com.kosmos.arcadia",
    ]) {
      expect(source).toContain(id);
    }
    expect(source).not.toContain("Рекомендуем");
    expect(source).not.toContain("store-section-title");
    expect(source).toContain("store-grid");
    expect(source).toContain("? { ...listing, ...app }");
    expect(source).toContain('name: "Memoria"');
    expect(source).toContain('name: "Agenda"');
    expect(source).toContain('name: "Arcadia"');
  });

  test("data and titlebar use the shared contracts", () => {
    const data = read("DataView.vue");
    const root = readFileSync(
      new URL("./ManagerRoot.vue", import.meta.url),
      "utf8",
    );
    expect(data).toContain("<SettingsList>");
    expect(data).toContain("<SettingsRow");
    expect(data).toContain('title="Типы данных пока недоступны." muted');
    expect(data).toContain('title="Нет объектов для отображения." muted');
    expect(data).toContain('class="list-title settings-list__title"');
    expect(data).not.toContain('<p v-if="!summaryTypes.length"');
    expect(data).not.toContain('<p v-if="!rows.length"');
    expect(root).toContain("#titlebar-leading");
    expect(root).toContain("Kosmos");
    expect(root).toContain("<DesktopChrome");
    expect(root).toContain("kosmos-titlebar-brand");
    expect(
      readFileSync(new URL("./main.ts", import.meta.url), "utf8"),
    ).toContain('import "@kosmos/visuals/css"');
    const chrome = readFileSync(
      new URL("../../../imago/components/DesktopChrome.vue", import.meta.url),
      "utf8",
    );
    expect(chrome).toContain("grid-rows-[auto_minmax(0,1fr)]");
    expect(chrome).toContain("col-span-full row-start-1");
    expect(chrome).toContain("col-start-1 row-start-2");
    expect(chrome).toContain("sidebar-titlebar-divider");
    expect(chrome).toContain("border-r border-[var(--border-color-low-emphasis)]");
    expect(chrome).toContain("background: var(--kosmos-titlebar-background)");
    const sidebar = readFileSync(
      new URL("../../../imago/components/Sidebar.vue", import.meta.url),
      "utf8",
    );
    expect(sidebar).toContain("scrollbar-gutter: stable both-edges");
    expect(sidebar).toContain("padding-inline: 0");
    const theme = readFileSync(
      new URL("../../../imago/theme/css-variables.css", import.meta.url),
      "utf8",
    );
    expect(theme).toContain("--kosmos-titlebar-background: #2a2a2a");
    expect(theme).toContain("letter-spacing: -0.015em");
    expect(
      readFileSync(new URL("./styles.css", import.meta.url), "utf8"),
    ).not.toContain(".manager-chrome .kosmos-desktop-chrome-settings__header");
  });

  test("updates page owns update actions and About stays informational", () => {
    const root = readFileSync(
      new URL("./ManagerRoot.vue", import.meta.url),
      "utf8",
    );
    const updates = read("UpdatesView.vue");
    const about = readFileSync(
      new URL(
        "../../desktop/src/views/settings/tabs/AboutTab.vue",
        import.meta.url,
      ),
      "utf8",
    );
    expect(root).toContain('label: "Обновления"');
    expect(root).toContain("UpdatesView");
    expect(updates).toContain("Проверить обновления");
    expect(updates).toContain("Обновить всё");
    expect(updates).not.toContain(
      "Проверка Desktop и приложений через их штатные каналы.",
    );
    expect(updates).toContain("<SettingsList>");
    expect(updates).toContain('<Button variant="ghost"');
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
      /\.store-card-icon\s*\{[^}]*width: 100%;[^}]*height: 100%;[^}]*object-fit: contain;[^}]*border-radius: 0;/s,
    );
    expect(css).toContain("filter: sepia(1)");
    expect(css).not.toContain(".store-card-featured .store-card-icon");
    expect(card).toContain("https:|http:|file:|data:image\\/|\\/|\\.\\/assets\\/");
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
    expect(css).toMatch(
      /\.store-card-icon-frame\s*\{(?![^}]*background:)[^}]*\}/s,
    );
  });

  test("updates keep canonical titles and shared row roots", () => {
    const updates = read("UpdatesView.vue");
    const row = read("UpdatesRow.vue");
    const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
    expect(updates).toContain("name: fallbackName");
    expect(updates).toContain('"Memoria"');
    expect(updates).toContain('"Agenda"');
    expect(updates).toContain('"Arcadia"');
    expect(updates).not.toContain("com.kosmos.graph");
    expect(updates).not.toContain("com.kosmos.dictation");
    expect(updates).not.toContain("listing?.name ?? fallbackName");
    expect(updates).toContain("memoriaIcon");
    expect(updates).toContain("agendaIcon");
    expect(updates).toContain("arcadiaIcon");
    expect(row).toMatch(
      /<template>\s*<SettingsRow[\s\S]*<\/SettingsRow>\s*<\/template>/,
    );
    expect(row).toContain("<template #leading-icon>");
    expect(row).not.toContain('<div class="updates-row">');
    expect(css).not.toContain(".updates-row {");
  });

  test("updates distinguish unavailable packages from an empty installed list", () => {
    const updates = read("UpdatesView.vue");
    expect(updates).toContain("const packagesAvailable = ref(false)");
    expect(updates).toContain(
      "Пакеты недоступны: проверьте соединение и повторите проверку.",
    );
    expect(updates).toContain(
      "packagesAvailable.value = catalog !== null && packages !== null",
    );
    expect(updates).toContain("!actionFor(item)");
    expect(updates).toContain("item.listing?.distribution?.version");
    expect(updates).toContain("actionLabel(item)");
    expect(updates).toContain("updates-initial-store-catalog");
    expect(updates).toContain("createDesktopVersionCache");
  });
});

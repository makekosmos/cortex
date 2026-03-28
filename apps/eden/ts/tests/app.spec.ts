import { test, expect, _electron as electron, type Page } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

type ElectronApp = Awaited<ReturnType<typeof electron.launch>>;

interface LaunchedApp {
  electronApp: ElectronApp;
  window: Page;
  pageErrors: string[];
  homePath: string;
}

interface EntryPayload {
  id: string;
  title: string;
  type_id: string | null;
  header_layout: string | null;
  header_props_json: string;
}

interface NoteTypePayload {
  id: string;
  name: string;
}

async function launchApp(homePath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-home-')), attempt = 0): Promise<LaunchedApp> {
  const electronApp = await electron.launch({
    args: ['.'],
    env: {
      ...process.env,
      EDEN_BACKGROUND_LAUNCH: '1',
      HOME: homePath,
      NODE_ENV: 'development',
    },
  });

  const window = await electronApp.firstWindow();
  const pageErrors: string[] = [];

  window.on('console', (msg) => console.log(msg.text()));
  window.on('pageerror', (error) => {
    pageErrors.push(error.message);
    console.log('Page error:', error);
  });

  await window.waitForLoadState('domcontentloaded');

  try {
    await window.waitForSelector('.app-container', { timeout: 10000 });
    return { electronApp, window, pageErrors, homePath };
  } catch (error) {
    await electronApp.close();

    if (attempt >= 1) {
      throw error;
    }

    return launchApp(homePath, attempt + 1);
  }
}

async function ensureVault(launch: LaunchedApp, vaultPath: string): Promise<LaunchedApp> {
  const { window, electronApp, homePath } = launch;

  await window.waitForSelector('.app-container');

  await window.evaluate(async (selectedVaultPath: string) => {
    const api = Reflect.get(window, 'api');
    if (api && typeof api === 'object') {
      const setVaultPath = Reflect.get(api, 'setVaultPath');
      if (typeof setVaultPath === 'function') {
        await setVaultPath(selectedVaultPath);
      }
      const updateSidebarConfig = Reflect.get(api, 'updateSidebarConfig');
      if (typeof updateSidebarConfig === 'function') {
        await updateSidebarConfig({
          vault: { width: 232, collapsed: false },
          widget: { width: 320, collapsed: false },
        });
      }
    }
  }, vaultPath);

  await electronApp.close();
  const relaunched = await launchApp(homePath);
  await relaunched.window.waitForSelector('.widget-sidebar-wrapper', { state: 'attached' });
  return relaunched;
}

async function createNote(launch: LaunchedApp, title: string, content: string): Promise<LaunchedApp> {
  const { window, electronApp, homePath } = launch;

  await window.evaluate(async ({ nextTitle, nextContent }: { nextTitle: string; nextContent: string }) => {
    const api = Reflect.get(window, 'api') as { saveEntry?: (entry: Entry) => Promise<SaveEntryResult> } | undefined;
    if (!api?.saveEntry) {
      return;
    }

    const now = Date.now();
    await api.saveEntry({
      id: crypto.randomUUID(),
      title: nextTitle,
      content_json: JSON.stringify({
        type: 'doc',
        content: [
          {
            type: 'paragraph',
            content: [
              {
                type: 'text',
                text: nextContent,
              },
            ],
          },
        ],
      }),
      created_at: now,
      updated_at: now,
      folder_id: null,
      type_id: null,
      header_layout: null,
      header_props_json: '{}',
      schema_version: 1,
    });
  }, { nextTitle: title, nextContent: content });

  await electronApp.close();
  const relaunched = await launchApp(homePath);
  await relaunched.window.waitForSelector('.widget-sidebar', { state: 'visible' });
  return relaunched;
}

async function createNoteType(page: Page, name: string) {
  await page.evaluate(async (noteTypeName: string) => {
    const api = Reflect.get(window, 'api') as { saveNoteType?: (noteType: NoteType) => Promise<SaveNoteTypeResult> } | undefined;
    if (!api?.saveNoteType) {
      return;
    }

    const now = Date.now();
    await api.saveNoteType({
      id: `note-type-${noteTypeName.toLowerCase().replace(/\s+/g, '-')}`,
      name: noteTypeName,
      slug: noteTypeName.toLowerCase().replace(/\s+/g, '-'),
      icon: '✦',
      color: '#7fb7ff',
      schema_json: JSON.stringify({
        fields: [
          {
            id: 'title',
            label: 'Title',
            kind: 'text',
            required: false,
          },
        ],
      }),
      header_template_json: JSON.stringify({ kind: 'default' }),
      created_at: now,
      updated_at: now,
    });
  }, name);
}

test.describe('Electron App', () => {
  test.describe.configure({ mode: 'serial' });

  test('should open an existing note from the list without blank screen', async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-playwright-'));
    const noteTitle = 'Playwright reopen note';
    const noteContent = 'Hello from Playwright reopen test';
    const secondTitle = 'Second note for switching';

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      launch = await createNote(launch, noteTitle, noteContent);
      launch = await createNote(launch, secondTitle, 'Secondary body');

      await expect
        .poll(async () => {
          return launch?.window.evaluate(async () => {
            const api = Reflect.get(window, 'api') as { listEntries?: () => Promise<Array<{ title: string }>> } | undefined;
            const entries = api?.listEntries ? await api.listEntries() : [];
            return entries.map((entry) => entry.title);
          })
        }, { timeout: 5000 })
        .toContain(noteTitle)

      const savedNote = launch.window.locator('.widget-nav-item', { hasText: noteTitle }).first();
      await expect(savedNote).toBeVisible();
      await savedNote.click();

      await expect(launch.window.locator('.editor-wrapper')).toBeVisible();
      await expect(launch.window.locator('.title-input')).toHaveValue(noteTitle);
      await expect(launch.window.locator('.ProseMirror')).toContainText(noteContent);
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test('should support spaces, settings screen, and focus mode flows', async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-playwright-'));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await createNoteType(launch.window, 'Проект');
      launch = await createNote(launch, 'Space alpha', 'First note inside spaces.');
      launch = await createNote(launch, 'Space beta', 'Second note inside spaces.');

      await launch.window.locator('[data-testid="widget-link-my-space"]').click();
      await expect(launch.window.locator('.editor-wrapper')).toBeVisible();
      await expect(launch.window.locator('.title-input')).toHaveValue('Мое пространство');

      await launch.window.locator('[data-testid="widget-link-all-objects"]').click();
      await expect(launch.window.locator('[data-testid="space-view-all-objects"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="space-view-all-objects"]')).toContainText('Space alpha');
      await expect(launch.window.locator('[data-testid="space-view-all-objects"]')).toContainText('Страница');

      await launch.window.locator('[data-testid="widget-link-all-properties"]').click();
      await expect(launch.window.locator('[data-testid="space-view-all-properties"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="space-view-all-properties"]')).toContainText('Проект');

      await launch.window.locator('[data-testid="widget-link-all-notes"]').click();
      await expect(launch.window.locator('[data-testid="space-view-all-notes"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="space-view-all-notes"]')).toContainText('Space beta');

      await launch.window.locator('[data-testid="space-view-all-notes"] .space-table-row', { hasText: 'Space beta' }).first().click();
      await expect(launch.window.locator('.editor-wrapper')).toBeVisible();

      // Verify diary space is accessible
      await launch.window.locator('[data-testid="widget-link-diary"]').scrollIntoViewIfNeeded();
      await launch.window.locator('[data-testid="widget-link-diary"]').click();
      await expect(launch.window.locator('[data-testid="space-view-diary"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="diary-today-section"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="diary-history-section"]')).toBeVisible();
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test('should create a custom note type and render a typed note header', async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-typed-note-'));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.evaluate(async () => {
        const api = Reflect.get(window, 'api') as { saveNoteType?: (noteType: NoteType) => Promise<SaveNoteTypeResult> } | undefined;
        if (!api?.saveNoteType) return;
        const now = Date.now();
        await api.saveNoteType({
          id: 'note-type-chelovek',
          name: 'Человек',
          slug: 'chelovek',
          icon: '✦',
          color: '#7fb7ff',
          schema_json: JSON.stringify({
            fields: [
              { id: 'name', label: 'name', kind: 'text', required: false },
              { id: 'photo', label: 'photo', kind: 'image', required: false },
            ],
          }),
          header_template_json: JSON.stringify({
            kind: 'centered_profile',
            primaryFieldIds: ['name'],
            secondaryFieldIds: [],
            imageFieldId: 'photo',
          }),
          created_at: now,
          updated_at: now,
        });
      });

      // Reload to pick up the new type
      await launch.electronApp.close();
      launch = await launchApp(launch.homePath);
      await launch.window.waitForSelector('.widget-sidebar', { state: 'visible' });

      await launch.window.click('button[title="Новая заметка"]');
      await launch.window.locator('.title-input').fill('Ada Lovelace');
      await launch.window.locator('[data-testid="typed-note-trigger"]').click();
      await expect(launch.window.locator('[data-testid="typed-note-menu"]')).toBeVisible();
      await launch.window.locator('.note-type-menu-item:has-text("Человек")').click();
      await launch.window.locator('[data-testid="typed-note-field-name"]').fill('Ada Lovelace');
      await launch.window.locator('[data-testid="typed-note-field-photo"]').fill('data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><rect width="80" height="80" fill="%2390caf9"/></svg>');
      await expect(launch.window.locator('[data-testid="typed-note-header"]')).toBeVisible();
      await expect(launch.window.locator('[data-testid="typed-note-primary"]')).toContainText('Ada Lovelace');
      await launch.window.locator('.ProseMirror').click();
      await launch.window.keyboard.insertText('First programmer note');
      await launch.window.waitForTimeout(1200);

      await expect
        .poll(async () => {
          return launch?.window.evaluate(async () => {
            const api = Reflect.get(window, 'api') as {
              listEntries?: () => Promise<Array<EntryPayload>>;
            } | undefined;
            const entries = api?.listEntries ? await api.listEntries() : [];
            return entries.some(currentEntry => currentEntry.title === 'Ada Lovelace');
          })
        }, { timeout: 10000 })
        .toBe(true)

      const typedEntry = await launch.window.evaluate(async () => {
        const api = Reflect.get(window, 'api') as {
          listEntries?: () => Promise<Array<EntryPayload>>;
          listNoteTypes?: () => Promise<Array<NoteTypePayload>>;
        } | undefined;
        const entries = api?.listEntries ? await api.listEntries() : [];
        const noteTypes = api?.listNoteTypes ? await api.listNoteTypes() : [];
        const entry = entries.find(currentEntry => currentEntry.title === 'Ada Lovelace') ?? null;
        const noteType = noteTypes.find(currentType => currentType.name === 'Человек') ?? null;
        return {
          entry,
          noteTypeId: noteType?.id ?? null,
        };
      });

      expect(typedEntry.entry).not.toBeNull();
      expect(typedEntry.noteTypeId).not.toBeNull();
      expect(typedEntry.entry?.type_id).toBe(typedEntry.noteTypeId);
      expect(typedEntry.entry?.header_layout).toBe('centered_profile');
      expect(typedEntry.entry?.header_props_json).toContain('Ada Lovelace');
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test('should show vault name in sidebar header and support collapse/expand', async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-sidebar-'));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      await launch.window.waitForSelector('.widget-sidebar', { state: 'visible' });

      const vaultName = path.basename(vaultPath);
      const vaultSwitcher = launch.window.locator('[data-testid="vault-switcher-menu"]');
      await expect(vaultSwitcher).toBeVisible();
      await expect(vaultSwitcher).toContainText(vaultName);

      const collapseBtn = launch.window.locator('[data-testid="sidebar-toggle"]');
      await expect(collapseBtn).toBeVisible();
      
      const sidebarWrapper = launch.window.locator('.widget-sidebar-wrapper');
      
      await collapseBtn.click();
      await launch.window.waitForTimeout(400);
      
      await expect(sidebarWrapper).toHaveClass(/collapsed/);

      const expandBtn = launch.window.locator('[data-testid="sidebar-toggle-external"]');
      await expect(expandBtn).toBeVisible();
      await expandBtn.click();
      await launch.window.waitForTimeout(400);
      
      await expect(sidebarWrapper).not.toHaveClass(/collapsed/);

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });

  test('should keep recent vaults in the left rail and switch between them', async () => {
    test.setTimeout(60000);

    const firstVaultPath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-vault-a-'));
    const secondVaultPath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-vault-b-'));

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, firstVaultPath);

      launch = await ensureVault(launch, secondVaultPath);
      await launch.window.waitForSelector('.widget-sidebar', { state: 'visible' });

      const firstVaultName = path.basename(firstVaultPath);
      const secondVaultName = path.basename(secondVaultPath);

      await expect(launch.window.locator(`[data-testid="vault-item-${secondVaultName}"]`)).toBeVisible();
      await expect(launch.window.locator('[data-testid="vault-switcher-menu"]')).toBeVisible();
      await expect(launch.window.locator('.vault-browser-item', { hasText: firstVaultName })).toBeVisible();

      await launch.window.locator('.vault-browser-item', { hasText: firstVaultName }).click();
      await launch.window.waitForTimeout(300);
      await expect(launch.window.locator('[data-testid="vault-switcher-menu"]')).toContainText(firstVaultName);

      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(firstVaultPath, { recursive: true, force: true });
      fs.rmSync(secondVaultPath, { recursive: true, force: true });
    }
  });

  test('should open search overlay via search button and search for notes', async () => {
    test.setTimeout(60000);

    const vaultPath = fs.mkdtempSync(path.join(os.tmpdir(), 'eden-search-'));
    const noteTitle = 'Unrelated title';
    const russianPartial = 'токен';
    const englishPartial = 'workf';
    const noteContent = 'Русский токенизатор помогает находить совпадения и english workflows тоже.';

    let launch: LaunchedApp | null = null;

    try {
      launch = await launchApp();
      launch = await ensureVault(launch, vaultPath);

      launch = await createNote(launch, noteTitle, noteContent);

      const searchBtn = launch.window.locator('[data-testid="widget-link-search"]');
      await expect(searchBtn).toBeVisible();
      
      await searchBtn.click();
      
      const searchOverlay = launch.window.locator('.search-overlay');
      await expect(searchOverlay).toBeVisible();
      
      const searchInput = launch.window.locator('.search-overlay-input');
      await expect(searchInput).toBeVisible();
      await expect(searchInput).toBeFocused();
      
      await searchInput.fill(russianPartial);
      await launch.window.waitForTimeout(500);

      const russianResult = launch.window.locator('.search-overlay-result-item').filter({ hasText: noteTitle }).first();
      await expect(russianResult).toBeVisible();
      await expect(russianResult).toContainText('токенизатор');

      await searchInput.fill(englishPartial);
      await launch.window.waitForTimeout(500);
      
      const searchResult = launch.window.locator('.search-overlay-result-item').filter({ hasText: noteTitle }).first();
      await expect(searchResult).toBeVisible();
      await expect(searchResult).toContainText('workflows');
      
      await searchResult.click();
      
      await expect(launch.window.locator('.editor-wrapper')).toBeVisible();
      await expect(launch.window.locator('.title-input')).toHaveValue(noteTitle);
      
      const overlayAfterSelect = launch.window.locator('.search-overlay');
      await expect(overlayAfterSelect).toHaveCount(0);
      
      expect(launch.pageErrors).toEqual([]);
    } finally {
      if (launch) {
        await launch.electronApp.close();
        fs.rmSync(launch.homePath, { recursive: true, force: true });
      }

      fs.rmSync(vaultPath, { recursive: true, force: true });
    }
  });
});

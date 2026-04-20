import fs from "node:fs/promises";
import path from "node:path";

function parseArgs(argv) {
  const parsed = {};
  for (let index = 2; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) {
      continue;
    }
    const key = token.slice(2);
    const value = argv[index + 1];
    parsed[key] = value;
    index += 1;
  }
  return parsed;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

class InspectorClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.ws = null;
    this.nextId = 1;
    this.pending = new Map();
  }

  async connect() {
    await new Promise((resolve, reject) => {
      const ws = new WebSocket(this.wsUrl);
      this.ws = ws;

      ws.addEventListener("open", () => resolve());
      ws.addEventListener("error", (error) => reject(error));
      ws.addEventListener("message", (event) => {
        const payload = JSON.parse(event.data.toString());
        if (typeof payload.id !== "number") {
          return;
        }

        const pending = this.pending.get(payload.id);
        if (!pending) {
          return;
        }

        this.pending.delete(payload.id);
        pending.resolve(payload);
      });
      ws.addEventListener("close", () => {
        for (const pending of this.pending.values()) {
          pending.reject(new Error("Inspector connection closed"));
        }
        this.pending.clear();
      });
    });
  }

  async send(method, params = {}) {
    if (!this.ws) {
      throw new Error("Inspector client is not connected");
    }

    const id = this.nextId++;
    const message = { id, method, params };

    const response = await new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify(message));
    });

    return response;
  }

  async close() {
    if (!this.ws) {
      return;
    }

    const ws = this.ws;
    this.ws = null;
    await new Promise((resolve) => {
      ws.addEventListener("close", () => resolve(), { once: true });
      ws.close();
    });
  }
}

function unwrapEvaluationResult(response) {
  if (response.exceptionDetails) {
    const description =
      response.result?.result?.description ??
      response.result?.result?.value ??
      response.exceptionDetails.text ??
      "Runtime.evaluate failed";
    throw new Error(String(description));
  }

  return response.result?.result?.value;
}

async function evalMain(client, expression) {
  const response = await client.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  return unwrapEvaluationResult(response);
}

async function execRenderer(client, source) {
  const expression = `(async () => {
    const { BrowserWindow } = require("electron");
    const win = BrowserWindow.getAllWindows()[0];
    if (!win) {
      throw new Error("No BrowserWindow found");
    }
    const debuggerApi = win.webContents.debugger;
    if (!debuggerApi.isAttached()) {
      debuggerApi.attach("1.3");
    }
    const response = await debuggerApi.sendCommand("Runtime.evaluate", {
      expression: ${JSON.stringify(source)},
      awaitPromise: true,
      returnByValue: true,
    });
    if (response.exceptionDetails) {
      throw new Error(
        response.result?.description ??
        response.exceptionDetails.text ??
        "Renderer Runtime.evaluate failed"
      );
    }
    return JSON.stringify(response.result?.value ?? null);
  })()`;

  const serialized = await evalMain(client, expression);
  return typeof serialized === "string" ? JSON.parse(serialized) : serialized;
}

async function waitForRenderer(client, predicateSource, label, timeoutMs = 30000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    const result = await execRenderer(client, predicateSource);
    if (result) {
      return result;
    }
    await sleep(200);
  }

  throw new Error(`Timed out waiting for ${label}`);
}

async function reloadWindow(client) {
  await evalMain(
    client,
    `(async () => {
      const { BrowserWindow } = require("electron");
      const win = BrowserWindow.getAllWindows()[0];
      if (!win) {
        throw new Error("No BrowserWindow found");
      }
      await new Promise((resolve) => {
        win.webContents.once("did-finish-load", () => setTimeout(resolve, 250));
        win.webContents.reloadIgnoringCache();
      });
      return true;
    })()`,
  );
}

function buildDoc(text) {
  return JSON.stringify({
    type: "doc",
    content: [
      {
        type: "paragraph",
        content: text
          ? [
            {
              type: "text",
              text,
            },
          ]
          : [],
      },
    ],
  });
}

async function main() {
  const args = parseArgs(process.argv);
  const wsUrl = args.ws;
  const vaultPath = args.vault;
  const resultsPath = args.out;

  if (!wsUrl || !vaultPath || !resultsPath) {
    throw new Error("Expected --ws, --vault, and --out arguments");
  }

  const results = {
    mainPing: null,
    rendererStringPing: null,
    rendererPing: null,
    customTypedHeader: null,
    settingsEditor: null,
    builtInLayoutPersistence: null,
    relationHydration: null,
  };

  const client = new InspectorClient(wsUrl);
  await client.connect();

  try {
    results.mainPing = await evalMain(client, `(async () => ({ mainPing: true, pid: process.pid }))()`);
    results.rendererStringPing = await execRenderer(client, `(() => "renderer-ok")()`);
    results.rendererPing = await execRenderer(
      client,
      `(() => ({ rendererPing: true, readyState: document.readyState, title: document.title }))()`,
    );

    await waitForRenderer(
      client,
      `(() => Boolean(document.querySelector(".app-container")))()`,
      "app container",
    );

    await execRenderer(
      client,
      `(async () => {
        await window.api.setVaultPath(${JSON.stringify(vaultPath)});
        await window.api.updateSidebarConfig({ widget: { width: 320, collapsed: false } });
        return true;
      })()`,
    );

    await reloadWindow(client);
    await waitForRenderer(
      client,
      `(() => Boolean(document.querySelector(".widget-sidebar-wrapper")))()`,
      "widget sidebar",
    );

    const customTypeId = "note-type-chelovek";
    const customEntryId = "typed-entry-ada";
    const linkedTargetId = "note-related-target";
    const linkedSourceId = "note-related-source";

    await execRenderer(
      client,
      `(async () => {
        const now = Date.now();
        await window.api.saveNoteType({
          id: ${JSON.stringify(customTypeId)},
          name: "Человек",
          slug: "chelovek",
          icon: "person",
          color: "#7fb7ff",
          schema_json: JSON.stringify({
            fields: [
              { id: "name", label: "Имя", kind: "text", required: false },
              { id: "description", label: "Описание", kind: "long_text", required: false },
              { id: "photo", label: "Фото", kind: "image", required: false },
            ],
          }),
          header_template_json: JSON.stringify({
            kind: "centered_profile",
            primaryFieldIds: ["name", "photo"],
            secondaryFieldIds: [],
            imageFieldId: "photo",
          }),
          ui_schema_json: JSON.stringify({
            featured_fields: ["name", "photo"],
            visible_fields: ["name", "photo"],
            hidden_fields: ["created_at", "updated_at", "deleted_at"],
            read_only_fields: [],
            field_order: ["description", "name", "photo"],
            header_layout: "column",
            default_layout: "page",
            default_template_id: null,
          }),
          created_at: now,
          updated_at: now,
        });

        await window.api.saveEntry({
          id: ${JSON.stringify(customEntryId)},
          title: "Ada Lovelace",
          content_json: ${JSON.stringify(buildDoc("First programmer note"))},
          created_at: now,
          updated_at: now,
          folder_id: null,
          type_id: ${JSON.stringify(customTypeId)},
          header_layout: "column",
          header_props_json: JSON.stringify({
            name: "Ada Lovelace",
            description: "Mathematician and writer",
            photo: 'data:image/svg+xml;utf8,<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80"><rect width="80" height="80" fill="%2390caf9"/></svg>',
          }),
          schema_version: 1,
          deleted_at: null,
        });

        await window.api.saveEntry({
          id: ${JSON.stringify(linkedTargetId)},
          title: "Target Note",
          content_json: ${JSON.stringify(buildDoc("Target body"))},
          created_at: now,
          updated_at: now,
          folder_id: null,
          type_id: "note_obj",
          header_layout: "default",
          header_props_json: JSON.stringify({
            description: "Target description",
            related_notes: [],
          }),
          schema_version: 1,
          deleted_at: null,
        });

        await window.api.saveEntry({
          id: ${JSON.stringify(linkedSourceId)},
          title: "Linked Note",
          content_json: ${JSON.stringify(buildDoc("Linked body"))},
          created_at: now,
          updated_at: now,
          folder_id: null,
          type_id: "note_obj",
          header_layout: "default",
          header_props_json: JSON.stringify({
            description: "Linked description",
            related_notes: [${JSON.stringify(linkedTargetId)}],
          }),
          schema_version: 1,
          deleted_at: null,
        });

        return {
          noteTypes: await window.api.listNoteTypes(),
          entries: await window.api.listEntries(),
        };
      })()`,
    );

    await reloadWindow(client);
    await waitForRenderer(
      client,
      `(() => Boolean(document.querySelector(".widget-sidebar-wrapper")))()`,
      "sidebar after data reload",
    );

    await execRenderer(
      client,
      `(() => {
        const item = [...document.querySelectorAll(".widget-nav-item")]
          .find((element) => element.textContent?.includes("Ada Lovelace"));
        if (!item) {
          throw new Error("Custom typed note list item not found");
        }
        item.click();
        return true;
      })()`,
    );

    await waitForRenderer(
      client,
      `(() => {
        const title = document.querySelector(".title-input");
        const header = document.querySelector(".typed-object-header");
        return title instanceof HTMLInputElement &&
          title.value === "Ada Lovelace" &&
          header instanceof HTMLElement;
      })()`,
      "custom typed note page",
    );

    results.customTypedHeader = await execRenderer(
      client,
      `(() => {
        const header = document.querySelector(".typed-object-header");
        const title = document.querySelector(".title-input");
        const typeBadge = document.querySelector(".typed-object-header__type-badge");
        const description = document.querySelector(".typed-object-header__description");
        const featuredLabels = [...document.querySelectorAll(".typed-object-header__properties--featured .object-property-field__label")]
          .map((element) => element.textContent?.trim())
          .filter(Boolean);
        return {
          hasHeader: header instanceof HTMLElement,
          headerClass: header instanceof HTMLElement ? header.className : null,
          titleValue: title instanceof HTMLInputElement ? title.value : null,
          typeBadge: typeBadge?.textContent?.trim() ?? null,
          descriptionValue:
            description instanceof HTMLTextAreaElement
              ? description.value
              : description instanceof HTMLElement
                ? description.textContent?.trim() ?? null
                : null,
          featuredLabels,
          hasCover: Boolean(document.querySelector(".typed-object-header__cover")),
        };
      })()`,
    );

    await execRenderer(
      client,
      `(() => {
        const settingsButton = document.querySelector('[data-testid="open-settings-btn"]');
        if (!(settingsButton instanceof HTMLElement)) {
          throw new Error("Settings button not found");
        }
        settingsButton.click();
        return true;
      })()`,
    );

    await waitForRenderer(
      client,
      `(() => Boolean(document.querySelector(".settings-page")))()`,
      "settings page",
    );

    await execRenderer(
      client,
      `(() => {
        const navButton = document.querySelector('[data-testid="settings-nav-object-types"]');
        if (!(navButton instanceof HTMLElement)) {
          throw new Error("Object types nav button not found");
        }
        navButton.click();
        return true;
      })()`,
    );

    await waitForRenderer(
      client,
      `(() => Boolean(document.querySelector(".object-types-layout")))()`,
      "object types layout",
    );

    await execRenderer(
      client,
      `(() => {
        const noteTypeButton = [...document.querySelectorAll(".object-types-item.builtin")]
          .find((element) => element.textContent?.includes("Заметка"));
        if (!(noteTypeButton instanceof HTMLElement)) {
          throw new Error("System note type button not found");
        }
        noteTypeButton.click();
        return true;
      })()`,
    );

    await waitForRenderer(
      client,
      `(() => Boolean(document.querySelector(".type-editor-chip")))()`,
      "system type editor",
    );

    results.settingsEditor = await execRenderer(
      client,
      `(() => ({
        builtinLabels: [...document.querySelectorAll(".object-types-item.builtin .object-types-item-name")]
          .map((element) => element.textContent?.trim())
          .filter(Boolean),
        hasLockChip: Boolean([...document.querySelectorAll(".type-editor-chip")]
          .find((element) => element.textContent?.includes("Контракт защищён кодом"))),
        hasPreview: Boolean(document.querySelector(".type-editor-preview .typed-object-header")),
      }))()`,
    );

    await execRenderer(
      client,
      `(() => {
        const group = [...document.querySelectorAll(".type-editor-input-group")]
          .find((element) => element.textContent?.includes("Макет header"));
        const select = group?.querySelector("select");
        if (!(select instanceof HTMLSelectElement)) {
          throw new Error("Header layout select not found");
        }
        select.value = "column";
        select.dispatchEvent(new Event("change", { bubbles: true }));
        const saveButton = document.querySelector(".type-editor-head .settings-btn-primary");
        if (!(saveButton instanceof HTMLElement)) {
          throw new Error("Type editor save button not found");
        }
        saveButton.click();
        return true;
      })()`,
    );

    await sleep(1200);

    results.builtInLayoutPersistence = await execRenderer(
      client,
      `(async () => {
        const noteTypes = await window.api.listNoteTypes();
        const noteType = noteTypes.find((item) => item.id === "note_obj") ?? null;
        const uiSchema = noteType?.ui_schema_json ? JSON.parse(noteType.ui_schema_json) : null;
        return {
          noteTypeFound: Boolean(noteType),
          headerLayout: uiSchema?.header_layout ?? null,
        };
      })()`,
    );

    await reloadWindow(client);
    await waitForRenderer(
      client,
      `(() => Boolean(document.querySelector(".widget-sidebar-wrapper")))()`,
      "sidebar after note_obj layout save",
    );

    await execRenderer(
      client,
      `(() => {
        const item = [...document.querySelectorAll(".widget-nav-item")]
          .find((element) => element.textContent?.includes("Linked Note"));
        if (!(item instanceof HTMLElement)) {
          throw new Error("Linked note list item not found");
        }
        item.click();
        return true;
      })()`,
    );

    await waitForRenderer(
      client,
      `(() => {
        const title = document.querySelector(".title-input");
        const header = document.querySelector(".typed-object-header");
        return title instanceof HTMLInputElement &&
          title.value === "Linked Note" &&
          header instanceof HTMLElement;
      })()`,
      "linked note page",
    );

    results.relationHydration = await execRenderer(
      client,
      `(async () => {
        const entry = await window.api.loadEntry(${JSON.stringify(linkedSourceId)});
        const parsedProps = entry?.header_props_json ? JSON.parse(entry.header_props_json) : null;
        const relationSelect = document.querySelector('[data-testid="typed-note-field-related_notes"]');
        const selectedRelated = relationSelect instanceof HTMLSelectElement
          ? Array.from(relationSelect.selectedOptions, (option) => ({ value: option.value, label: option.textContent?.trim() ?? "" }))
          : [];
        const header = document.querySelector(".typed-object-header");
        return {
          loadedRelatedNotes: Array.isArray(parsedProps?.related_notes) ? parsedProps.related_notes : [],
          selectedRelated,
          headerClass: header instanceof HTMLElement ? header.className : null,
        };
      })()`,
    );
  } finally {
    await client.close();
  }

  await fs.mkdir(path.dirname(resultsPath), { recursive: true });
  await fs.writeFile(resultsPath, JSON.stringify(results, null, 2));
  console.log(JSON.stringify(results, null, 2));
}

await main();

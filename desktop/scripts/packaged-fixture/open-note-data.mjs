// open-note scenario data builders, CDP helpers and the Memoria state dump.
import { delay } from "./lib.mjs";

export const AGENDA_ID = "com.kosmos.agenda";
export const MEMORIA_ID = "com.kosmos.memoria";
const NOTE_TYPE = "com.kosmos.note";
const NOTE_TYPE_VERSION = "1.0.0";
const TASK_TYPE = "com.kosmos.task";
const TASK_TYPE_VERSION = "1.0.0";
const LINK_TYPE = "related";

export function noteObject(id, title, now) {
  return {
    id,
    typeId: NOTE_TYPE,
    typeVersion: NOTE_TYPE_VERSION,
    title,
    contentJson: { type: "doc", content: [{ type: "paragraph" }] },
    propsJson: { description: null, extensions: {} },
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
  };
}

export function taskObject(id, title, now) {
  // Canonical com.kosmos.task@1.0.0 shape (host/e2e/first-party-agenda-contract).
  return {
    id,
    typeId: TASK_TYPE,
    typeVersion: TASK_TYPE_VERSION,
    title,
    contentJson: { type: "doc", content: [] },
    propsJson: {
      status: "todo",
      priority: "none",
      scheduledAt: null,
      dueAt: null,
      reminderAt: null,
      completedAt: null,
      canceledAt: null,
      recurrence: null,
      checklist: [],
      extensions: {},
    },
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
  };
}

export function taskNoteLink(taskId, noteId, now) {
  // agenda/src/lib/task-note-links.ts buildTaskNoteLink()
  return {
    id: `agenda-task-note:v1:${taskId.length}:${taskId}:${noteId.length}:${noteId}`,
    sourceObjectId: taskId,
    targetObjectId: noteId,
    linkType: LINK_TYPE,
    createdAt: now,
  };
}

export async function cdpPages(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
    signal: AbortSignal.timeout(2_000),
  });
  return response.json();
}

export const memoriaTitle = async (page) =>
  page.evaluate(
    () =>
      document.querySelector('[data-testid="titlebar-page-title"]')?.textContent?.trim() ?? null,
  );

// When a note-open check times out, dump what the packaged Memoria page
// actually sees — bridge results, DOM text and entry state — so a FAIL
// carries evidence instead of a bare timeout.
export async function dumpMemoriaState(page, noteId) {
  try {
    return await Promise.race([
      page.evaluate(async (id) => {
        const out = {};
        const api = globalThis.kosmosApp;
        out.identity = api?.identity ?? null;
        out.bodyText = document.body?.innerText?.replace(/\s+/g, " ").slice(0, 600) ?? null;
        try {
          out.getObject = await api.ark.request("get_object", { id });
        } catch (error) {
          out.getObject = `throw: ${error?.message ?? error}`;
        }
        try {
          out.listLinks = await api.ark.request("list_object_links", {});
        } catch (error) {
          out.listLinks = `throw: ${error?.message ?? error}`;
        }
        try {
          const entry = await globalThis.window.api?.loadEntry?.(id);
          out.loadEntry = entry ? { id: entry.id, title: entry.title } : (entry ?? null);
        } catch (error) {
          out.loadEntry = `throw: ${error?.message ?? error}`;
        }
        try {
          const app = document.querySelector("#root")?.__vue_app__;
          const provides = app?._context?.provides ?? {};
          const pinia = Object.getOwnPropertySymbols(provides)
            .map((symbol) => provides[symbol])
            .find((value) => value && value._s instanceof Map);
          const store =
            pinia &&
            [...pinia._s.values()].find((candidate) => candidate.navigateTo instanceof Function);
          out.store = store
            ? {
                activeScreen: store.activeScreen,
                currentEntryId: store.currentEntry?.id ?? null,
                currentEntryTitle: store.currentEntry?.title ?? null,
                loadingEntryId: store.loadingEntryId ?? null,
                entriesCount: store.entries?.length ?? null,
                hasTarget: store.entries?.some((e) => e.id === id) ?? null,
              }
            : "no store";
        } catch (error) {
          out.store = `throw: ${error?.message ?? error}`;
        }
        out.cmdProbe = globalThis.__kos58Cmds ?? "unregistered";
        const toastHost = document.querySelector('[aria-live="polite"]');
        const toasts = [...document.querySelectorAll('[role="status"]')].map((el) =>
          el.textContent.replace(/\s+/g, " ").trim().slice(0, 120),
        );
        out.toastHost = toastHost ? "mounted" : "absent";
        out.toasts = toasts;
        try {
          const app = document.querySelector("#root")?.__vue_app__;
          const seen = [];
          const walk = (instance, depth) => {
            if (!instance || depth > 4 || seen.length > 12) return;
            const name = instance.type?.name ?? instance.type?.__name ?? "anon";
            const symbols = Object.getOwnPropertySymbols(instance.provides ?? {})
              .map((s) => s.description ?? String(s))
              .filter((d) => !d.startsWith("v-") && d !== "__v_skip");
            seen.push(`${name}@${depth}:${symbols.join("|") || "-"}`);
            const child = instance.subTree?.component;
            if (child) walk(child, depth + 1);
          };
          walk(app?._instance, 0);
          out.providesTree = seen.join(" > ");
        } catch (error) {
          out.providesTree = `throw: ${error?.message ?? error}`;
        }
        return JSON.stringify(out).slice(0, 1600);
      }, noteId),
      delay(20_000).then(() => '"dump timeout>20s"'),
    ]);
  } catch (error) {
    return `dump failed: ${error?.message ?? error}`;
  }
}

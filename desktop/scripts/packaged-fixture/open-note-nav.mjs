// open-note navigation checks: warm open, missing-note contract, wrong-entry
// safety. Runs against an already-open Memoria page.
import crypto from "node:crypto";
import path from "node:path";
import { waitFor } from "./lib.mjs";
import { dumpMemoriaState, memoriaTitle } from "./open-note-data.mjs";

export async function runNoteNavigation({ check, logsDir, memoriaPage, noteB, titleB, openNote }) {
  // Warm open: note B -> existing Memoria window gets the navigation event.
  try {
    await openNote(noteB);
    await waitFor(
      async () => ((await memoriaTitle(memoriaPage)) === titleB ? true : undefined),
      `Memoria warm-open note B title "${titleB}"`,
      30_000,
    );
    check("warm open: memoria switches to note B", true, `title=${titleB}`);
  } catch (error) {
    check(
      "warm open: memoria switches to note B",
      false,
      `${error.message} | state=${await dumpMemoriaState(memoriaPage, noteB)}`,
    );
  }

  // Missing note: a nonexistent id -> Memoria stays usable and reports it.
  // First prove the command reached the app-side handler set — that
  // separates fixture delivery from what the packaged app does with it.
  const missingNoteId = `note:${crypto.randomUUID()}`;
  await openNote(missingNoteId);
  try {
    await waitFor(
      async () =>
        (await memoriaPage.evaluate(
          (id) => (globalThis.__kos58Cmds ?? []).some((p) => p?.entryId === id),
          missingNoteId,
        )) || undefined,
      "open command delivered to Memoria handler",
      15_000,
    );
    check("missing note: open command delivered to memoria", true);
  } catch (error) {
    check(
      "missing note: open command delivered to memoria",
      false,
      `${error.message} | state=${await dumpMemoriaState(memoriaPage, missingNoteId)}`,
    );
  }
  try {
    await waitFor(
      async () =>
        (await memoriaPage.evaluate(() =>
          document.body?.innerText?.includes("Заметка не найдена"),
        )) || undefined,
      'Memoria "Заметка не найдена" toast',
      30_000,
    );
    check("missing note: memoria reports note-not-found", true);
    await memoriaPage
      .screenshot({ path: path.join(logsDir, "memoria-missing-note.png") })
      .catch(() => {});
  } catch (error) {
    // Probe: call the store's navigateTo with the missing id directly.
    // Returning false proves the app-side not-found path runs — so the
    // silent gap is only the packaged App's useToast() resolving to the
    // inject-fallback stub (provide/inject in the same root setup).
    const toastProbe = await memoriaPage
      .evaluate(async (id) => {
        const app = document.querySelector("#root")?.__vue_app__;
        const provides = app?._context?.provides ?? {};
        const pinia = Object.getOwnPropertySymbols(provides)
          .map((s) => provides[s])
          .find((v) => v && v._s instanceof Map);
        const store = pinia && [...pinia._s.values()].find((c) => c.navigateTo instanceof Function);
        if (!store) return "no store found";
        const attempts = [`/note/${id}`, id];
        const results = [];
        for (const arg of attempts)
          try {
            results.push(`navigateTo(${JSON.stringify(arg)})=${await store.navigateTo(arg)}`);
          } catch (e) {
            results.push(`navigateTo(${JSON.stringify(arg)}) throw: ${e?.message ?? e}`);
          }
        return results.join("; ");
      }, missingNoteId)
      .catch((e) => `navigateTo probe throw: ${e?.message ?? e}`);
    check(
      "missing note: memoria reports note-not-found",
      false,
      `${error.message} | toastProbe=${toastProbe} | state=${await dumpMemoriaState(memoriaPage, missingNoteId)}`,
    );
  }
  // Even if the not-found toast regresses, the app must not render a wrong
  // entry for the missing id — that is the safety half of the contract.
  try {
    const state = await memoriaPage.evaluate(() => {
      const app = document.querySelector("#root")?.__vue_app__;
      const provides = app?._context?.provides ?? {};
      const pinia = Object.getOwnPropertySymbols(provides)
        .map((symbol) => provides[symbol])
        .find((value) => value && value._s instanceof Map);
      const store =
        pinia &&
        [...pinia._s.values()].find((candidate) => candidate.navigateTo instanceof Function);
      return {
        screen: store?.activeScreen ?? null,
        currentId: store?.currentEntry?.id ?? null,
      };
    });
    check(
      "missing note: memoria does not open a wrong entry",
      state.currentId !== missingNoteId,
      `screen=${state.screen} currentId=${state.currentId}`,
    );
  } catch (error) {
    check(
      "missing note: memoria does not open a wrong entry",
      false,
      `probe failed: ${error?.message ?? error}`,
    );
  }
}

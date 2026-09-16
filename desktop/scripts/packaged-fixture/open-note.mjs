// open-note scenario (KOS-30 packaged path): packaged Host + Agenda deep-links
// notes into packaged Memoria — cold open, warm open, missing note.
import fs from "node:fs";
import path from "node:path";
import { processInfo } from "../dev-run-process.mjs";
import {
  DEFAULT_OUT,
  delay,
  engineRpc,
  ensureDir,
  fixtureEnv,
  freeTcpPort,
  iso,
  loadFixture,
  makeCheck,
  scenarioNotRun,
  spawnLogged,
  waitFor,
  waitForEngineLock,
  writeJson,
} from "./lib.mjs";
import { AGENDA_ID, dumpMemoriaState, MEMORIA_ID, memoriaTitle } from "./open-note-data.mjs";
import { runNoteNavigation } from "./open-note-nav.mjs";
import { findPage, installPackages, makeCleanup, seedObjects } from "./open-note-steps.mjs";

export async function cmdOpenNote(args) {
  const outDir = path.resolve(args.out ?? DEFAULT_OUT);
  let fixture;
  try {
    fixture = loadFixture(outDir);
  } catch (error) {
    scenarioNotRun(outDir, "open-note", error.message);
    return;
  }
  const runDir = path.join(outDir, "runs", "open-note");
  const dataDir = path.join(runDir, "data");
  const logsDir = path.join(runDir, "logs");
  fs.rmSync(runDir, { recursive: true, force: true });
  ensureDir(dataDir);
  ensureDir(logsDir);

  const result = {
    scenario: "open-note",
    started_at: iso(),
    command: `node desktop/scripts/packaged-fixture.mjs open-note --out ${outDir}`,
    status: "FAIL",
    checks: [],
    logs: {},
    cleanup: {},
  };
  const check = makeCheck(result);
  const record = (name, fn) => {
    try {
      return check(name, true, fn());
    } catch (error) {
      return check(name, false, error.message);
    }
  };

  const hostExe = fixture.artifacts.host_exe.path;
  const engineBackend = fixture.engine.backend;
  const processRoots = [runDir, fixture.engine.root];
  const spawned = []; // {pid, identity, label}
  const trackSpawn = (child, label) => {
    if (!child?.pid) return;
    const identity = processInfo(child.pid);
    if (identity) spawned.push({ pid: child.pid, identity, label });
  };
  const ctx = { engine: null, host: null, browser: null };
  const cleanup = makeCleanup({
    ctx,
    result,
    check,
    spawned,
    engineBackend,
    dataDir,
    processRoots,
  });

  try {
    // 1. Seed the signed production catalog into the isolated Engine data dir.
    //    PackageService persists catalogs at <dataDir>/packages/catalog.json.
    ensureDir(path.join(dataDir, "packages"));
    fs.copyFileSync(fixture.artifacts.catalog.path, path.join(dataDir, "packages", "catalog.json"));

    // 2. Start the packaged Engine (supervisor mode, like the Desktop does).
    ctx.engine = spawnLogged(engineBackend, ["--start"], {
      env: fixtureEnv({
        KOSMOS_DATA_DIR: dataDir,
        ARK_CORE_RPC_PATH: fixture.engine.ark,
        KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        KOSMOS_TEST_MODE: "1",
        KOSMOS_HEADLESS: "1",
        KEPLER_SKIP_SYNC: "1",
        KEPLER_USAGE_TRACKER: "0",
        KEPLER_INSTANCE: "kos58",
      }),
      logFile: path.join(logsDir, "engine.log"),
    });
    trackSpawn(ctx.engine.child, "engine");
    const lock = await waitForEngineLock(dataDir);
    result.logs.engine = path.join(logsDir, "engine.log");
    check("engine lock acquired", true, `pid=${lock.pid} http_port=${lock.http_port}`);

    // 3. Verify package trust + seeded catalog were accepted on startup.
    //    trust_status shape: data.catalog = CatalogSummary|null, data.trust =
    //    TrustSummary (catalog_sequence/fault_code).
    const trust = await engineRpc(lock, "packages.trust_status");
    record("packages.trust_status ok", () => {
      if (!trust.ok) throw new Error(JSON.stringify(trust.error));
      const catalogSeq = trust.data?.catalog?.sequence ?? trust.data?.trust?.catalog_sequence;
      if (catalogSeq !== fixture.catalog.sequence)
        throw new Error(
          `catalog sequence ${JSON.stringify(catalogSeq)} != ${fixture.catalog.sequence}` +
            ` (fault_code=${trust.data?.trust?.fault_code ?? "null"};` +
            ` engine catalog_compatible=${fixture.engine.catalog_compatible})`,
        );
      return `sequence=${catalogSeq}`;
    });

    // 4. Install + enable the pinned apps, then seed objects.
    await installPackages(lock, fixture, record);
    const { noteA, noteB, titleA, titleB } = await seedObjects(lock, record);

    // 5. Launch the packaged Host with Agenda on an isolated user-data dir.
    const cdpPort = await freeTcpPort();
    const hostUserData = path.join(runDir, "host-user-data");
    ctx.host = spawnLogged(
      hostExe,
      [
        `--open-app=${AGENDA_ID}`,
        `--user-data-dir=${hostUserData}`,
        `--remote-debugging-port=${cdpPort}`,
      ],
      {
        env: fixtureEnv({
          KOSMOS_DATA_DIR: dataDir,
          KOSMOS_TEST_MODE: "1",
          KOSMOS_HEADLESS: "1",
          APPDATA: path.join(runDir, "appdata"),
          LOCALAPPDATA: path.join(runDir, "localappdata"),
          ELECTRON_ENABLE_LOGGING: "1",
        }),
        logFile: path.join(logsDir, "host.log"),
      },
    );
    trackSpawn(ctx.host.child, "host");
    result.logs.host = path.join(logsDir, "host.log");

    const { chromium } = await import("playwright");
    ctx.browser = await chromium.connectOverCDP(`http://127.0.0.1:${cdpPort}`);
    const findAppPage = findPage(ctx.browser);
    const agendaPage = await waitFor(
      () => findAppPage(AGENDA_ID),
      "Agenda app window (CDP page)",
      60_000,
    );
    check("agenda window opened", true, agendaPage.url());
    await waitFor(() => agendaPage.url().includes("index.html") || undefined, "Agenda launch_url");
    await waitFor(async () => {
      try {
        return (
          (await agendaPage.evaluate(() =>
            Boolean(globalThis.kosmosApp?.ark?.request && globalThis.kosmosApp?.apps?.open),
          )) || undefined
        );
      } catch {
        return undefined;
      }
    }, "kosmosApp bridge in Agenda");

    // 6. Deep-link: Agenda -> apps.open(Memoria, /note/<id>) through the
    //    packaged Host bridge (the KOS-30 contract).
    const openNote = (noteId) =>
      agendaPage.evaluate(async (id) => {
        const response = await globalThis.kosmosApp.apps.open({
          id: "com.kosmos.memoria",
          route: `/note/${encodeURIComponent(id)}`,
        });
        if (!response || response.ok !== true)
          throw new Error(`apps.open rejected: ${JSON.stringify(response)}`);
        return true;
      }, noteId);

    // Cold open: note A -> Memoria launches on /note/<id>.
    let memoriaPage = null;
    try {
      await openNote(noteA);
      memoriaPage = await waitFor(
        () => findAppPage(MEMORIA_ID),
        "Memoria app window (CDP page)",
        60_000,
      );
      check("memoria window opened", true, memoriaPage.url());
      // Diagnostic: the launch-scoped ARK bridge must resolve get_object for
      // the seeded note; a failure here explains why navigateTo shows nothing.
      const probe = await Promise.race([
        memoriaPage.evaluate(async (id) => {
          try {
            return JSON.stringify(
              await globalThis.kosmosApp.ark.request("get_object", { id }),
            ).slice(0, 500);
          } catch (error) {
            return `throw: ${error?.message ?? error}`;
          }
        }, noteA),
        delay(15_000).then(() => "timeout>15s"),
      ]);
      check("memoria launch-scoped ark get_object noteA", probe.includes('"ok":true'), probe);
      // Probe listener: proves whether dispatched note-open commands reach
      // the app-side handler set (independent of App.vue's own subscription).
      await memoriaPage
        .evaluate(() => {
          globalThis.__kos58Cmds = [];
          globalThis.window.api?.onCommand?.("eden:cmd:note:open", (params) =>
            globalThis.__kos58Cmds.push(params),
          );
        })
        .catch(() => {});
      await waitFor(
        async () => ((await memoriaTitle(memoriaPage)) === titleA ? true : undefined),
        `Memoria cold-open note A title "${titleA}"`,
        30_000,
      );
      check("cold open: memoria shows seeded note A", true, `title=${titleA}`);
      await memoriaPage
        .screenshot({ path: path.join(logsDir, "memoria-note-a.png") })
        .catch(() => {});
    } catch (error) {
      check(
        "cold open: memoria shows seeded note A",
        false,
        `${error.message} | state=${await dumpMemoriaState(memoriaPage, noteA)}`,
      );
    }

    if (memoriaPage) {
      await runNoteNavigation({
        check,
        logsDir,
        memoriaPage,
        noteB,
        titleB,
        openNote,
      });
    }
  } catch (error) {
    check("scenario error", false, error.message);
  } finally {
    await cleanup();
    result.status = result.checks.every((item) => item.status === "pass") ? "PASS" : "FAIL";
    result.finished_at = iso();
    writeJson(path.join(runDir, "result.json"), result);
  }
  console.log(JSON.stringify({ scenario: "open-note", status: result.status, runDir }));
  if (result.status !== "PASS") process.exitCode = 1;
}

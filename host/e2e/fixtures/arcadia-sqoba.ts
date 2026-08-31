import fs from "node:fs";
import path from "node:path";
import { expect, type Page } from "@playwright/test";

type RecoveryFixture = {
  gameId: string;
  journal: string;
  saveFile: string;
};

export function createSqobaRoot(root: string) {
  const saveRoot = path.join(root, "selected-saves");
  fs.mkdirSync(saveRoot, { recursive: true });
  fs.writeFileSync(path.join(saveRoot, "a-first.dat"), "backup-a");
  fs.writeFileSync(path.join(saveRoot, "z-last.dat"), "backup-z");
  return saveRoot;
}

export async function exerciseSqoba(
  page: Page,
  dataDir: string,
  fakeExe: string,
  saveRoot: string,
): Promise<RecoveryFixture> {
  const prepared = await page.evaluate(async (exePath) => {
    const grant = await window.kosmosApp.dialogs.pickDirectoryGrant();
    if (!grant) throw new Error("directory grant was not selected");
    const added = await window.kosmosApp.ark!.request("games.add_manual", {
      name: "Arcadia SQOBA E2E",
      exe_path: exePath,
      save_roots: [grant],
    });
    // SAFETY: the signed Arcadia operation has the response contract asserted by this E2E.
    const gameId = (added as { data: { id: string } }).data.id;
    const backup = await window.kosmosApp.ark!.request("games.sqoba.backup", { game_id: gameId });
    // SAFETY: the signed Arcadia operation has the response contract asserted by this E2E.
    const backupId = (backup as { data: { id: string } }).data.id;
    return { gameId, backupId, grant };
  }, fakeExe);

  const first = path.join(saveRoot, "a-first.dat");
  const blocked = path.join(saveRoot, "z-last.dat");
  fs.writeFileSync(first, "live-a");
  fs.rmSync(blocked);
  fs.mkdirSync(blocked);
  const failed = await page.evaluate(
    ({ gameId, backupId }) =>
      window.kosmosApp.ark!.request("games.sqoba.restore", {
        game_id: gameId,
        backup_id: backupId,
      }),
    prepared,
  );
  expect(failed, JSON.stringify(failed)).toMatchObject({
    ok: true,
    data: { ok: false, status: "rolled_back", restored_files: 1 },
  });
  expect(fs.readFileSync(first, "utf8")).toBe("live-a");

  fs.rmSync(blocked, { recursive: true });
  fs.writeFileSync(blocked, "live-z");
  const restored = await page.evaluate(
    ({ gameId, backupId }) =>
      window.kosmosApp.ark!.request("games.sqoba.restore", {
        game_id: gameId,
        backup_id: backupId,
      }),
    prepared,
  );
  expect(restored, JSON.stringify(restored)).toMatchObject({
    ok: true,
    data: { ok: true, status: "committed", restored_files: 2 },
  });
  expect(fs.readFileSync(first, "utf8")).toBe("backup-a");
  expect(fs.readFileSync(blocked, "utf8")).toBe("backup-z");

  fs.writeFileSync(first, "before-crash");
  const journal = path.join(
    dataDir,
    "packages",
    "package-state",
    "com.kosmos.arcadia",
    "arrancador-backups",
    prepared.gameId,
    `${prepared.backupId}.journal.json`,
  );
  fs.writeFileSync(
    journal,
    JSON.stringify({
      game_id: prepared.gameId,
      backup_id: prepared.backupId,
      root_grant_ids: [prepared.grant.persistentGrantId],
      committed: false,
      entries: [
        {
          root_index: 0,
          relative_path: "a-first.dat",
          original: Buffer.from("before-crash").toString("base64"),
        },
      ],
    }),
  );
  fs.writeFileSync(first, "partial-after-crash");
  return { gameId: prepared.gameId, journal, saveFile: first };
}

export async function expectSqobaRecovered(page: Page, fixture: RecoveryFixture) {
  const listed = await page.evaluate(
    (gameId) => window.kosmosApp.ark!.request("games.sqoba.list", { game_id: gameId }),
    fixture.gameId,
  );
  expect(listed, JSON.stringify(listed)).toMatchObject({ ok: true });
  expect(fs.readFileSync(fixture.saveFile, "utf8")).toBe("before-crash");
  expect(fs.existsSync(fixture.journal)).toBe(false);
}

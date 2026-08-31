import fs from "node:fs";
import path from "node:path";
import { expect, type Page } from "@playwright/test";

const HOSTILE_GAME = "Arcadia hostile Steam E2E";

export function createArcadiaFixtures(dataDir: string) {
  const fakeExe = path.join(
    dataDir,
    "packages",
    "package-state",
    "com.kosmos.arcadia",
    "ArcadiaHostE2E.exe",
  );
  const root = path.join(dataDir, "steam-hostile");
  const installDir = path.join(root, "steamapps", "common", "Hostile");
  const marker = path.join(installDir, "arcadia-steam-injected.txt");
  const appId = `123&echo owned>"${marker.replaceAll("\\", "/")}"`;
  const manifestAppId = appId.replaceAll('"', '\\"');
  fs.mkdirSync(path.dirname(fakeExe), { recursive: true });
  fs.mkdirSync(installDir, { recursive: true });
  fs.writeFileSync(fakeExe, Buffer.from("MZ ArcadiaHostE2E"));
  fs.writeFileSync(path.join(root, "steam.exe"), Buffer.from("MZ ArcadiaSteamHostE2E"));
  fs.writeFileSync(
    path.join(root, "steamapps", "appmanifest_123.acf"),
    `"AppState" { "appid" "${manifestAppId}" "name" "${HOSTILE_GAME}" "installdir" "Hostile" "SizeOnDisk" "1" }`,
  );
  return { fakeExe, hostileSteam: { root, marker, appId } };
}

export async function expectHostileSteamRejected(
  page: Page,
  fixture: ReturnType<typeof createArcadiaFixtures>["hostileSteam"],
) {
  const result = await page.evaluate(
    async ({ root, title }) => {
      const scan = await window.kosmosApp.ark.request("games.scan", {
        steam_library_override: root,
      });
      const listed = await window.kosmosApp.ark.request("games.list", {});
      // SAFETY: the pinned Arcadia worker's games.list response shape is asserted below.
      const games = (listed as { data?: Array<{ id?: string; title?: string }> }).data ?? [];
      const game = games.find((candidate) => candidate.title === title);
      const launch = game?.id
        ? await window.kosmosApp.ark.request("games.launch", { game_id: game.id })
        : { ok: false, message: "missing-hostile-game" };
      const after = await window.kosmosApp.ark.request("games.list", {});
      return { scan, game, launch, after };
    },
    { root: fixture.root, title: HOSTILE_GAME },
  );
  expect(result.scan, JSON.stringify(result.scan)).toMatchObject({
    ok: true,
    data: { added: 0, discovered: 0, skipped: 0 },
  });
  expect(result.game).toBeUndefined();
  expect(result.launch).toEqual({ ok: false, message: "missing-hostile-game" });
  expect(result.after).toMatchObject({ ok: true, data: [] });
  expect(fs.existsSync(fixture.marker)).toBe(false);
}

// Universal extension contract spec — manifest-driven.
//
// Phase 6.0.5: вместо повторения per-app boilerplate (open + sanity check),
// каждый extension'а manifest.json объявляет минимальную «test contract»:
//   tests: {
//     commands?: string[],            // ожидаемые команды после boot
//     smoke?: {
//       objectType: string,           // ARK type для round-trip
//       sample?: { title?, content?, props? }
//     }
//   }
//
// Этот spec на boot:
//   1) discover'ит все `extensions/<id>/manifest.json`
//   2) для каждого с `tests` блоком — генерирует test.describe
//   3) каждый describe: открыть extension через command bus, дождаться
//      окна, проверить commands.list + ARK round-trip
//
// Per-app UI flow'ы (TipTap рендерится, Pomodoro переключается и т.п.) —
// отдельные spec'и (`eden.spec.ts`, `horologion.spec.ts`, …).

import { test, expect, type Page } from "@playwright/test";
import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { launchKepler, REPO_ROOT } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

interface ExtensionTestContract {
  commands?: string[];
  smoke?: {
    objectType: string;
    sample?: { title?: string; content?: unknown; props?: Record<string, unknown> };
  };
}

interface ExtensionManifest {
  id: string;
  name: string;
  tests?: ExtensionTestContract;
}

function discoverExtensionsWithTests(): ExtensionManifest[] {
  const root = path.join(REPO_ROOT, "extensions");
  if (!fs.existsSync(root)) return [];
  const result: ExtensionManifest[] = [];
  for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const manifestPath = path.join(root, entry.name, "manifest.json");
    if (!fs.existsSync(manifestPath)) continue;
    try {
      const raw = fs.readFileSync(manifestPath, "utf-8");
      const manifest = JSON.parse(raw) as ExtensionManifest;
      if (manifest.tests) result.push(manifest);
    } catch (e) {
      console.warn(`[contract-spec] skip ${manifestPath}:`, e);
    }
  }
  return result;
}

const manifests = discoverExtensionsWithTests();

for (const manifest of manifests) {
  test.describe(`extension contract: ${manifest.id}`, () => {
    test(`${manifest.id}: загружается + commands + ARK round-trip`, async () => {
      const app = await launchKepler({ slug: `contract-${manifest.id}` });
      try {
        const launcher = await app.firstWindow();
        await launcher.waitForLoadState("domcontentloaded");

        // Phase 1 determinism: ждём ArkClient handshake вместо sleep.
        await waitForBackendReady(launcher);

        // Триггерим открытие extension'а через command bus.
        const openCommand = `${manifest.id}:open`;
        const triggered = await app.evaluate(async ({ BrowserWindow }, cmd) => {
          const wins = BrowserWindow.getAllWindows();
          const win = wins[0];
          if (!win) return false;
          try {
            await win.webContents.executeJavaScript(
              `window.kepler?.commands?.invoke?.(${JSON.stringify(cmd)})`,
            );
            return true;
          } catch {
            return false;
          }
        }, openCommand);
        expect(triggered, `command ${openCommand} должен invoke'аться`).toBe(true);

        const extWindow = await app.waitForEvent("window", { timeout: 10_000 });
        await extWindow.waitForLoadState("domcontentloaded");

        // 1.5s — достаточно для shim install + initial render + commands.register
        // успело отработать в extension'е (см. main.ts по образцу Horologion).
        await extWindow.waitForTimeout(1500);

        // Sanity: extension window жив, не destroyed.
        expect(
          await extWindow.evaluate(() => document.readyState),
          "extension window живой DOM",
        ).toBe("complete");

        const contract = manifest.tests!;

        // -- commands check --
        if (contract.commands && contract.commands.length > 0) {
          const registered = await extWindow.evaluate(async () => {
            const kepler = (
              window as unknown as {
                kepler?: {
                  ark: {
                    request: <T = unknown>(op: string, params?: unknown) => Promise<T>;
                  };
                };
              }
            ).kepler;
            if (!kepler) return [];
            try {
              const res = await kepler.ark.request<
                { commands?: Array<{ id: string }> } | Array<{ id: string }>
              >("commands.list", {});
              if (Array.isArray(res)) return res.map((c) => c.id);
              if (res && "commands" in res && Array.isArray(res.commands)) {
                return res.commands.map((c) => c.id);
              }
              return [];
            } catch (e) {
              console.warn("[contract-spec] commands.list failed:", e);
              return [];
            }
          });

          const missing = contract.commands.filter((c) => !registered.includes(c));
          expect(
            missing,
            `extension ${manifest.id}: команды ${JSON.stringify(missing)} не появились в commands.list. ` +
              `Зарегистрированы: ${JSON.stringify(registered)}`,
          ).toEqual([]);
        }

        // -- ARK smoke round-trip --
        if (contract.smoke) {
          const smoke = contract.smoke;
          const sampleId = `contract-smoke-${manifest.id}-${Date.now()}`;
          const sampleTitle = smoke.sample?.title ?? `contract-smoke-${manifest.id}`;
          const nowIso = new Date().toISOString();

          const result = await extWindow.evaluate(
            async (payload) => {
              const kepler = (
                window as unknown as {
                  kepler?: {
                    ark: {
                      request: <T = unknown>(op: string, params?: unknown) => Promise<T>;
                    };
                  };
                }
              ).kepler;
              if (!kepler) return { stage: "no-kepler", error: "no kepler bridge" };

              try {
                await kepler.ark.request<boolean>("upsert_object", {
                  object: {
                    id: payload.id,
                    typeId: payload.typeId,
                    title: payload.title,
                    contentJson: payload.content,
                    propsJson: payload.props,
                    createdAt: payload.ts,
                    updatedAt: payload.ts,
                    deletedAt: null,
                  },
                });
              } catch (e) {
                return {
                  stage: "upsert",
                  error: e instanceof Error ? e.message : String(e),
                };
              }

              let got: { id?: string; title?: string; typeId?: string } | null = null;
              try {
                got = await kepler.ark.request<typeof got>("get_object", { id: payload.id });
              } catch (e) {
                return {
                  stage: "get",
                  error: e instanceof Error ? e.message : String(e),
                };
              }

              try {
                await kepler.ark.request<boolean>("delete_object", { id: payload.id });
              } catch (e) {
                return {
                  stage: "cleanup",
                  error: e instanceof Error ? e.message : String(e),
                  got,
                };
              }

              return { stage: "ok", got };
            },
            {
              id: sampleId,
              typeId: smoke.objectType,
              title: sampleTitle,
              content: smoke.sample?.content ?? { type: "doc", content: [{ type: "paragraph" }] },
              props: smoke.sample?.props ?? {},
              ts: nowIso,
            },
          );

          if (result.stage !== "ok") {
            // Если type ещё не зарегистрирован (extension его lazy-init'ит при
            // первом write своих real-объектов) — не валим контракт. Smoke
            // объявленный в manifest должен быть валидируемым; если он
            // зависит от ленивого ensure-type — манифест должен это
            // декларировать или smoke перенесён в per-app spec.
            throw new Error(
              `extension ${manifest.id} smoke failed at stage=${result.stage}: ${result.error}`,
            );
          }

          expect(result.got, `get_object вернул object`).toBeTruthy();
          expect(result.got?.id).toBe(sampleId);
          expect(result.got?.typeId).toBe(smoke.objectType);
          expect(result.got?.title).toBe(sampleTitle);
        }
      } finally {
        await app.close();
      }
    });
  });
}

if (manifests.length === 0) {
  test("extension contract: no manifests with `tests` block", async () => {
    test.skip(
      true,
      "Ни один extension не объявляет `tests` в manifest.json — universal contract нечего проверять.",
    );
  });
}

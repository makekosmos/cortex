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
//   1) discover'ит все source manifests under `products/`, `incubator/`, `extensions/`
//   2) для каждого с `tests` блоком — генерирует test.describe
//   3) каждый describe: открыть extension через command bus, дождаться
//      окна, проверить commands.list + ARK round-trip
//
// Per-app UI flow'ы (TipTap рендерится и т.п.) —
// отдельные spec'и (`eden.spec.ts`, …).

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
  kind?: "vue" | "static" | "native";
  tests?: ExtensionTestContract;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

function isExtensionTestContract(value: unknown): value is ExtensionTestContract {
  if (!isRecord(value)) return false;
  if (value.commands !== undefined && !isStringArray(value.commands)) return false;

  const smoke = value.smoke;
  if (smoke === undefined) return true;
  if (!isRecord(smoke) || typeof smoke.objectType !== "string") return false;
  if (smoke.sample !== undefined && !isRecord(smoke.sample)) return false;
  return true;
}

function parseExtensionManifest(raw: string): ExtensionManifest | null {
  const parsed: unknown = JSON.parse(raw);
  if (!isRecord(parsed)) return null;
  if (typeof parsed.id !== "string" || typeof parsed.name !== "string") return null;
  if (parsed.tests !== undefined && !isExtensionTestContract(parsed.tests)) return null;
  return {
    id: parsed.id,
    name: parsed.name,
    kind:
      parsed.kind === "vue" || parsed.kind === "static" || parsed.kind === "native"
        ? parsed.kind
        : undefined,
    tests: parsed.tests,
  };
}

function discoverExtensionsWithTests(): ExtensionManifest[] {
  const result: ExtensionManifest[] = [];
  const seen = new Set<string>();
  for (const rootName of ["products", "incubator", "extensions"]) {
    const root = path.join(REPO_ROOT, rootName);
    if (!fs.existsSync(root)) continue;
    for (const entry of fs.readdirSync(root, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const manifestPath = path.join(root, entry.name, "manifest.json");
      if (!fs.existsSync(manifestPath)) continue;
      try {
        const raw = fs.readFileSync(manifestPath, "utf-8");
        const manifest = parseExtensionManifest(raw);
        if (!manifest) continue;
        if (!manifest.tests || seen.has(manifest.id)) continue;
        seen.add(manifest.id);
        result.push(manifest);
      } catch (e) {
        console.warn(`[contract-spec] skip ${manifestPath}:`, e);
      }
    }
  }
  return result;
}

const manifests = discoverExtensionsWithTests();

async function registeredCommandIds(extWindow: Page): Promise<string[]> {
  return extWindow.evaluate(async () => {
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
}

async function expectContractCommands(
  extWindow: Page,
  manifest: ExtensionManifest,
  commands: string[] | undefined,
): Promise<void> {
  if (!commands?.length) return;
  const registered = await registeredCommandIds(extWindow);
  const missing = commands.filter((c) => !registered.includes(c));
  expect(
    missing,
    `extension ${manifest.id}: команды ${JSON.stringify(missing)} не появились в commands.list. ` +
      `Зарегистрированы: ${JSON.stringify(registered)}`,
  ).toEqual([]);
}

type SmokeObject = { id?: string; title?: string; typeId?: string };
type SmokeResult =
  | { stage: "ok"; got: SmokeObject | null }
  | { stage: "no-kepler" | "upsert" | "get" | "cleanup"; error: string; got?: SmokeObject | null };

interface SmokePayload {
  id: string;
  typeId: string;
  title: string;
  content: unknown;
  props: Record<string, unknown>;
  ts: string;
}

async function runSmokeRoundTrip(extWindow: Page, payload: SmokePayload): Promise<SmokeResult> {
  return extWindow.evaluate(async (payload): Promise<SmokeResult> => {
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

    let got: SmokeObject | null = null;
    try {
      got = await kepler.ark.request<SmokeObject | null>("get_object", { id: payload.id });
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
  }, payload);
}

async function expectSmokeRoundTrip(
  extWindow: Page,
  manifest: ExtensionManifest,
  smoke: NonNullable<ExtensionTestContract["smoke"]>,
): Promise<void> {
  const sampleId = `contract-smoke-${manifest.id}-${Date.now()}`;
  const sampleTitle = smoke.sample?.title ?? `contract-smoke-${manifest.id}`;
  const result = await runSmokeRoundTrip(extWindow, {
    id: sampleId,
    typeId: smoke.objectType,
    title: sampleTitle,
    content: smoke.sample?.content ?? { type: "doc", content: [{ type: "paragraph" }] },
    props: smoke.sample?.props ?? {},
    ts: new Date().toISOString(),
  });

  if (result.stage !== "ok") {
    throw new Error(
      `extension ${manifest.id} smoke failed at stage=${result.stage}: ${result.error}`,
    );
  }

  expect(result.got, `get_object вернул object`).toMatchObject({
    id: sampleId,
    typeId: smoke.objectType,
    title: sampleTitle,
  });
}

test("extension contract discovery finds manifests with tests", () => {
  expect(manifests.length).toBeGreaterThan(0);
});

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

        if (manifest.kind === "native") {
          const commands = await launcher.evaluate(async () => {
            return (await window.kepler.commands.list()).map((command) => command.id);
          });
          expect(
            commands,
            `native extension ${manifest.id}: command ${openCommand} виден в registry`,
          ).toContain(openCommand);
          return;
        }

        const extWindow = await app.waitForEvent("window", { timeout: 10_000 });
        await extWindow.waitForLoadState("domcontentloaded");

        // 1.5s — достаточно для shim install + initial render + commands.register
        // в extension'е.
        await extWindow.waitForTimeout(1500);

        // Sanity: extension window жив, не destroyed.
        expect(
          await extWindow.evaluate(() => document.readyState),
          "extension window живой DOM",
        ).toBe("complete");

        const contract = manifest.tests!;

        await expectContractCommands(extWindow, manifest, contract.commands);

        if (contract.smoke) {
          await expectSmokeRoundTrip(extWindow, manifest, contract.smoke);
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

// Autorun toggle IPC (Фича A): `kepler:settings:autostart:get/set/allowed`.
//
// В test slot'е `resolveInstance().autorunEnabled` = false (kind === "test",
// см. instance.ts:158). Это значит:
//   - allowed() → false
//   - set(true) — silent no-op, НЕ должен дёрнуть app.setLoginItemSettings
//     (что бы поломало пользовательский HKCU\...\Run).
//   - get() → возвращает текущее app.getLoginItemSettings().openAtLogin
//     (в test это false по умолчанию).
//
// Edge: запуск с `--autostart` argv не должен ронять main process.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { getLauncherWindow } from "./helpers/launcher";

test.describe("kepler-shell autorun (test slot)", () => {
  test("autostart.allowed() === false в test slot'е", async () => {
    const app = await launchKepler({ slug: "autorun-allowed-false" });
    try {
      const launcher = await getLauncherWindow(app);
      const allowed = await launcher.evaluate(async () => {
        const w = window as unknown as {
          kepler?: {
            settings?: { autostart?: { allowed?: () => Promise<boolean> } };
          };
        };
        return await w.kepler?.settings?.autostart?.allowed?.();
      });
      expect(allowed).toBe(false);
    } finally {
      await app.close();
    }
  });

  test("autostart.get() возвращает boolean (default false)", async () => {
    const app = await launchKepler({ slug: "autorun-get-default" });
    try {
      const launcher = await getLauncherWindow(app);
      const got = await launcher.evaluate(async () => {
        const w = window as unknown as {
          kepler?: {
            settings?: { autostart?: { get?: () => Promise<boolean> } };
          };
        };
        return await w.kepler?.settings?.autostart?.get?.();
      });
      expect(typeof got).toBe("boolean");
      expect(got).toBe(false);
    } finally {
      await app.close();
    }
  });

  test("autostart.set(true) не падает в test slot (silent no-op)", async () => {
    const app = await launchKepler({ slug: "autorun-set-noop" });
    try {
      const launcher = await getLauncherWindow(app);
      const result = await launcher.evaluate(async () => {
        const w = window as unknown as {
          kepler?: {
            settings?: {
              autostart?: {
                set?: (v: boolean) => Promise<void>;
                get?: () => Promise<boolean>;
              };
            };
          };
        };
        try {
          await w.kepler?.settings?.autostart?.set?.(true);
          const after = await w.kepler?.settings?.autostart?.get?.();
          return { ok: true, after };
        } catch (e) {
          return { ok: false, error: String((e as Error)?.message ?? e) };
        }
      });
      expect(result.ok).toBe(true);
      // get() остаётся false — set был no-op'ом (slot не prod).
      expect(result.after).toBe(false);
    } finally {
      await app.close();
    }
  });

  test("preload экспонирует autostart bridge (get/set/allowed)", async () => {
    const app = await launchKepler({ slug: "autorun-bridge-shape" });
    try {
      const launcher = await getLauncherWindow(app);
      const shape = await launcher.evaluate(() => {
        const w = window as unknown as {
          kepler?: { settings?: { autostart?: Record<string, unknown> } };
        };
        const a = w.kepler?.settings?.autostart;
        return {
          hasGet: typeof a?.get === "function",
          hasSet: typeof a?.set === "function",
          hasAllowed: typeof a?.allowed === "function",
        };
      });
      expect(shape).toEqual({
        hasGet: true,
        hasSet: true,
        hasAllowed: true,
      });
    } finally {
      await app.close();
    }
  });

  test("запуск с --autostart argv не роняет main process", async () => {
    // Симулируем autorun-launch — в test slot --autostart должен быть просто
    // принят, без падения; launcher остаётся hidden.
    const app = await launchKepler({
      slug: "autorun-argv-survives",
      env: { KEPLER_TEST_AUTOSTART_ARGV: "1" },
    });
    try {
      const launcher = await getLauncherWindow(app);
      // Дёрнем что-нибудь через bridge — убедимся что main process жив.
      const name = await app.evaluate(({ app: a }) => a.getName());
      expect(name).toBeTruthy();
      const allowed = await launcher.evaluate(async () => {
        const w = window as unknown as {
          kepler?: {
            settings?: { autostart?: { allowed?: () => Promise<boolean> } };
          };
        };
        return await w.kepler?.settings?.autostart?.allowed?.();
      });
      expect(allowed).toBe(false);
    } finally {
      await app.close();
    }
  });
});

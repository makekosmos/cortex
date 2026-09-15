import { expect, mock, test } from "../test-support/node-test.mjs";
import type { FocusNativeAdapter, FocusRequest } from "./focus-enforcement";

mock.module("electron", () => ({
  app: { getAppPath: () => process.cwd(), isPackaged: false },
  BrowserWindow: { getAllWindows: () => [] },
}));
mock.module("../../shared/focus-service-client", () => ({
  pingService: async () => false,
  sendViaPipe: async () => null,
}));

const { applyAuthoritativeFocusState, reconcileFocusState } = await import("./focus-enforcement");

const inactive = {
  active: false,
  blocklist_id: null,
  blocked_app_ids: [],
  blocked_apps: [],
};
const ok = (domains: string[]) => ({ ok: true, active_domains: domains });

for (const failure of ["response", "rejection", "extra-domains"] as const) {
  test(`native ${failure} is surfaced before focus state persistence`, async () => {
    const requests: string[] = [];
    const request: FocusRequest = async (operation) => {
      requests.push(operation);
      if (operation === "focus.get_active_state") return inactive;
      throw new Error(`unexpected persisted operation: ${operation}`);
    };
    let call = 0;
    const native: FocusNativeAdapter = {
      status: async () => ok([]),
      apply: async ({ active }) => {
        call += 1;
        if (active) {
          if (failure === "response") return { ok: false, error: "uac-cancelled" };
          if (failure === "rejection") throw new Error("helper-disconnected");
          return ok(["example.com", "stale.example"]);
        }
        return ok([]);
      },
    };

    await expect(
      applyAuthoritativeFocusState({
        request,
        active: true,
        blocklistId: "work",
        resolveDomains: async () =>
          failure === "extra-domains" ? ["example.com", "example.com"] : ["example.com"],
        native,
      }),
    ).rejects.toThrow(
      failure === "response"
        ? "uac-cancelled"
        : failure === "rejection"
          ? "helper-disconnected"
          : "verification failed",
    );
    expect(call).toBe(3);
    expect(requests).toEqual(["focus.get_active_state"]);
  });
}

test("a delayed activation cannot complete after a newer deactivation", async () => {
  const events: string[] = [];
  let state = { ...inactive };
  let nativeDomains: string[] = [];
  let releaseActivation!: () => void;
  let markActivationStarted!: () => void;
  const activationGate = new Promise<void>((resolve) => (releaseActivation = resolve));
  const activationStarted = new Promise<void>((resolve) => (markActivationStarted = resolve));
  const request: FocusRequest = async (operation, params) => {
    events.push(`request:${operation}`);
    if (operation === "focus.get_active_state") return state;
    if (operation === "focus.set_active_state") {
      state = {
        active: params?.active === true,
        blocklist_id: params?.active === true ? "work" : null,
        blocked_app_ids: [],
        blocked_apps: [],
      };
      events.push(`persist:${state.active}`);
      return state;
    }
    throw new Error(`unexpected operation: ${operation}`);
  };
  const native: FocusNativeAdapter = {
    status: async () => ok(nativeDomains),
    apply: async ({ active, domains }) => {
      events.push(`native:${active}:start`);
      if (active) {
        markActivationStarted();
        await activationGate;
      }
      nativeDomains = [...domains];
      events.push(`native:${active}:done`);
      return ok(nativeDomains);
    },
  };

  const activation = applyAuthoritativeFocusState({
    request,
    active: true,
    blocklistId: "work",
    resolveDomains: async () => ["example.com"],
    native,
  });
  await activationStarted;
  const deactivation = applyAuthoritativeFocusState({ request, active: false, native });
  try {
    await Promise.resolve();
    expect(events.filter((event) => event === "request:focus.get_active_state")).toHaveLength(1);
  } finally {
    releaseActivation();
    await Promise.allSettled([activation, deactivation]);
  }
  await Promise.all([activation, deactivation]);
  expect(events.indexOf("persist:true")).toBeLessThan(events.lastIndexOf("native:false:start"));
  expect(events.at(-1)).toBe("persist:false");
  expect(state.active).toBe(false);
  expect(nativeDomains).toEqual([]);
});

test("restart reconciliation reapplies persisted native enforcement", async () => {
  const calls: boolean[] = [];
  let nativeDomains: string[] = [];
  const native: FocusNativeAdapter = {
    status: async () => ok(nativeDomains),
    apply: async ({ active, domains }) => {
      calls.push(active);
      nativeDomains = [...domains];
      return ok(nativeDomains);
    },
  };
  const request: FocusRequest = async (operation) => {
    if (operation !== "focus.get_active_state") throw new Error(`unexpected ${operation}`);
    return { ...inactive, active: true, blocklist_id: "work" };
  };

  const reconciled = await reconcileFocusState({
    request,
    resolveDomains: async () => ["example.com"],
    native,
  });

  expect(reconciled.nativeActive).toBe(true);
  expect(calls).toEqual([false, true]);
  expect(nativeDomains).toEqual(["example.com"]);
});

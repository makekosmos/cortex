import { reactive } from "vue";
import type { ManagerClient } from "./useManagerClient";
import type { PackageDisclosure } from "../manager-api";
import { disclosureSections, type DisclosureSection } from "../disclosure-helpers";

export type DisclosureTarget = {
  package_id: string;
  version?: string;
};

// Session-scoped consent state for the pre-connect disclosure screen. The
// declared permission contract always comes from the engine
// (`packages.disclosure`), never from marketplace copy.
type DisclosureState = {
  open: boolean;
  busy: boolean;
  unavailable: boolean;
  name: string;
  sections: DisclosureSection[];
};

export function usePackageDisclosure(client: Pick<ManagerClient, "call">) {
  const state = reactive<DisclosureState>({
    open: false,
    busy: false,
    unavailable: false,
    name: "",
    sections: [],
  });
  const consented = new Set<string>();
  let pending: (() => Promise<void> | void) | null = null;
  let pendingKey = "";

  async function request(
    target: DisclosureTarget | null,
    label: string,
    proceed: () => Promise<void> | void,
  ) {
    const key = target ? `${target.package_id}@${target.version ?? ""}` : "";
    if (key && consented.has(key)) {
      await proceed();
      return;
    }
    pending = proceed;
    pendingKey = key;
    state.name = label;
    state.sections = [];
    state.unavailable = !target?.version;
    state.busy = Boolean(target?.version);
    state.open = true;
    if (!target?.version) return;
    const result = await client.call<PackageDisclosure>(
      "getPackageDisclosure",
      { package_id: target.package_id, version: target.version },
      `disclosure:${key}`,
    );
    if (!state.open || pendingKey !== key) return;
    state.busy = false;
    if (result) {
      state.sections = disclosureSections(result);
      state.name = result.name || label;
    } else {
      state.unavailable = true;
    }
  }

  async function confirm() {
    const action = pending;
    if (pendingKey) consented.add(pendingKey);
    state.open = false;
    pending = null;
    pendingKey = "";
    if (action) await action();
  }

  function decline() {
    state.open = false;
    pending = null;
    pendingKey = "";
  }

  return { state, request, confirm, decline };
}

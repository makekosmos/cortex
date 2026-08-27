import { applyFocusBlock, assertFocusBlockResult, getFocusBlockStatus } from "./focus-block";
import { isRecord, isString, type JsonRecord, type JsonValue } from "./extension-permissions";

export type FocusRequest = (operation: string, params?: JsonRecord) => Promise<JsonValue>;
const isBoolean = (value: JsonValue | undefined): value is boolean => typeof value === "boolean";

type ActiveState = {
  active: boolean;
  blocklist_id?: string | null;
  blocked_app_ids: string[];
  blocked_apps: JsonValue[];
};

let focusTransitionQueue: Promise<void> = Promise.resolve();

// ponytail: process-wide lock; split by session only if independent sessions are added.
function enqueueFocusTransition<T>(work: () => Promise<T>): Promise<T> {
  const next = focusTransitionQueue.then(work, work);
  focusTransitionQueue = next.then(
    () => undefined,
    () => undefined,
  );
  return next;
}

function readActiveState(value: JsonValue): ActiveState {
  if (!isRecord(value)) {
    throw new Error("focus.get_active_state returned an invalid state");
  }
  if (!isBoolean(value.active)) {
    throw new Error("focus.get_active_state returned an invalid active flag");
  }
  if (
    value.blocklist_id !== undefined &&
    value.blocklist_id !== null &&
    !isString(value.blocklist_id)
  ) {
    throw new Error("focus.get_active_state returned an invalid blocklist id");
  }
  if (
    value.blocked_app_ids !== undefined &&
    (!Array.isArray(value.blocked_app_ids) || !value.blocked_app_ids.every(isString))
  ) {
    throw new Error("focus.get_active_state returned invalid blocked app ids");
  }
  if (value.blocked_apps !== undefined && !Array.isArray(value.blocked_apps)) {
    throw new Error("focus.get_active_state returned invalid blocked apps");
  }
  // SAFETY: the branch above verifies blocklist_id is string, null, or absent.
  return {
    active: value.active,
    blocklist_id: value.blocklist_id as string | null | undefined,
    blocked_app_ids: (value.blocked_app_ids as string[] | undefined) ?? [],
    blocked_apps: (value.blocked_apps as JsonValue[] | undefined) ?? [],
  };
}

async function domainsForState(
  request: FocusRequest,
  state: ActiveState,
  resolveDomains?: (blocklistId: string) => Promise<string[]>,
): Promise<string[]> {
  const blocklistId = state.blocklist_id?.trim() || null;
  if (!state.active || !blocklistId) return [];
  return resolveDomains
    ? resolveDomains(blocklistId)
    : resolveDomainsFromRequest(request, blocklistId);
}

async function resolveDomainsFromRequest(
  request: FocusRequest,
  blocklistId: string,
): Promise<string[]> {
  try {
    const result = await request("focus.resolve_blocklist_domains", { id: blocklistId });
    if (!isRecord(result) || !Array.isArray(result.domains) || !result.domains.every(isString)) {
      throw new Error("invalid domain resolution");
    }
    return result.domains;
  } catch {
    const result = await request("focus.list_blocklists");
    const list = isRecord(result) && Array.isArray(result.blocklists) ? result.blocklists : [];
    const blocklist = list.find((item) => isRecord(item) && item.id === blocklistId);
    if (
      !isRecord(blocklist) ||
      !Array.isArray(blocklist.domains) ||
      !blocklist.domains.every(isString)
    ) {
      throw new Error(`focus blocklist not found: ${blocklistId}`);
    }
    return blocklist.domains;
  }
}

async function resolveDomains(request: FocusRequest, blocklistId: string): Promise<string[]> {
  return resolveDomainsFromRequest(request, blocklistId);
}

async function nativeState(domains: string[]): Promise<void> {
  if (domains.length > 0) {
    const reset = await applyFocusBlock({ active: false, domains: [] });
    assertFocusBlockResult(reset, []);
  }
  const result = await applyFocusBlock({ active: domains.length > 0, domains });
  assertFocusBlockResult(result, domains);
}

async function restoreNativeState(domains: string[]): Promise<void> {
  try {
    await nativeState(domains);
  } catch (error) {
    throw new Error(`native focus rollback failed: ${String(error)}`);
  }
}

async function applyAuthoritativeFocusStateUnsafe(options: {
  request: FocusRequest;
  active: boolean;
  blocklistId?: string | null;
  blockedAppIds?: string[];
  blockedApps?: JsonValue[];
  resolveDomains?: (blocklistId: string) => Promise<string[]>;
}): Promise<{ result: JsonValue; nativeActive: boolean }> {
  readActiveState(await options.request("focus.get_active_state"));
  const before = await getFocusBlockStatus();
  assertFocusBlockResult(before, before.active_domains ?? null);

  const blocklistId = options.blocklistId?.trim() || null;
  const hasAppTarget =
    (options.blockedAppIds?.length ?? 0) > 0 || (options.blockedApps?.length ?? 0) > 0;
  if (options.active && !blocklistId && !hasAppTarget) {
    throw new Error("active focus state requires a blocklist or blocked app");
  }
  const domains =
    options.active && blocklistId
      ? await (options.resolveDomains?.(blocklistId) ??
          resolveDomains(options.request, blocklistId))
      : [];
  if (options.active && blocklistId && domains.length === 0) {
    throw new Error(`focus blocklist is empty: ${blocklistId}`);
  }

  try {
    await nativeState(domains);
  } catch (error) {
    await restoreNativeState(before.active_domains ?? []);
    throw error;
  }
  try {
    const result = await options.request("focus.set_active_state", {
      active: options.active,
      blocklist_id: blocklistId,
      blocked_app_ids: options.blockedAppIds ?? [],
      blocked_apps: options.blockedApps ?? [],
    });
    return { result, nativeActive: domains.length > 0 };
  } catch (error) {
    try {
      const persisted = readActiveState(await options.request("focus.get_active_state"));
      await nativeState(await domainsForState(options.request, persisted, options.resolveDomains));
    } catch (readbackError) {
      await restoreNativeState(before.active_domains ?? []);
      throw new Error(
        `focus state write failed: ${String(error)}; read-back failed: ${String(readbackError)}`,
      );
    }
    throw error;
  }
}

export function applyAuthoritativeFocusState(options: {
  request: FocusRequest;
  active: boolean;
  blocklistId?: string | null;
  blockedAppIds?: string[];
  blockedApps?: JsonValue[];
  resolveDomains?: (blocklistId: string) => Promise<string[]>;
}): Promise<{ result: JsonValue; nativeActive: boolean }> {
  return enqueueFocusTransition(() => applyAuthoritativeFocusStateUnsafe(options));
}

async function reconcileFocusStateUnsafe(options: {
  request: FocusRequest;
  resolveDomains?: (blocklistId: string) => Promise<string[]>;
}): Promise<{ nativeActive: boolean; state: ActiveState }> {
  const persisted = readActiveState(await options.request("focus.get_active_state"));
  const status = await getFocusBlockStatus();
  assertFocusBlockResult(status, status.active_domains ?? null);
  const blocklistId = persisted.blocklist_id?.trim() || null;
  if (
    persisted.active &&
    !blocklistId &&
    persisted.blocked_app_ids.length === 0 &&
    persisted.blocked_apps.length === 0
  ) {
    throw new Error("active focus state has no enforcement target");
  }
  let domains: string[] = [];
  try {
    domains = await domainsForState(options.request, persisted, options.resolveDomains);
  } catch (error) {
    try {
      await nativeState([]);
    } catch (resetError) {
      throw new Error(
        `focus reconciliation failed: ${String(error)}; native reset failed: ${String(resetError)}`,
      );
    }
    throw error;
  }
  if (persisted.active && blocklistId && domains.length === 0) {
    throw new Error(`focus blocklist is empty: ${blocklistId}`);
  }
  try {
    await nativeState(domains);
  } catch (error) {
    await restoreNativeState(status.active_domains ?? []);
    throw error;
  }
  return { nativeActive: domains.length > 0, state: persisted };
}

export function reconcileFocusState(options: {
  request: FocusRequest;
  resolveDomains?: (blocklistId: string) => Promise<string[]>;
}): Promise<{ nativeActive: boolean; state: ActiveState }> {
  return enqueueFocusTransition(() => reconcileFocusStateUnsafe(options));
}

export function resetNativeFocusState(): Promise<void> {
  return enqueueFocusTransition(() => nativeState([]));
}

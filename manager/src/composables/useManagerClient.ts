import { computed, getCurrentInstance, onBeforeUnmount, ref } from "vue";
import type { ComputedRef, Ref } from "vue";
import type { ManagerApi, ManagerErrorCode, ManagerResult } from "../manager-api";

type ManagerMethod = keyof ManagerApi;
type JsonValue = string | number | boolean | null | JsonRecord | JsonValue[];
interface JsonRecord {
  [key: string]: JsonValue;
}

const CONNECTIVITY_CODES: ReadonlySet<ManagerErrorCode> = new Set([
  "engine_unavailable",
  "transport",
  "incompatible_api",
]);

export type EngineState = "connecting" | "ready" | "failed";

export interface ManagerClientOptions {
  api?: ManagerApi;
  graceMs?: number;
  probeMs?: number;
}

export interface ManagerClient {
  loading: Ref<boolean>;
  error: Ref<string | null>;
  engine: Ref<EngineState>;
  banner: ComputedRef<string | null>;
  call<T>(method: ManagerMethod, payload?: JsonValue, key?: string): Promise<T | null>;
  dispose(): void;
}

const ENGINE_GRACE_MS = 5_000;
const ENGINE_PROBE_MS = 1_500;

export function useManagerClient(options: ManagerClientOptions = {}): ManagerClient {
  const graceMs = options.graceMs ?? ENGINE_GRACE_MS;
  const probeMs = options.probeMs ?? ENGINE_PROBE_MS;
  const getApi = (): ManagerApi | undefined => options.api ?? window.kosmosManager;
  const loading = ref(false);
  const rawError = ref<string | null>(null);
  const engine = ref<EngineState>("connecting");
  // True while `rawError` holds a connectivity failure; the banner stays gated
  // by the grace window so cold starts and brief reconnects do not flash red.
  const errorIsConnectivity = ref(false);
  // Views write `error` directly for their own messages — always non-connectivity.
  const error = computed<string | null>({
    get: () => rawError.value,
    set: (value) => {
      rawError.value = value;
      errorIsConnectivity.value = false;
    },
  });
  function setError(message: string | null, connectivity = false) {
    rawError.value = message;
    errorIsConnectivity.value = connectivity;
  }
  // Last connectivity failure — the only text the root banner may show.
  const outageError = ref<string | null>(null);
  const requests = new Map<string, number>();
  let disposed = false;
  let failTimer: ReturnType<typeof setTimeout> | null = null;
  let probeTimer: ReturnType<typeof setTimeout> | null = null;
  let probing = false;

  function dispose() {
    disposed = true;
    requests.clear();
    if (failTimer !== null) clearTimeout(failTimer);
    if (probeTimer !== null) clearTimeout(probeTimer);
  }

  function noteSuccess() {
    if (failTimer !== null) {
      clearTimeout(failTimer);
      failTimer = null;
    }
    if (probeTimer !== null) {
      clearTimeout(probeTimer);
      probeTimer = null;
    }
    outageError.value = null;
    engine.value = "ready";
  }

  function noteOutage() {
    if (disposed) return;
    if (engine.value !== "failed") {
      engine.value = "connecting";
      if (failTimer === null) {
        failTimer = setTimeout(() => {
          failTimer = null;
          engine.value = "failed";
        }, graceMs);
      }
    }
    scheduleProbe();
  }

  function scheduleProbe() {
    if (disposed || probeTimer !== null || engine.value === "ready") return;
    probeTimer = setTimeout(() => void probe(), probeMs);
  }

  // Lightweight readiness probe: keeps `engine` converging without touching the
  // visible error/loading state that `call` manages for view requests.
  async function probe() {
    probeTimer = null;
    if (disposed || engine.value === "ready" || probing) return;
    probing = true;
    try {
      const result = await getApi()?.getHealth();
      if (result?.ok) {
        noteSuccess();
        if (errorIsConnectivity.value) setError(null);
      } else {
        noteOutage();
      }
    } catch {
      noteOutage();
    } finally {
      probing = false;
    }
  }

  // The red banner means exactly one thing: no Engine access for the whole
  // grace window. Ordinary errors (including `engine` answers from a reachable
  // backend) stay on `error` for view-level display and never flash the banner.
  const banner = computed(() =>
    engine.value === "failed"
      ? (outageError.value ?? "Engine недоступен. Повторите попытку.")
      : null,
  );

  async function call<T>(
    method: ManagerMethod,
    payload?: JsonValue,
    key = method,
  ): Promise<T | null> {
    const request = (requests.get(key) ?? 0) + 1;
    requests.set(key, request);
    loading.value = true;
    if (!errorIsConnectivity.value) setError(null);
    try {
      // SAFETY: ManagerApi methods share the JSON payload boundary used by the IPC preload.
      const handler = getApi()?.[method] as (value?: JsonValue) => Promise<ManagerResult<T>>;
      if (!handler) throw new Error("Менеджер недоступен");
      const result = await handler(payload);
      if (disposed || requests.get(key) !== request) return null;
      if (!result.ok) {
        const connectivity = CONNECTIVITY_CODES.has(result.code);
        setError(result.message, connectivity);
        if (connectivity) {
          outageError.value = result.message;
          noteOutage();
        } else if (result.code === "engine") {
          // The backend answered — transport is alive; not an outage.
          noteSuccess();
        }
        return null;
      }
      setError(null);
      noteSuccess();
      return result.data;
    } catch (cause) {
      if (!disposed && requests.get(key) === request) {
        const message = cause instanceof Error ? cause.message : "Не удалось связаться с движком";
        setError(message, true);
        outageError.value = message;
        noteOutage();
      }
      return null;
    } finally {
      if (!disposed && requests.get(key) === request) loading.value = false;
    }
  }

  if (getCurrentInstance()) onBeforeUnmount(dispose);

  return { loading, error, engine, banner, call, dispose };
}

export type { ManagerResult };

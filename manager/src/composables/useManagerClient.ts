import { onBeforeUnmount, ref, type Ref } from "vue";
import type { ManagerApi, ManagerResult } from "../manager-api";

type ManagerMethod = keyof ManagerApi;

export interface ManagerClient {
  loading: Ref<boolean>;
  error: Ref<string | null>;
  call<T>(method: ManagerMethod, payload?: unknown, key?: string): Promise<T | null>;
}

function getApi(): ManagerApi | undefined {
  return window.kosmosManager;
}

export function useManagerClient(): ManagerClient {
  const loading = ref(false);
  const error = ref<string | null>(null);
  const requests = new Map<string, number>();
  let disposed = false;

  async function call<T>(
    method: ManagerMethod,
    payload?: unknown,
    key = method,
  ): Promise<T | null> {
    const request = (requests.get(key) ?? 0) + 1;
    requests.set(key, request);
    loading.value = true;
    error.value = null;
    try {
      const handler = getApi()?.[method] as (value?: unknown) => Promise<ManagerResult<T>>;
      if (!handler) throw new Error("Менеджер недоступен");
      const result = await handler(payload);
      if (disposed || requests.get(key) !== request) return null;
      if (!result.ok) {
        error.value = result.message;
        return null;
      }
      return result.data;
    } catch (cause) {
      if (!disposed && requests.get(key) === request) {
        error.value = cause instanceof Error ? cause.message : "Не удалось связаться с движком";
      }
      return null;
    } finally {
      if (!disposed && requests.get(key) === request) loading.value = false;
    }
  }

  onBeforeUnmount(() => {
    disposed = true;
    requests.clear();
  });

  return { loading, error, call };
}

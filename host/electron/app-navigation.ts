import type { JsonRecord, JsonValue } from "./host-api";

export const SAFE_ID = /^[a-z0-9][a-z0-9._-]{0,127}$/;
const SAFE_ROUTE = /^\/(?!\/)[A-Za-z0-9._~!$&'()*+,;=:@%/-]{0,1023}$/;

export type OpenAppRequest = Readonly<{
  id: string;
  route?: string;
}>;

export type OpenAppRequestResult =
  | { ok: true; request: OpenAppRequest }
  | { ok: false; message: string };

export function isSafeAppRoute(route: string): boolean {
  return SAFE_ROUTE.test(route) && (!route.includes("%") || isValidEncodedRoute(route));
}

export function parseOpenAppRequest(input: JsonRecord | undefined): OpenAppRequestResult {
  if (!input || !isString(input.id) || !SAFE_ID.test(input.id)) {
    return { ok: false, message: "Некорректный идентификатор приложения." };
  }
  if (input.route === undefined) return { ok: true, request: { id: input.id } };
  if (!isString(input.route) || !isSafeAppRoute(input.route)) {
    return { ok: false, message: "Некорректный маршрут приложения." };
  }
  return { ok: true, request: { id: input.id, route: input.route } };
}

export async function sendNavigationWhenReady(
  ready: Promise<boolean> | undefined,
  send: () => void,
): Promise<boolean> {
  if (ready && !(await ready)) return false;
  try {
    send();
    return true;
  } catch {
    return false;
  }
}

function isString(value: JsonValue | undefined): value is string {
  return typeof value === "string";
}

function isValidEncodedRoute(route: string): boolean {
  try {
    return !decodeURIComponent(route).startsWith("//");
  } catch {
    return false;
  }
}

import { createHash } from "node:crypto";
import {
  isJsonBoolean,
  isJsonNumber,
  isJsonRecord,
  isJsonString,
  type JsonRecord,
  type JsonValue,
} from "./host-api";
import { isSafeAppRoute } from "./app-navigation";

// Renderer-keyed secondary windows ("sticker"-style surfaces) of an already
// launched app. They share the app's Engine launch lease: ownership is only
// released when the last window of the app goes away.
export const MAX_AUX_WINDOWS_PER_APP = 8;

// Window keys are renderer-provided identifiers ("sticker:<noteId>"), so they
// stay short, printable and path-safe; persistence hashes them anyway.
const AUX_WINDOW_KEY = /^[A-Za-z0-9][A-Za-z0-9._:%-]{0,63}$/;
const MIN_WIDTH = 160;
const MIN_HEIGHT = 120;
const MAX_WIDTH = 3840;
const MAX_HEIGHT = 2160;

export type AuxWindowSpec = Readonly<{
  key: string;
  route?: string;
  width: number;
  height: number;
  minWidth: number;
  minHeight: number;
  alwaysOnTop?: boolean;
}>;

export type AuxWindowSpecResult =
  | { ok: true; spec: AuxWindowSpec }
  | { ok: false; message: string };

export type AuxWindowState = Readonly<{
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  alwaysOnTop?: boolean;
}>;

// Mutable build-time views of the readonly result contracts above.
type AuxWindowSpecDraft = {
  key: string;
  route?: string;
  width: number;
  height: number;
  minWidth: number;
  minHeight: number;
  alwaysOnTop?: boolean;
};
type AuxWindowStateDraft = {
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  alwaysOnTop?: boolean;
};

const dimension = (
  value: JsonValue | undefined,
  lo: number,
  hi: number,
  fallback: number,
): number =>
  isJsonNumber(value) && Number.isFinite(value)
    ? Math.min(hi, Math.max(lo, Math.round(value)))
    : fallback;

export function parseAuxWindowSpec(input: JsonRecord | undefined): AuxWindowSpecResult {
  if (!input || !isJsonString(input.key) || !AUX_WINDOW_KEY.test(input.key))
    return { ok: false, message: "Некорректный ключ окна приложения." };
  const route = isJsonString(input.route) && isSafeAppRoute(input.route) ? input.route : undefined;
  if (input.route !== undefined && route === undefined)
    return { ok: false, message: "Некорректный маршрут окна приложения." };
  const alwaysOnTop = isJsonBoolean(input.alwaysOnTop) ? input.alwaysOnTop : undefined;
  if (input.alwaysOnTop !== undefined && alwaysOnTop === undefined)
    return { ok: false, message: "Некорректный параметр окна приложения." };
  const width = dimension(input.width, MIN_WIDTH, MAX_WIDTH, 380);
  const height = dimension(input.height, MIN_HEIGHT, MAX_HEIGHT, 480);
  const spec: AuxWindowSpecDraft = {
    key: input.key,
    width,
    height,
    minWidth: dimension(input.minWidth, MIN_WIDTH, width, 240),
    minHeight: dimension(input.minHeight, MIN_HEIGHT, height, 180),
  };
  if (route !== undefined) spec.route = route;
  if (alwaysOnTop !== undefined) spec.alwaysOnTop = alwaysOnTop;
  return { ok: true, spec };
}

export function auxWindowKey(appId: string, key: string): string {
  return `${appId} ${key}`;
}

// Aux window bounds are host-internal bookkeeping, not app user data: they
// live in `window-states/<appId>/`, outside the app-writable `extension-data`
// tree, and the file name is a hash of the renderer-provided key.
export function auxWindowStateFileName(key: string): string {
  return `${createHash("sha256").update(key).digest("hex").slice(0, 16)}.json`;
}

export function parseAuxWindowState(value: JsonValue | undefined): AuxWindowState {
  if (!isJsonRecord(value)) return {};
  const state: AuxWindowStateDraft = {};
  for (const field of ["x", "y", "width", "height"] as const) {
    const candidate = value[field];
    if (isJsonNumber(candidate) && Number.isFinite(candidate)) state[field] = Math.round(candidate);
  }
  // Persisted files are host-owned but still hand-editable: keep restored
  // sizes inside the same envelope as renderer-provided spec dimensions.
  if (state.width !== undefined)
    state.width = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, state.width));
  if (state.height !== undefined)
    state.height = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, state.height));
  if (isJsonBoolean(value.alwaysOnTop)) state.alwaysOnTop = value.alwaysOnTop;
  return state;
}

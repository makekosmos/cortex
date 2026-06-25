import { createRaycastElement } from "./components";

export const Fragment = "Fragment";

const EMPTY_JSX_PROPS: Record<string, unknown> = {};

function normalizeJsxProps(
  props: Record<string, unknown> | null | undefined,
): Record<string, unknown> {
  return props ?? EMPTY_JSX_PROPS;
}

export function jsx(
  type: string | ((props: Record<string, unknown>) => unknown),
  props: Record<string, unknown> | null | undefined,
) {
  const normalizedProps = normalizeJsxProps(props);
  if (typeof type === "function") return type(normalizedProps);
  return createRaycastElement(type, normalizedProps);
}

export const jsxs = jsx;

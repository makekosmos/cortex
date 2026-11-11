import { signal } from "nsh";

/**
 * Where the pages have been, so the header's anchors can walk back and forth.
 *
 * The sidebar and the Engine's `navigate` event both land here: a page reached
 * from either place is a step the anchors can undo. A route that equals the
 * current one is not a step — pressing the sidebar row you are already on must
 * not fill the history with the same page.
 */
// `commands.ts` declares the route global the host seeds the bundle with.
const initial = globalThis.__MUNDUS_MANAGER_ROUTE__ ?? "server";

const stack = signal<string[]>([initial]);
const at = signal(0);

export const currentRoute = () => stack()[at()];
export const canBack = () => at() > 0;
export const canForward = () => at() < stack().length - 1;

export function go(route: string) {
  if (route === currentRoute()) return;
  const kept = stack().slice(0, at() + 1);
  stack.set([...kept, route]);
  at.set(kept.length);
}

export function back() {
  if (canBack()) at.set(at() - 1);
}

export function forward() {
  if (canForward()) at.set(at() + 1);
}

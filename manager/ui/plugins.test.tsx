import { open, test } from "nsh/test";
import { hasNewer } from "./pages/plugins";
import { demoState } from "./state";

/**
 * The page that offers an update, without a host: the Manager reads its first
 * state from `__MUNDUS_MANAGER_INITIAL__`, which is how the shell hands it over,
 * and the demo state is what stands in for it. A registry answer is the only
 * thing these tests change — what the catalog says a plugin's newest release is.
 */
function initialState(latest: string) {
  const id = demoState.plugins.installed[0].id;
  return {
    ...demoState,
    plugins: {
      ...demoState.plugins,
      releases: { [id]: { version: latest, tag: `dictation-v${latest}` } },
    },
  };
}

test("an installed plugin with a newer release offers «Обновить»", async () => {
  globalThis.__MUNDUS_MANAGER_INITIAL__ = initialState("0.5.3");
  const app = await open("./App.tsx");
  await app.press("Плагины");
  // The pill is the promise: it exists only because the registry named a
  // version the install does not have.
  await app.waitFor("Обновить");
});

test("a plugin already on the newest release has nothing to press", async () => {
  const version = demoState.plugins.installed[0].version;
  // A page is opened once per test file, so the switch is checked where the
  // decision is made rather than by opening a second app with other state.
  if (hasNewer(version, version)) {
    throw new Error("a plugin is offered its own version as an update");
  }
  if (hasNewer(version, undefined) || hasNewer(version, "")) {
    throw new Error("an unanswered registry check counts as an update");
  }
  if (!hasNewer(version, "0.5.3")) {
    throw new Error("a newer release is not offered as an update");
  }
});

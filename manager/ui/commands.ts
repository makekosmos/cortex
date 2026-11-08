import type { ManagerState } from "./state";

/**
 * A press, as the pages hand it to the app: the operation's name and the few
 * typed fields one carries. `nsh` seeds the page from these two globals when a
 * host embeds the bundle with a starting state (the screenshot tests do).
 */
declare global {
  // eslint-disable-next-line no-var
  var __MUNDUS_MANAGER_INITIAL__: ManagerState | undefined;
  // eslint-disable-next-line no-var
  var __MUNDUS_MANAGER_ROUTE__: string | undefined;
}

/** What a page may send with a command. Nothing untyped crosses this line. */
export type CommandArgs = {
  /** The row a command acts on: a device, a model, a backup. */
  id?: string;
  /** A switch's new position, or the text a field holds. */
  value?: string | boolean;
  /** A route to open. */
  route?: string;
  /** Which of the two theme slots a pick belongs to. */
  mode?: "light" | "dark";
  /** A theme key. */
  key?: string;
  /** The Engine's `appearance.set` patch, already named. */
  patch?: { dark_theme?: string; light_theme?: string; follow_apps?: boolean };
};

export type Command = (name: string, args?: CommandArgs) => void;

export type PageProps = {
  colors: import("@makekosmos/imago-nsh").ApplicationTheme;
  state: () => ManagerState;
  onCommand: Command;
};

import { platform } from "os";

/**
 * Windows draws three caption buttons on the right of the titleband — the
 * host paints them (`manager/src/chrome.rs`) or, under `nsh run`, the surface
 * does. Either way that strip is not the app's: a page heading that claimed it
 * as a drag area would turn a press on those buttons into a titlebar press, and
 * the button would never click.
 */
export const CAPTION_BUTTON_WIDTH = 46;

/**
 * The band the caption buttons sit in, which the page heading and the sidebar's
 * search field share with them: the first row of the window is the titlebar.
 */
export const TITLEBAR_HEIGHT = 36;

export function reservesCaptions(): boolean {
  const host = platform();
  return host === "win32" || host === "windows";
}

/** The width of the strip the caption buttons take, in points. */
export function captionStrip(): number {
  return reservesCaptions() ? CAPTION_BUTTON_WIDTH * 3 : 0;
}

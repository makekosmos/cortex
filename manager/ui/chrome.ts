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
 *
 * 44, not the 36 a bare caption strip needs: at 36 a 32-point search field sits
 * one point off the window's edge and the heading's title two thirds of the way
 * up, which reads as "no titlebar" next to the macOS build, where the same two
 * sit in a 48-point band. Windows gets the same room for its own chrome.
 */
export const TITLEBAR_HEIGHT = 44;

export function reservesCaptions(): boolean {
  const host = platform();
  return host === "win32" || host === "windows";
}

/** The width of the strip the caption buttons take, in points. */
export function captionStrip(): number {
  return reservesCaptions() ? CAPTION_BUTTON_WIDTH * 3 : 0;
}

/**
 * Where macOS puts the traffic lights: twenty points in and down from the
 * corner, the way Chrome and System Settings do it. `manager/src/main.rs` sets
 * the same point on the window, so the buttons land over the empty band the
 * sidebar keeps for them.
 */
export const MAC_LIGHTS_LEFT = 20;
export const MAC_LIGHTS_TOP = 18;

/** Below the lights: the three buttons, then air, so the two bands read apart. */
const MAC_LIGHTS_INSET = MAC_LIGHTS_TOP + 12 + 18;

/**
 * The page header's height. macOS puts the traffic lights a shade lower than
 * the band a Windows caption sits in, so the title needs a taller row to line
 * up with them; the window's own titlebar is not resized.
 */
export function headerHeight(): number {
  const host = platform();
  if (host === "darwin" || host === "macos") return MAC_LIGHTS_TOP + 30;
  return TITLEBAR_HEIGHT;
}

/**
 * The space above the sidebar's search field.
 *
 * macOS draws the traffic lights over the field's own corner, so the field
 * starts below the band they sit in. Windows draws its caption buttons at the
 * far right, and the sidebar's first row (search) shares that band — centred in
 * it, which is what puts the field level with the window controls instead of
 * 2 pt under the top edge.
 */
export function sidebarTopInset(searchHeight: number): number {
  const host = platform();
  if (host === "darwin" || host === "macos") return MAC_LIGHTS_INSET;
  if (host === "win32" || host === "windows") return (TITLEBAR_HEIGHT - searchHeight) / 2;
  return 16;
}

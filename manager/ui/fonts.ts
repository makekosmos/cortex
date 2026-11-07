// The design is set in Inter. Importing the file registers the family with the
// host before the first frame, and the theme (font-sans) names it, so nothing
// falls back to the system's UI face.
import inter from "./assets/Inter.ttf";

export const uiFont: string = inter;

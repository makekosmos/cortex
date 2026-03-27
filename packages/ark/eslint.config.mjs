// Root ESLint config.
//
// Some editors (VS Code ESLint extension) pick up ESLint config only from the
// workspace root. The actual JS/TS/Svelte code lives in `ui/`, so we simply
// re-export the UI flat config from the repo root to avoid false-positive
// diagnostics (e.g. `RequestInit` reported as `no-undef`).

import uiConfig from "./ui/eslint.config.js";

export default uiConfig;

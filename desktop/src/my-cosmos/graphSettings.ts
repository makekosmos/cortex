// Shared types and helpers for graph settings persistence.

export interface GraphSettings {
  gravity: number;
  repulsion: number;
  linkDistance: number;
  friction: number;
  pointSizeScale: number;
  linkOpacity: number;
  curvedLinks: boolean;
  highlightNeighbors: boolean;
  frozen: boolean;
  antiOverlap: boolean;
  gap: number;
}

export const DEFAULT_GRAPH_SETTINGS: GraphSettings = {
  gravity: 0.1,
  repulsion: 2.0,
  linkDistance: 18,
  friction: 0.85,
  pointSizeScale: 1,
  linkOpacity: 0.35,
  curvedLinks: false,
  highlightNeighbors: true,
  frozen: false,
  antiOverlap: true,
  gap: 8,
};

const LS_KEY = "kosmos.myCosmos.graphSettings";

function mergeGraphSettings(value: unknown): GraphSettings {
  const next = { ...DEFAULT_GRAPH_SETTINGS };
  if (!value || typeof value !== "object" || Array.isArray(value)) return next;
  const parsed = value as Record<string, unknown>;

  for (const key of [
    "gravity",
    "repulsion",
    "linkDistance",
    "friction",
    "pointSizeScale",
    "linkOpacity",
    "gap",
  ] as const) {
    if (typeof parsed[key] === "number" && Number.isFinite(parsed[key])) {
      next[key] = parsed[key];
    }
  }

  for (const key of ["curvedLinks", "highlightNeighbors", "frozen", "antiOverlap"] as const) {
    if (typeof parsed[key] === "boolean") {
      next[key] = parsed[key];
    }
  }

  return next;
}

export function loadGraphSettings(): GraphSettings {
  try {
    if (typeof localStorage === "undefined") return { ...DEFAULT_GRAPH_SETTINGS };
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return { ...DEFAULT_GRAPH_SETTINGS };
    return mergeGraphSettings(JSON.parse(raw));
  } catch {
    return { ...DEFAULT_GRAPH_SETTINGS };
  }
}

export function saveGraphSettings(settings: GraphSettings): void {
  try {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(LS_KEY, JSON.stringify(settings));
  } catch {
    // storage might be full or unavailable — silently ignore
  }
}

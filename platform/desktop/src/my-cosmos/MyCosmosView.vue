<template>
  <div class="cosmos-root">
    <div ref="containerRef" class="cosmos-canvas" />

    <!-- Прозрачная перетаскиваемая полоса сверху (как в настройках): тянем
         окно за верхушку. Нативные контролы (overlay) рендерятся ОС поверх. -->
    <div class="cosmos-dragbar" />

    <!-- Loading overlay -->
    <div v-if="loading" class="cosmos-loading">
      <span>Загрузка…</span>
    </div>

    <!-- Empty state overlay -->
    <div v-else-if="nodeCount === 0" class="cosmos-empty">
      <h2 class="cosmos-empty__heading">Космос пуст</h2>
      <p class="cosmos-empty__sub">Нет объектов для отображения</p>
    </div>

    <!-- Hover tooltip -->
    <div
      v-if="tooltip.visible"
      class="cosmos-tooltip"
      :style="{ top: tooltip.y + 'px', left: tooltip.x + 'px' }"
    >
      {{ tooltip.text }}
    </div>

    <!-- Legend -->
    <div v-if="legend.length > 0" class="cosmos-legend">
      <div v-for="entry in legend" :key="entry.typeId" class="cosmos-legend__row">
        <span class="cosmos-legend__dot" :style="{ background: entry.color }" />
        <span>{{ entry.name }}</span>
      </div>
    </div>

    <!-- Settings panel (anchored above gear button, bottom-right) -->
    <GraphSettingsPanel v-if="showSettings" :settings="settings" class="cosmos-settings-panel" />

    <!-- Gear button (bottom-right) -->
    <div class="cosmos-gear-wrap">
      <IconButton :size="32" :radius="8" @click="showSettings = !showSettings">
        <PhSlidersHorizontal :size="16" weight="regular" />
      </IconButton>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, watch, onMounted, onBeforeUnmount } from "vue";
import { Graph } from "@cosmos.gl/graph";
import { IconButton } from "@kosmos/visuals";
import { PhSlidersHorizontal } from "@phosphor-icons/vue";
import {
  buildGraph,
  type GraphObjectInput,
  type GraphLinkInput,
  type GraphTypeInput,
} from "./graphData";
import type { GraphNodeMeta } from "./graphData";
import { colorForType, cssColorToRgba } from "./typeColor";
import { HIDDEN_DASHBOARD_TYPE_IDS } from "../dashboard/typeVisuals";
import GraphSettingsPanel from "./GraphSettingsPanel.vue";
import { loadGraphSettings, saveGraphSettings, DEFAULT_GRAPH_SETTINGS } from "./graphSettings";
import type { GraphSettings } from "./graphSettings";

// ---------------------------------------------------------------------------
// Raw ARK response shapes
// ---------------------------------------------------------------------------

interface RawObjectType {
  id: string;
  name?: string;
  schemaJson?: string;
}

interface RawObject {
  id: string;
  typeId: string;
  title?: string;
  deletedAt?: string | null;
}

interface RawLink {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
  createdAt: string;
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

function rgbaArrayToCss(c: [number, number, number, number]): string {
  return `rgba(${(c[0] * 255) | 0}, ${(c[1] * 255) | 0}, ${(c[2] * 255) | 0}, ${c[3]})`;
}

// ---------------------------------------------------------------------------
// Settings state (persisted)
// ---------------------------------------------------------------------------

const settings: GraphSettings = reactive(loadGraphSettings());

watch(
  settings,
  (val) => {
    saveGraphSettings({ ...val });
  },
  { deep: true },
);

// ---------------------------------------------------------------------------
// UI state
// ---------------------------------------------------------------------------

const containerRef = ref<HTMLDivElement | null>(null);
const loading = ref(true);
const nodeCount = ref(0);
const tooltip = ref<{ visible: boolean; x: number; y: number; text: string }>({
  visible: false,
  x: 0,
  y: 0,
  text: "",
});
const legend = ref<Array<{ typeId: string; name: string; color: string }>>([]);
const showSettings = ref(false);

// ---------------------------------------------------------------------------
// Non-reactive: graph instance, node metadata, base point colors, dim state
// ---------------------------------------------------------------------------

let graph: Graph | null = null;
let nodeMeta: GraphNodeMeta[] = [];
let basePointColors: Float32Array = new Float32Array(0);
let isDimmed = false;
let settleTimer: ReturnType<typeof setTimeout> | null = null;
let relaxing = false;

// ---------------------------------------------------------------------------
// Anti-overlap relaxation
// ---------------------------------------------------------------------------

function relaxOverlaps(iterations = 60): void {
  if (!graph || nodeMeta.length < 2) return;

  // Space-per-screen-pixel scale
  const o = graph.screenToSpacePosition([0, 0]);
  const x = graph.screenToSpacePosition([1, 0]);
  const scale = Math.hypot(x[0] - o[0], x[1] - o[1]) || 1;

  const n = nodeMeta.length;
  const rawPos = graph.getPointPositions();
  // Copy into a mutable Float32Array (getPointPositions returns number[])
  const p = new Float32Array(n * 2);
  for (let i = 0; i < n * 2; i++) {
    p[i] = rawPos[i] ?? 0;
  }

  // Precompute radii in space units (gap already in screen px → space)
  const radius = new Float32Array(n);
  let maxRadius = 0;
  for (let i = 0; i < n; i++) {
    const r = ((graph.getPointRadiusByIndex(i) ?? 4) + settings.gap) * scale;
    radius[i] = r;
    if (r > maxRadius) maxRadius = r;
  }

  const cellSize = 2 * maxRadius || 1;

  for (let iter = 0; iter < iterations; iter++) {
    // Build spatial hash: cell key → array of node indices
    const grid = new Map<string, number[]>();
    for (let i = 0; i < n; i++) {
      const cx = Math.floor(p[i * 2] / cellSize);
      const cy = Math.floor(p[i * 2 + 1] / cellSize);
      const key = `${cx},${cy}`;
      let bucket = grid.get(key);
      if (!bucket) {
        bucket = [];
        grid.set(key, bucket);
      }
      bucket.push(i);
    }

    let maxMove = 0;

    for (let i = 0; i < n; i++) {
      const cx = Math.floor(p[i * 2] / cellSize);
      const cy = Math.floor(p[i * 2 + 1] / cellSize);

      // Check own cell + 8 neighbors
      for (let dcx = -1; dcx <= 1; dcx++) {
        for (let dcy = -1; dcy <= 1; dcy++) {
          const key = `${cx + dcx},${cy + dcy}`;
          const bucket = grid.get(key);
          if (!bucket) continue;
          for (const j of bucket) {
            if (j <= i) continue; // avoid double-checking & self
            const dx = p[j * 2] - p[i * 2];
            const dy = p[j * 2 + 1] - p[i * 2 + 1];
            const d = Math.hypot(dx, dy);
            const minD = radius[i] + radius[j];
            if (d < minD) {
              if (d > 0) {
                const overlap = (minD - d) / 2;
                const ux = dx / d;
                const uy = dy / d;
                const move = overlap;
                p[i * 2] -= ux * move;
                p[i * 2 + 1] -= uy * move;
                p[j * 2] += ux * move;
                p[j * 2 + 1] += uy * move;
                if (move > maxMove) maxMove = move;
              } else {
                // Exactly coincident — nudge with tiny random offset
                const angle = Math.random() * 2 * Math.PI;
                const nudge = 0.01 * scale;
                p[i * 2] -= Math.cos(angle) * nudge;
                p[i * 2 + 1] -= Math.sin(angle) * nudge;
                p[j * 2] += Math.cos(angle) * nudge;
                p[j * 2 + 1] += Math.sin(angle) * nudge;
                if (nudge > maxMove) maxMove = nudge;
              }
            }
          }
        }
      }
    }

    if (maxMove < 0.5 * scale) break; // sub-pixel convergence
  }

  graph.setPointPositions(p, true);
  graph.render();
}

function scheduleSettleAndRelax(): void {
  if (!settings.antiOverlap) return;
  if (settleTimer !== null) clearTimeout(settleTimer);
  settleTimer = setTimeout(() => {
    if (!graph) return;
    relaxOverlaps();
    graph.pause();
  }, 2200);
}

// ---------------------------------------------------------------------------
// Apply settings to live graph
// ---------------------------------------------------------------------------

function applySettings(s: GraphSettings): void {
  if (!graph) return;

  graph.setConfigPartial({
    simulationGravity: s.gravity,
    simulationRepulsion: s.repulsion,
    simulationLinkDistance: s.linkDistance,
    simulationFriction: s.friction,
    pointSizeScale: s.pointSizeScale,
    linkOpacity: s.linkOpacity,
    curvedLinks: s.curvedLinks,
  });

  if (s.frozen) {
    graph.pause();
    graph.render();
  } else if (s.antiOverlap) {
    graph.unpause();
    graph.start(0.3);
    scheduleSettleAndRelax();
  } else {
    graph.unpause();
    graph.start(0.2);
  }
}

// Watch settings after mount (graph may not exist yet, applySettings guards)
watch(
  settings,
  (s) => {
    applySettings(s);

    // If highlightNeighbors was turned off while dimmed, restore base colors
    if (!s.highlightNeighbors && isDimmed && graph) {
      graph.setPointColors(basePointColors);
      graph.render();
      isDimmed = false;
    }
  },
  { deep: true },
);

// When gap changes and antiOverlap is on, immediately re-resolve overlaps
watch(
  () => settings.gap,
  () => {
    if (settings.antiOverlap && graph) {
      relaxOverlaps();
    }
  },
);

// When antiOverlap is toggled
watch(
  () => settings.antiOverlap,
  (on) => {
    if (!graph) return;
    if (on) {
      scheduleSettleAndRelax();
    } else if (!settings.frozen) {
      if (settleTimer !== null) {
        clearTimeout(settleTimer);
        settleTimer = null;
      }
      graph.unpause();
      graph.start(0.3);
    }
  },
);

// ---------------------------------------------------------------------------
// Neighbor dim helpers
// ---------------------------------------------------------------------------

function dimAllExcept(index: number): void {
  if (!graph || basePointColors.length === 0) return;
  const keep = new Set(graph.getNeighboringPointIndices(index));
  keep.add(index);

  const dim = new Float32Array(basePointColors);
  const n = dim.length / 4;
  for (let i = 0; i < n; i++) {
    if (!keep.has(i)) {
      dim[i * 4 + 3] = basePointColors[i * 4 + 3] * 0.12;
    }
  }
  graph.setPointColors(dim);
  graph.render();
  isDimmed = true;
}

function restoreColors(): void {
  if (!graph || !isDimmed) return;
  graph.setPointColors(basePointColors);
  graph.render();
  isDimmed = false;
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

onMounted(async () => {
  // --- Fetch data ---
  let rawTypes: RawObjectType[] = [];
  let rawObjects: RawObject[] = [];
  let rawLinks: RawLink[] = [];

  try {
    [rawTypes, rawObjects, rawLinks] = await Promise.all([
      window.kepler.ark.request<RawObjectType[]>("list_object_types"),
      window.kepler.ark.request<RawObject[]>("list_objects"),
      window.kepler.ark.request<RawLink[]>("list_object_links"),
    ]);
  } catch (e) {
    console.warn("[my-cosmos] load failed", e);
    loading.value = false;
    nodeCount.value = 0;
    return;
  }

  // --- Map to graphData inputs ---
  const types: GraphTypeInput[] = rawTypes.map((t) => ({
    id: t.id,
    name: t.name && t.name.length > 0 ? t.name : t.id,
  }));

  const objects: GraphObjectInput[] = rawObjects.map((o) => ({
    id: o.id,
    typeId: o.typeId,
    title: o.title,
    deletedAt: o.deletedAt,
  }));

  const links: GraphLinkInput[] = rawLinks.map((l) => ({
    id: l.id,
    sourceObjectId: l.sourceObjectId,
    targetObjectId: l.targetObjectId,
    linkType: l.linkType,
  }));

  // --- Build graph data ---
  const built = buildGraph(objects, links, types, colorForType, {
    hiddenTypeIds: HIDDEN_DASHBOARD_TYPE_IDS,
  });

  nodeCount.value = built.nodeMeta.length;
  nodeMeta = built.nodeMeta;

  // Keep a stable copy of base colors for neighbor dimming
  basePointColors = built.pointColors;

  if (nodeCount.value === 0) {
    loading.value = false;
    return;
  }

  // --- Build legend ---
  const seenTypeIds = new Set<string>();
  const typeNameMap = new Map<string, string>(types.map((t) => [t.id, t.name]));

  for (const meta of nodeMeta) {
    if (!seenTypeIds.has(meta.typeId)) {
      seenTypeIds.add(meta.typeId);
    }
  }

  legend.value = Array.from(seenTypeIds).map((typeId) => ({
    typeId,
    name: typeNameMap.get(typeId) ?? typeId,
    color: rgbaArrayToCss(colorForType(typeId)),
  }));

  // --- Generate initial positions ---
  const spaceSize = 4096;
  const n = nodeMeta.length;
  const positions = new Float32Array(n * 2);
  for (let i = 0; i < n; i++) {
    positions[i * 2] = Math.random() * spaceSize;
    positions[i * 2 + 1] = Math.random() * spaceSize;
  }

  // --- Create cosmos graph ---
  try {
    if (!containerRef.value) return;

    // Серый фон в духе Eden-поверхностей, а не near-black `--background`.
    const bgColor = cssColorToRgba("var(--surface)");
    const mutedFgColor = cssColorToRgba("var(--muted-foreground)");
    // Full-alpha link color — opacity controlled via linkOpacity config
    const linkDefaultColor: [number, number, number, number] = [
      mutedFgColor[0],
      mutedFgColor[1],
      mutedFgColor[2],
      1,
    ];
    const fgColor = cssColorToRgba("var(--foreground)");

    graph = new Graph(containerRef.value, {
      backgroundColor: bgColor,
      linkDefaultColor,
      linkOpacity: settings.linkOpacity,
      pointDefaultSize: 4,
      renderHoveredPointRing: true,
      hoveredPointRingColor: fgColor,
      fitViewOnInit: true,
      scalePointsOnZoom: true,
      spaceSize,
      simulationGravity: settings.gravity,
      simulationRepulsion: settings.repulsion,
      simulationLinkSpring: 1.0,
      simulationLinkDistance: settings.linkDistance,
      simulationFriction: settings.friction,
      pointSizeScale: settings.pointSizeScale,
      curvedLinks: settings.curvedLinks,
      enableDrag: true,
      onPointMouseOver: (index, _pos, event) => {
        const meta = nodeMeta[index];
        if (!meta) return;
        const ev = event as MouseEvent | undefined;
        tooltip.value = {
          visible: true,
          x: (ev?.offsetX ?? 0) + 14,
          y: (ev?.offsetY ?? 0) + 14,
          text: meta.title,
        };
        // Neighbor highlight
        if (settings.highlightNeighbors && graph) {
          dimAllExcept(index);
        }
      },
      onPointMouseOut: () => {
        tooltip.value = { ...tooltip.value, visible: false };
        restoreColors();
      },
    });

    graph.setPointPositions(positions);
    graph.setPointColors(built.pointColors);
    graph.setPointSizes(built.pointSizes);
    graph.setLinks(built.links);
    graph.render();

    // Freeze initial state if settings say so
    if (settings.frozen) {
      graph.pause();
    } else if (settings.antiOverlap) {
      scheduleSettleAndRelax();
    }
  } catch (e) {
    console.warn("[my-cosmos] graph init failed", e);
    loading.value = false;
    return;
  }

  loading.value = false;
});

onBeforeUnmount(() => {
  if (settleTimer !== null) {
    clearTimeout(settleTimer);
    settleTimer = null;
  }
  graph?.destroy();
  graph = null;
});

// Satisfy type checker — DEFAULT_GRAPH_SETTINGS is imported for completeness
void DEFAULT_GRAPH_SETTINGS;
</script>

<style scoped>
.cosmos-root {
  position: fixed;
  inset: 0;
  background: var(--background);
  font-family: var(--font-sans);
  color: var(--foreground);
  overflow: hidden;
}

.cosmos-canvas {
  width: 100%;
  height: 100%;
}

/* Перетаскиваемая зона сверху. Высота совпадает с titleBarOverlay (36px),
   справа оставляем место под нативные контролы — они кликабельны, т.к. ОС
   рисует их поверх web-слоя. */
.cosmos-dragbar {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 36px;
  z-index: 5;
  -webkit-app-region: drag;
}

/* Loading overlay */
.cosmos-loading,
.cosmos-empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  pointer-events: none;
  color: var(--muted-foreground);
}

/* Empty state */
.cosmos-empty__heading {
  font-size: var(--kosmos-text-heading-size);
  font-weight: var(--kosmos-text-heading-weight);
  color: var(--foreground);
  margin: 0 0 var(--space-1);
}

.cosmos-empty__sub {
  font-size: var(--kosmos-text-caption-size);
  color: var(--muted-foreground);
  margin: 0;
}

/* Tooltip */
.cosmos-tooltip {
  position: absolute;
  pointer-events: none;
  background: var(--card);
  color: var(--foreground);
  border: 1px solid var(--border);
  border-radius: var(--radius-input, 8px);
  padding: 4px 8px;
  font-size: var(--kosmos-text-caption-size);
  box-shadow: var(--shadow-floating);
  white-space: nowrap;
  z-index: 10;
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Legend */
.cosmos-legend {
  position: absolute;
  left: var(--space-2);
  bottom: var(--space-2);
  background: color-mix(in srgb, var(--card) 88%, transparent);
  border: 1px solid var(--border);
  border-radius: var(--radius-card, 16px);
  padding: var(--space-1) var(--space-2);
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: var(--kosmos-text-caption-size);
  color: var(--foreground);
  box-shadow: var(--shadow-floating);
  max-height: 40vh;
  overflow: auto;
  backdrop-filter: blur(8px);
  z-index: 9;
}

.cosmos-legend__row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.cosmos-legend__dot {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

/* Settings panel — anchored above the gear button, bottom-right */
.cosmos-settings-panel {
  position: absolute;
  right: var(--space-2, 16px);
  bottom: calc(32px + var(--space-2, 16px) + var(--space-1, 8px));
  z-index: 11;
}

/* Gear button wrap — bottom-right corner */
.cosmos-gear-wrap {
  position: absolute;
  right: var(--space-2, 16px);
  bottom: var(--space-2, 16px);
  z-index: 11;
  -webkit-app-region: no-drag;
}
</style>

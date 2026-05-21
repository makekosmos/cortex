import DefaultTheme from "vitepress/theme";
import { nextTick, onMounted, watch } from "vue";
import { useRoute } from "vitepress";
import "./custom.css";

/*
 * Ленивая обработка mermaid:
 *  - VitePress (Shiki) рендерит ```mermaid ... ``` как code-block с классом
 *    `language-mermaid`. Мы перехватываем такие блоки и подменяем рендером
 *    через mermaid + svg-pan-zoom.
 *  - mermaid (2.8MB) и svg-pan-zoom грузятся ТОЛЬКО когда на странице реально
 *    есть блок диаграммы. На страницах без диаграмм цена нулевая.
 */

const MERMAID_THEME = {
  theme: "base",
  themeVariables: {
    darkMode: true,
    background: "#1c1c1c",
    primaryColor: "#2b2b46",
    primaryTextColor: "#fafafa",
    primaryBorderColor: "#3b3b66",
    lineColor: "#9b9bb2",
    secondaryColor: "#222230",
    tertiaryColor: "#1a1a1f",
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Display', Inter, sans-serif",
    fontSize: "13px",
  },
} as const;

let mermaidInited = false;

async function renderMermaid() {
  if (typeof window === "undefined") return;

  // Ищем блоки, оставленные Shiki: <div class="language-mermaid"><pre><code>...
  const blocks = Array.from(
    document.querySelectorAll<HTMLElement>(
      "div.language-mermaid:not([data-mermaid-rendered]), pre.mermaid:not([data-mermaid-rendered])",
    ),
  );
  if (blocks.length === 0) return;

  const mermaid = (await import("mermaid")).default;
  if (!mermaidInited) {
    mermaid.initialize({ startOnLoad: false, ...MERMAID_THEME });
    mermaidInited = true;
  }

  const { default: svgPanZoom } = await import("svg-pan-zoom");

  let counter = 0;
  for (const block of blocks) {
    const codeNode = block.querySelector("code") ?? block;
    const raw = (codeNode.textContent ?? "").trim();
    if (!raw) continue;

    const id = `mmd-${Date.now()}-${counter++}`;
    let svg: string;
    try {
      const result = await mermaid.render(id, raw);
      svg = result.svg;
    } catch (err) {
      console.warn("mermaid render failed", err);
      continue;
    }

    const wrap = document.createElement("div");
    wrap.className = "mermaid mermaid-rendered";
    wrap.setAttribute("data-mermaid-rendered", "1");
    wrap.innerHTML = svg;
    block.replaceWith(wrap);

    // pan/zoom
    const svgEl = wrap.querySelector<SVGSVGElement>("svg");
    if (svgEl) {
      wrap.classList.add("mermaid-pz");
      svgEl.setAttribute("width", "100%");
      svgEl.setAttribute("height", "100%");
      svgEl.style.maxWidth = "none";
      svgEl.style.maxHeight = "none";
      try {
        const pz = svgPanZoom(svgEl, {
          zoomEnabled: true,
          controlIconsEnabled: true,
          fit: true,
          center: true,
          minZoom: 0.4,
          maxZoom: 12,
          zoomScaleSensitivity: 0.3,
          dblClickZoomEnabled: true,
          mouseWheelZoomEnabled: true,
        });
        const onResize = () => {
          try {
            pz.resize();
            pz.fit();
            pz.center();
          } catch (_) {}
        };
        window.addEventListener("resize", onResize);
      } catch (e) {
        console.warn("svg-pan-zoom init failed", e);
      }
    }
  }
}

function schedule(fn: () => void) {
  if (typeof window === "undefined") return;
  const ric = (window as any).requestIdleCallback as
    | ((cb: () => void, opts?: { timeout: number }) => void)
    | undefined;
  if (ric) ric(fn, { timeout: 600 });
  else setTimeout(fn, 80);
}

export default {
  extends: DefaultTheme,
  setup() {
    const route = useRoute();
    const run = async () => {
      await nextTick();
      schedule(() => {
        void renderMermaid();
      });
    };
    onMounted(run);
    watch(() => route.path, run);
  },
};

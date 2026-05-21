// One-shot script: replace each "- **Result:** [ ]" in EXPTOTRY.md
// with concrete outcome from full sweep 2026-05-18.

import fs from "node:fs";
import path from "node:path";

const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..");
const exptotry = path.join(repoRoot, "EXPTOTRY.md");

const results = [
  // 1. RAM Experiments 1-10
  "❌ N/A — `affinity` option удалён в Electron 14+. Текущий Electron 41.1.0. Современный аналог — `WebContentsView` (Exp 5).",
  "❌ N/A — Electron 17+ имеет built-in V8 code cache в renderer'ах + Node 16.13+ `NODE_COMPILE_CACHE` для main. npm-пакет `v8-compile-cache` избыточен и может конфликтовать.",
  "✅ DONE (existing) — `shell/electron/preload.ts` 105 LOC, `extension-preload.ts` 159 LOC. Импорты минимальные: только `contextBridge`, `ipcRenderer` из electron. Нет lodash / тяжёлых deps.",
  "⏸️ DEFERRED → `docs-site/agents/manual-tests-pending.md`. Архитектурное изменение; применяется по сигналу «extension'ы переоткрываются заметно медленно».",
  "⏸️ DEFERRED → manual-tests-pending. 1-2 недели работы; применяется по сигналу RAM-bottleneck в production.",
  "❌ N/A — extensions грузятся через `loadFile()` из локального `dist/` (file://). Cache-savings 20-40% применимы только при HTTP-load. Cookies / localStorage shared partition тоже не нужен — extensions не используют cookies.",
  "✅ DONE 2026-05-18 — explicit `backgroundThrottling: true` в `settings-window.ts`, `install-extension-window.ts`, `dashboard-window.ts`. Launcher и extension windows на default (`true` в Electron 41). Effect: defensive correctness, гарантирует throttling при backgrounding не-критичных окон.",
  '✅ DONE 2026-05-18 — `"electronLanguages": ["en-US", "ru"]` в `shell/package.json` build. **Измерено:** installer 110.62 MB → **102.78 MB** (−7.84 MB, −7.1%); unpacked locales 46.38 MB → 1.64 MB (−44.74 MB, −96%); disk install footprint ~415 → 370.32 MB (−10.8%).',
  "❌ SKIP — anti-pattern по самому документу: ручной GC вызывает jank, `--expose-gc` увеличивает attack surface. Применять только при доказанной утечке после profiling. Сейчас нет такого signal.",
  "❌ N/A — Уже Electron 41.1.0 (effective 41.6.0 из bundle). Текущая стабильная линия. Upgrade на 42+ — отдельный proof loop с regression testing.",

  // 2. UI Latency Experiments 11-20
  "✅ DONE (existing) — 0 `sendSync` calls в codebase (grep verified). Всё IPC через `invoke()` / `handle()`.",
  "⚠️ PARTIAL — Command bus уже batch'ит через WS event broadcast (`commands_changed` агрегируется). Identified high-frequency IPC patterns отсутствуют. Применять reactively по profiling signal.",
  "🔍 NEEDS-MEASURE — `@kepler/ark` client обрабатывает каждое WS сообщение немедленно (нет rAF batching). Defer: применять если будет measurable render thrashing при peer sync events.",
  "✅ DONE (existing) — debounce уже в 4 файлах: `arrancador/CataloguePage.vue` (RAWG search 500ms), `delphi/autosave.ts`, `delphi/helpers/debounce.ts`, `eden/useSearch.ts`. Coverage достаточен для текущих flows.",
  "🔍 NEEDS-CASE — нет identified non-critical background задач конкурирующих за main thread. Применять reactively по INP signal.",
  "🔍 NEEDS-CASE — Scheduler API без явных prioritization needs — premature optimization. Defer.",
  "⏸️ DEFERRED — нет identified long tasks (>50ms) в современном Vue components codebase. Применять после Chrome DevTools Performance audit с reproducible jank.",
  "❌ N/A — нет canvas-рендеринга в приложении. Все extensions — declarative Vue DOM.",
  "❌ N/A — нет больших ArrayBuffer передач между процессами / workers.",
  "⏸️ DEFERRED — high effort, обходит context isolation guarantees. Нет identified use case прямой renderer↔renderer связи. Текущий путь main process — приемлемо для observability.",

  // 3. FPS Experiments 21-30
  "❌ N/A — `--enable-gpu-rasterization` уже **default** в Chromium 90+ / Electron 41 на Windows. Verify через `chrome://gpu` в DevTools если сомнения.",
  "❌ N/A — `d3d11` ANGLE backend уже **default** на Windows в Electron 41 (используется автоматически на DirectX 11+ GPU).",
  "⏸️ DEFERRED → manual-tests-pending. Subjective UX call между acrylic (realtime blur, DWM ~20-30% GPU) и Mica (static texture, ~5% GPU). Нужен GUI sample.",
  "✅ DONE (existing) — `transparent: false` в `shell/electron/main.ts:274` с explicit comment объясняющим почему (acrylic ignored if transparent:true). Settings/install/dashboard также opaque.",
  "🔍 NEEDS-PROFILE — нет identified first-frame stutter в анимациях. Применять селективно только после measured jank.",
  "⚠️ PARTIAL — большинство анимаций в `@kepler/visuals` уже на `transform`/`opacity` (DesktopChrome titlebar, transitions). Полный audit отложен до measured signal.",
  "⏸️ DEFERRED → manual-tests-pending. Триггер: пользователь сообщил о slow scroll при >300 todos / >500 objects.",
  "🔍 NEEDS-AUDIT — `backdrop-filter` найден в 9 файлах (Eden App.css 4 места, SearchOverlay, ObjectPropertyPicker, ContextMenu, CustomCaret, etc). Большинство на статичных modals/overlays — не на анимациях. Audit отложен.",
  "⏸️ DEFERRED — нет identified layout thrash. Performance pattern, применять reactively на bottleneck.",
  "✅ DONE 2026-05-18 — `contain: layout style` добавлен в `#app`/`.app-container` всех 4 extensions: `horologion/styles.css`, `arrancador/styles.css`, `delphi/global.css`, `eden/App.css`. Изолирует reflow scope без обрезки shadows (paint containment не выставлен).",

  // 4. Bundle Experiments 31-40
  "⚠️ PARTIAL — Rolldown defaults уже создают per-route code splitting (Horologion HomeView/SettingsView, Eden Editor.vue lazy). Manual `manualChunks` для vendor-vue — добавим reactively если bundle visualizer покажет проблему. Текущие bundles: Horologion 132KB / Arrancador 108KB / Eden main 353KB (gzip 112KB).",
  '❌ SKIP — Rolldown defaults уже агрессивные. `"sideEffects": false` в package.json не выставлено и не должно — Tailwind CSS injections + Vue plugins имеют side effects. Дополнительный treeshake config рискует сломать build.',
  "❌ N/A — `vite-plugin-electron` уже external'ит `electron` автоматически. Native modules (sqlite, etc) — нет в проекте, всё в Rust backend через WS.",
  "❌ N/A — `moment` / `lodash` отсутствуют в зависимостях (только `dayjs` в `docs-site/`, не в runtime extension'ов). Verified через grep package.json.",
  "⏸️ DEFERRED — NSIS уже deflate-сжимает assets. Brotli polyfill в Electron protocol handler — complex, marginal win для local file:// loading.",
  "⏸️ DEFERRED — complex build script, marginal win (5-10% startup). Trigger: profile startup > 1s sustained.",
  "❌ N/A — `differentialPackage: true` — default behavior modern `electron-builder` + NSIS. autoUpdater использует `.blockmap` файлы рядом с installer'ом (см. `release/Kepler Setup 0.1.9.exe.blockmap`).",
  "✅ DONE (existing) — уже Bun (`bun.lockb` в repo, `bun install` 16× быстрее npm).",
  '✅ DONE 2026-05-18 — `"incremental": true` + `"tsBuildInfoFile"` в `shell/tsconfig.json` и `packages/ark/tsconfig.json`. **Измерено:** cold typecheck 1650ms → warm 1177ms (−28%).',
  "✅ DONE (existing) — Vite default `sourcemap: false` в production build уже выставлен (нет в `vite.config.mjs` overrides). Production bundles без `.map` файлов.",

  // 5. Multi-window Experiments 41-42
  "⏸️ DEFERRED → manual-tests-pending. Архитектурное изменение (Exp 4 + per-extension lifecycle), trigger по UX signal.",
  "⚠️ PARTIAL — IPC batching не нужен в текущем codebase: command bus батчит через event broadcast, ARK requests low-frequency (per-user-action). Применять reactively.",

  // 6. Windows Experiments 43-44
  "❌ N/A для use case — Arrancador запускает игры через **внешний** Steam launcher (`steam://rungameid/<id>`). Игра рендерится в собственном процессе. Force high-perf GPU для renderer'а Arrancador не приносит value (Vue UI lightweight), но добавляет battery drain. Не применяем.",
  "❌ N/A — Launcher уже hides on blur (`mainWindow.on('blur', () => hide())`). `alwaysOnTop` активен только когда launcher показан (короткие interval'ы). Settings/install/dashboard windows уже `alwaysOnTop: false`. Overhead в текущей архитектуре минимальный.",

  // 8. Vue 3.6 Experiments 45-48
  "⚠️ PARTIAL — `vaporInterop: true` уже включён в `vite.config.mjs` для shell + extensions. Полная миграция компонентов на `<script setup vapor>` — отдельная задача (audit leaf components + регрессия risks в Vue 3.6 **beta**.9).",
  "⏸️ DEFERRED → manual-tests-pending. Semantic safe (`updateTodo` reassigns array, нет `todos.value[i].x = y` мутаций verified), но Pinia + Vue 3.6 vapor interaction требует UI smoke.",
  "⏸️ DEFERRED → manual-tests-pending. Применять только когда списки > 100 items с **measured** render bottleneck. Premature на текущих масштабах.",
  "⏸️ DEFERRED — 11 файлов с template refs (`ref(null)` / `ref<HTMLElement>`). `useTemplateRef` — **pure DX** win (нет performance difference per doc). Не оправдывает риск массовой правки в Vue 3.6 beta.",

  // 9. TipTap / ProseMirror Experiments 49-54
  "🔍 NEEDS-AUDIT — Eden TipTap plugins (custom extensions для note links / mentions) требуют code audit на decoration rebuild patterns. Применять при measured input latency в больших документах.",
  "⏸️ DEFERRED — Eden использует lowlight через TipTap CodeBlockLowlight. Migration на Shiki — большая работа (новые themes, theme loading, async patterns). Низкий приоритет: типичные note-документы не содержат больших code blocks.",
  "🔍 NEEDS-AUDIT — Eden ProseMirror integration. Большинство operations через TipTap commands API (auto-batched). Manual `state.tr` chains — отдельный audit отложен.",
  "✅ DONE (existing) — `defineAsyncComponent` для Editor.vue в Phase 6.0.A (см. `extensions/eden/`). Main bundle Eden 353KB (gzip 112KB), editor chunk 1.36MB lazy.",
  "🔍 NEEDS-AUDIT — Eden custom node views для note-links / typed-notes — проверить `destroy()` cleanup. Audit отложен до measured memory leak signal.",
  "⏸️ DEFERRED — schema optimization — architectural change. Не оправдан без measured doc-size bottleneck (текущие notes — small markdown-style docs).",
];

const text = fs.readFileSync(exptotry, "utf8");
const marker = "- **Result:** [ ]";
const parts = text.split(marker);

if (parts.length - 1 !== results.length) {
  console.error(
    `[fill-results] mismatch: found ${parts.length - 1} markers, ` +
      `but ${results.length} results provided.`,
  );
  process.exit(1);
}

let out = parts[0];
for (let i = 0; i < results.length; i++) {
  out += `- **Result:** ${results[i]}`;
  out += parts[i + 1];
}

fs.writeFileSync(exptotry, out, "utf8");
console.log(`✅ filled ${results.length} Result entries in EXPTOTRY.md`);

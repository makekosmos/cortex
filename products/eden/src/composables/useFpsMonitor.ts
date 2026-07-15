import { onMounted, onUnmounted, readonly, shallowRef } from "vue";

export type FpsDropCause = "long-task" | "page-hidden" | "unknown";

export interface FpsDropRecord {
  at: string;
  fps: number;
  expectedFps: number;
  cause: FpsDropCause;
  longestFrameMs: number;
  longTaskDurationMs?: number;
}

const MAX_DROP_RECORDS = 100;
const DROP_COOLDOWN_MS = 2_000;

export function shouldRecordFpsDrop(fps: number, expectedFps: number): boolean {
  return expectedFps >= 30 && fps < expectedFps * 0.75;
}

export function classifyFpsDrop(hidden: boolean, longTaskDurationMs: number): FpsDropCause {
  if (hidden) return "page-hidden";
  if (longTaskDurationMs > 0) return "long-task";
  return "unknown";
}

export function useFpsMonitor() {
  const fps = shallowRef(0);
  const drops = shallowRef<readonly FpsDropRecord[]>([]);
  let animationFrame = 0;
  let observer: PerformanceObserver | null = null;
  let sampleStartedAt = 0;
  let previousFrameAt = 0;
  let frameCount = 0;
  let expectedFps = 0;
  let longestFrameMs = 0;
  let longTaskDurationMs = 0;
  let lastDropAt = -Infinity;

  function recordDrop(now: number, currentFps: number): void {
    if (now - lastDropAt < DROP_COOLDOWN_MS || !shouldRecordFpsDrop(currentFps, expectedFps))
      return;

    lastDropAt = now;
    const record: FpsDropRecord = {
      at: new Date().toISOString(),
      fps: currentFps,
      expectedFps,
      cause: classifyFpsDrop(document.hidden, longTaskDurationMs),
      longestFrameMs: Math.round(longestFrameMs),
      ...(longTaskDurationMs > 0 && { longTaskDurationMs: Math.round(longTaskDurationMs) }),
    };
    drops.value = [...drops.value.slice(-(MAX_DROP_RECORDS - 1)), record];
    window.__edenFpsDrops = drops.value;
    console.warn("[eden:perf] FPS drop", record);
  }

  function tick(now: number): void {
    if (sampleStartedAt === 0) sampleStartedAt = now;
    if (previousFrameAt > 0) longestFrameMs = Math.max(longestFrameMs, now - previousFrameAt);
    previousFrameAt = now;
    frameCount += 1;

    const elapsed = now - sampleStartedAt;
    if (elapsed >= 1_000) {
      const currentFps = Math.round((frameCount * 1_000) / elapsed);
      fps.value = currentFps;
      if (!document.hidden) expectedFps = Math.max(expectedFps, currentFps);
      recordDrop(now, currentFps);
      sampleStartedAt = now;
      frameCount = 0;
      longestFrameMs = 0;
      longTaskDurationMs = 0;
    }

    animationFrame = window.requestAnimationFrame(tick);
  }

  onMounted(() => {
    window.__edenFpsDrops = drops.value;
    if ("PerformanceObserver" in window) {
      try {
        observer = new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) {
            longTaskDurationMs = Math.max(longTaskDurationMs, entry.duration);
          }
        });
        observer.observe({ type: "longtask", buffered: true });
      } catch {
        observer = null;
      }
    }
    animationFrame = window.requestAnimationFrame(tick);
  });

  onUnmounted(() => {
    window.cancelAnimationFrame(animationFrame);
    observer?.disconnect();
  });

  return { fps: readonly(fps), drops: readonly(drops) };
}

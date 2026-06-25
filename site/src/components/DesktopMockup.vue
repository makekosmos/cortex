<script setup lang="ts">
// Desktop mockup chrome — 16:9 frame с macOS menubar сверху. Внутри
// (slot default) рендерится контент — например <LauncherView />.
// Вдохновлено raycast.com homepage frame (sample/raycaststuff/page.html).

interface Props {
  /** Имя приложения справа от 🍎 в menubar'е. */
  appName?: string;
  /** Дата/время справа. */
  clock?: string;
}

withDefaults(defineProps<Props>(), {
  appName: "Kepler",
  clock: "Mon May 18  9:41",
});
</script>

<template>
  <div class="frame-outer">
    <div class="frame-inner">
      <!-- Subtle glow в фоне как у Raycast — radial gradient + slight texture -->
      <div class="bg-glow" aria-hidden="true" />

      <!-- macOS menubar -->
      <div class="menu-bar">
        <div class="menu-left">
          <svg
            class="apple"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden="true"
          >
            <path
              d="M17.05 12.04q-.05-3.42 2.81-5.18-1.6-2.27-4.62-2.42-2.86-.3-6.05 3.36-1.16-1.31-2.27-1.31-1.42 0-2.83 1.05Q1.13 9.91 1.5 14.5q.35 3.86 2.18 6.42 1.8 2.5 3.6 2.5 1.16 0 2.34-.96 1.18-.97 2.24-.97 1.05 0 2.23.97t2.32.97q1.8 0 3.57-2.5 1.51-2.16 2.07-4.83-2.99-1.21-3.99-4.06m-2.97-7.27Q15.91 2.5 15.7.5q-1.9.16-3.34 1.55-1.44 1.4-1.46 3.18 1.85.16 3.18-.46"
            />
          </svg>
          <span class="menu-item menu-item--app">{{ appName }}</span>
          <span class="menu-item">File</span>
          <span class="menu-item">Edit</span>
          <span class="menu-item">View</span>
          <span class="menu-item">Window</span>
          <span class="menu-item">Help</span>
        </div>

        <div class="menu-right">
          <svg
            class="status-icon"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M2 12a10 10 0 1 0 20 0a10 10 0 1 0 -20 0M12 8l4 4M12 8v8M12 8l-4 4" />
          </svg>
          <svg
            class="status-icon"
            width="16"
            height="14"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden="true"
          >
            <path
              d="M12 3C7 3 2.7 6.1 1 10.5c1.7 4.4 6 7.5 11 7.5s9.3-3.1 11-7.5C21.3 6.1 17 3 12 3m0 12.5c-2.8 0-5-2.2-5-5s2.2-5 5-5 5 2.2 5 5-2.2 5-5 5m0-8a3 3 0 1 0 0 6 3 3 0 0 0 0-6"
            />
          </svg>
          <svg
            class="status-icon battery"
            width="22"
            height="14"
            viewBox="0 0 26 12"
            fill="none"
            stroke="currentColor"
            stroke-width="1.2"
            aria-hidden="true"
          >
            <rect x="0.6" y="0.6" width="22" height="10.8" rx="2.4" />
            <rect x="2" y="2" width="17" height="8" rx="1" fill="currentColor" />
            <rect x="23.5" y="3.5" width="1.5" height="5" rx="0.5" fill="currentColor" />
          </svg>
          <span class="menu-clock">{{ clock }}</span>
        </div>
      </div>

      <!-- Content area: centered slot -->
      <div class="content-area">
        <slot />
      </div>
    </div>
  </div>
</template>

<style scoped>
.frame-outer {
  width: 100%;
  max-width: 1200px;
  aspect-ratio: 16 / 9;
  padding: 6px;
  border-radius: 22px;
  background: linear-gradient(180deg, oklch(0.22 0 0) 0%, oklch(0.16 0 0) 100%);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.05) inset,
    0 60px 120px rgba(0, 0, 0, 0.55),
    0 16px 40px rgba(0, 0, 0, 0.4);
}

.frame-inner {
  position: relative;
  width: 100%;
  height: 100%;
  border-radius: 16px;
  background: oklch(0.11 0 0);
  border: 1px solid oklch(0.2 0 0);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.bg-glow {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background:
    radial-gradient(
      ellipse at 70% 75%,
      color-mix(in srgb, oklch(0.4 0.18 25) 18%, transparent) 0%,
      transparent 55%
    ),
    radial-gradient(
      ellipse at 20% 30%,
      color-mix(in srgb, oklch(0.4 0.15 250) 10%, transparent) 0%,
      transparent 60%
    );
}

/* --- macOS menubar --- */
.menu-bar {
  position: relative;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 18px;
  height: 28px;
  flex-shrink: 0;
  color: color-mix(in srgb, var(--foreground) 90%, transparent);
  font-size: 13px;
  font-weight: 400;
  font-family: var(--font-sans);
  letter-spacing: -0.005em;
  user-select: none;
}

.menu-left,
.menu-right {
  display: flex;
  align-items: center;
  gap: 14px;
}

.apple {
  color: color-mix(in srgb, var(--foreground) 92%, transparent);
  position: relative;
  top: -1px;
}

.menu-item {
  white-space: nowrap;
}
.menu-item--app {
  font-weight: 600;
}

.status-icon {
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
}
.battery {
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
}

.menu-clock {
  font-variant-numeric: tabular-nums;
  color: color-mix(in srgb, var(--foreground) 95%, transparent);
  letter-spacing: 0.01em;
}

/* --- content area --- */
.content-area {
  position: relative;
  z-index: 1;
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 5%;
  min-height: 0;
}

/* Layout инструкции для slot контента: ограничен по высоте 76% area,
   шириной 60% (или max 720 для launcher). */
.content-area :deep(> *) {
  width: 100%;
  max-width: 720px;
  height: 78%;
  max-height: 460px;
  border-radius: 12px;
  overflow: hidden;
  border: 1px solid color-mix(in srgb, var(--foreground) 10%, transparent);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 24px 60px rgba(0, 0, 0, 0.55),
    0 8px 20px rgba(0, 0, 0, 0.4);
}

@media (max-width: 900px) {
  .frame-outer {
    padding: 4px;
    border-radius: 16px;
  }
  .frame-inner {
    border-radius: 12px;
  }
  .menu-bar {
    padding: 0 10px;
    font-size: 11px;
    gap: 8px;
    height: 24px;
  }
  .menu-left,
  .menu-right {
    gap: 8px;
  }
  /* На узких экранах прячем все пункты кроме app name и часов */
  .menu-item:not(.menu-item--app) {
    display: none;
  }
  .status-icon {
    display: none;
  }
  .content-area {
    padding: 0 4%;
  }
}
</style>

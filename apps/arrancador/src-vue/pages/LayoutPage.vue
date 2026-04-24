<script setup lang="ts">
import { Menu, X } from "lucide-vue-next";
import { computed, onMounted, shallowRef, watch } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import AppSidebar from "../components/AppSidebar.vue";
import AppSpotlight from "../components/AppSpotlight.vue";
import AppTitlebar from "../components/AppTitlebar.vue";
import ToastViewport from "../components/ToastViewport.vue";
import { useSidebarConfig } from "../composables/useSidebarConfig";
import { useGamesStore } from "../stores/games";

const route = useRoute();
const router = useRouter();
const gamesStore = useGamesStore();
const { sidebarConfig, persistSidebarConfig } = useSidebarConfig();

const isMobileMenuOpen = shallowRef(false);
const historyIndex = shallowRef(0);
const maxHistoryIndex = shallowRef(0);

function readBrowserHistoryIndex() {
  if (typeof window === "undefined") {
    return 0;
  }

  const state = window.history.state as { position?: unknown; idx?: unknown } | null;
  if (typeof state?.position === "number") {
    return state.position;
  }

  return typeof state?.idx === "number" ? state.idx : 0;
}

const canGoBack = computed(() => historyIndex.value > 0);
const canGoForward = computed(() => historyIndex.value < maxHistoryIndex.value);

watch(
  () => route.fullPath,
  () => {
    isMobileMenuOpen.value = false;
    const nextHistoryIndex = readBrowserHistoryIndex();
    historyIndex.value = nextHistoryIndex;
    maxHistoryIndex.value = Math.max(maxHistoryIndex.value, nextHistoryIndex);
  },
  { immediate: true },
);

function setSidebarHidden(hidden: boolean) {
  persistSidebarConfig({
    ...sidebarConfig.value,
    hidden,
  });
}

function handleSidebarConfigChange(nextConfig: { width: number; hidden: boolean }) {
  persistSidebarConfig(nextConfig);
}

function navigateBack() {
  if (!canGoBack.value) {
    return;
  }

  router.go(-1);
}

function navigateForward() {
  if (!canGoForward.value) {
    return;
  }

  router.go(1);
}

onMounted(async () => {
  if (gamesStore.games.length === 0) {
    await gamesStore.refreshGames();
  }
});
</script>

<template>
  <div class="kepler-desktop-chrome flex h-screen min-h-0 flex-col overflow-hidden bg-background text-foreground">
    <div class="hidden lg:block">
      <AppTitlebar
        :sidebar-hidden="sidebarConfig.hidden"
        :can-go-back="canGoBack"
        :can-go-forward="canGoForward"
        @toggle-sidebar="setSidebarHidden(!sidebarConfig.hidden)"
        @back="navigateBack"
        @forward="navigateForward"
      />
    </div>

    <div class="kepler-desktop-chrome__body">
      <aside class="kepler-desktop-chrome__sidebar hidden lg:flex">
        <AppSidebar
          :hidden="sidebarConfig.hidden"
          :config="sidebarConfig"
          :show-toggle="false"
          :reserve-top-inset="false"
          @toggle-hidden="setSidebarHidden(!sidebarConfig.hidden)"
          @config-change="handleSidebarConfigChange"
        />
      </aside>

      <div class="kepler-desktop-chrome__content">
        <div
          class="fixed top-0 left-0 right-0 z-50 flex h-14 items-center justify-between border-b px-4 backdrop-blur-xl lg:hidden"
          style="
            border-color: var(--border);
            background: color-mix(in srgb, var(--sidebar-bg) 90%, transparent);
          "
        >
          <span class="text-sm font-[510] tracking-[-0.01em]">Arrancador</span>
          <div class="flex items-center gap-2">
            <AppSpotlight :enable-shortcut="false" trigger-class-name="h-9 px-3" />
            <button
              type="button"
              class="inline-flex h-9 w-9 items-center justify-center rounded-md border border-border/60 bg-card/70 transition-colors hover:bg-card"
              :aria-label="isMobileMenuOpen ? 'Закрыть меню' : 'Открыть меню'"
              :aria-expanded="isMobileMenuOpen"
              @click="isMobileMenuOpen = !isMobileMenuOpen"
            >
              <X v-if="isMobileMenuOpen" class="h-5 w-5" />
              <Menu v-else class="h-5 w-5" />
            </button>
          </div>
        </div>

        <Teleport to="body">
          <div
            v-if="isMobileMenuOpen"
            class="fixed inset-0 z-[95] bg-black/70 backdrop-blur-sm lg:hidden"
            @click="isMobileMenuOpen = false"
          >
            <div
              class="absolute inset-y-0 left-0 w-[320px] max-w-[86vw] border-r border-border/70 bg-[var(--sidebar-bg)]"
              @click.stop
            >
              <AppSidebar
                mobile
                :hidden="false"
                :config="sidebarConfig"
                :show-toggle="false"
                :reserve-top-inset="false"
                :enable-toggle-shortcut="false"
                @config-change="handleSidebarConfigChange"
              />
            </div>
          </div>
        </Teleport>

        <section
          class="kepler-desktop-content-surface flex min-h-0 min-w-0 flex-1 flex-col"
          :style="{
            '--kepler-content-padding-top': '0',
            '--kepler-content-padding-inline': '0',
            '--kepler-content-padding-bottom': '0',
            '--kepler-content-radius-top-left': sidebarConfig.hidden ? '0px' : '16px',
            '--kepler-content-radius-bottom-left': '0px',
            '--kepler-content-border-color': 'var(--border)',
            '--kepler-content-border-left-color': sidebarConfig.hidden ? 'transparent' : 'var(--border)',
          }"
        >
          <main class="min-w-0 flex-1 overflow-auto pt-14 lg:pt-0">
            <div class="arrancador-page-shell">
              <RouterView />
            </div>
          </main>
        </section>
      </div>
    </div>

    <ToastViewport />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { RouterView, useRoute, useRouter } from "vue-router";
import { Settings, ArrowLeft } from "lucide-vue-next";

const route = useRoute();
const router = useRouter();

const isSettingsRoute = computed(() => route.path === "/settings");

function openSettings() {
    void router.push("/settings");
}

function goBack() {
    void router.push("/");
}

// ---------------------------------------------------------------------------
// ARK connection status — точка-индикатор в topbar.
// Каждые 10 секунд (и при mount) дёргаем дешёвую операцию `list_object_types`,
// успех → connected, ошибка → error. Используем такой же визуал как в Delphi
// extension'е (8px dot, 32x32 button, oklch tokens из @kepler/visuals).
// ---------------------------------------------------------------------------

type ArkStatus = "connected" | "connecting" | "error";

const arkStatus = ref<ArkStatus>("connecting");

const arkDotClass = computed(() => `dot dot--${arkStatus.value}`);

const arkStatusMessage = computed(() => {
    switch (arkStatus.value) {
        case "connected":
            return "ARK подключен";
        case "connecting":
            return "Подключение к ARK…";
        case "error":
        default:
            return "ARK недоступен";
    }
});

let probeTimer: ReturnType<typeof setInterval> | null = null;

async function probeArk(): Promise<void> {
    const kepler = window.kepler;
    if (!kepler) {
        arkStatus.value = "error";
        return;
    }
    try {
        await kepler.ark.request("list_object_types");
        arkStatus.value = "connected";
    } catch {
        arkStatus.value = "error";
    }
}

onMounted(() => {
    void probeArk();
    probeTimer = setInterval(() => {
        void probeArk();
    }, 10_000);
});

onBeforeUnmount(() => {
    if (probeTimer) {
        clearInterval(probeTimer);
        probeTimer = null;
    }
});
</script>

<template>
    <div class="app">
        <header class="topbar">
            <button v-if="isSettingsRoute" type="button" class="iconbtn" title="Назад" @click="goBack">
                <ArrowLeft :size="16" :stroke-width="1.7" />
            </button>
            <div class="appname">{{ isSettingsRoute ? "Настройки помодоро" : "Horologion" }}</div>
            <div class="trail">
                <button
                    type="button"
                    class="ark-status-btn"
                    :title="arkStatusMessage"
                    :aria-label="arkStatusMessage"
                >
                    <span :class="arkDotClass" />
                </button>
                <button v-if="!isSettingsRoute" type="button" class="iconbtn" title="Настройки" @click="openSettings">
                    <Settings :size="16" :stroke-width="1.7" />
                </button>
            </div>
        </header>

        <main class="content">
            <RouterView />
        </main>
    </div>
</template>

<style scoped>
.app {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
}

.topbar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: 44px;
    padding: 0 0.75rem;
    background: var(--sidebar-bg);
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
    -webkit-app-region: drag;
    flex-shrink: 0;
}

.appname {
    flex: 1;
    font-size: 0.8125rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: color-mix(in srgb, var(--sidebar-foreground) 55%, transparent);
    line-height: 1;
}

.trail {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    -webkit-app-region: no-drag;
}

.iconbtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    background: transparent;
    border: none;
    color: color-mix(in srgb, var(--sidebar-foreground) 55%, transparent);
    border-radius: 8px;
    cursor: pointer;
    -webkit-app-region: no-drag;
    transition:
        color 120ms cubic-bezier(0.2, 0, 0, 1),
        background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.iconbtn:hover {
    color: var(--sidebar-foreground);
    background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
}

/* ARK status indicator — single-line dot, без popover'а.
   Совпадает с Delphi extension'ом: 32x32 transparent button + 8px dot,
   цвета из @kepler/visuals (--status-success / --destructive). */
.ark-status-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border: none;
    background: transparent;
    border-radius: 8px;
    cursor: default;
    flex-shrink: 0;
    color: color-mix(in srgb, var(--sidebar-foreground) 55%, transparent);
    -webkit-app-region: no-drag;
    transition:
        color 120ms cubic-bezier(0.2, 0, 0, 1),
        background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.ark-status-btn:hover {
    color: var(--sidebar-foreground);
    background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
}

.dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: currentColor;
}

.dot--connected {
    color: var(--status-success);
}

.dot--connecting {
    color: oklch(0.75 0.14 75);
}

.dot--error {
    color: var(--destructive);
}

.content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 0.5rem;
    scrollbar-gutter: stable both-edges;
}

.content::-webkit-scrollbar {
    width: 8px;
}

.content::-webkit-scrollbar-track {
    background: transparent;
}

.content::-webkit-scrollbar-thumb {
    background: transparent;
}
</style>

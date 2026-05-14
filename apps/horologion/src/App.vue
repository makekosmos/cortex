<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import { RouterView, useRoute } from "vue-router";
import { Settings } from "lucide-vue-next";
import { DesktopChrome, DesktopContentSurface } from "@kepler/visuals";
import type { ArkStatus, ArkConnectionStatus } from "@shared/ipc-types";
import SettingsView from "./views/SettingsView.vue";

const route = useRoute();
// На route '/settings' (открыто отдельным окном) — рендерим только SettingsView,
// без основной chrome'ы. Окно создаётся в Electron main процессе через
// `window.horologion.settings.open()`.
const isSettingsWindow = computed(() => route.path === "/settings");

const errMsg = ref<string | null>(null);

// --- ARK status ---
const arkStatus = ref<ArkConnectionStatus>("connecting");
const arkStatusMessage = ref<string>("Подключение к ARK…");
const arkDbPath = ref<string>("");
let arkPollHandle: ReturnType<typeof setInterval> | null = null;

async function refreshArkStatus() {
    try {
        const s: ArkStatus = await window.horologion.ark.status();
        arkStatus.value = s.status;
        arkDbPath.value = s.dbPath ?? "";
        arkStatusMessage.value =
            s.status === "connected"
                ? `ARK подключен${s.dbPath ? ` · ${s.dbPath}` : ""}`
                : s.status === "connecting"
                    ? "Подключение к ARK…"
                    : `Ошибка ARK: ${s.message ?? "unknown"}`;
    } catch (e) {
        arkStatus.value = "error";
        arkStatusMessage.value = `Не удалось получить статус: ${e instanceof Error ? e.message : e}`;
    }
}

// Класс точки соответствует тону подключения: success / warning / danger.
const arkDotClass = computed(() => `dot dot--${arkStatus.value}`);

onMounted(() => {
    void refreshArkStatus();
    arkPollHandle = setInterval(refreshArkStatus, 2000);
});

onBeforeUnmount(() => {
    if (arkPollHandle) clearInterval(arkPollHandle);
});

async function openSettingsWindow() {
    try {
        await window.horologion.settings.open();
    } catch (e) {
        console.error("Failed to open settings window:", e);
    }
}
</script>

<template>
    <!-- Settings-окно — отдельное Electron BrowserWindow, но использует тот же
         DesktopChrome + DesktopContentSurface чтобы стиль совпадал с main. -->
    <DesktopChrome v-if="isSettingsWindow" platform="windows">
        <template #titlebar-leading>
            <div class="appname">Настройки помодоро</div>
        </template>
        <DesktopContentSurface :padding-top="'0'" :padding-inline="'0'" :padding-bottom="'0'" :scrollable="false">
            <main class="content content--settings">
                <SettingsView />
            </main>
        </DesktopContentSurface>
    </DesktopChrome>

    <DesktopChrome v-else platform="windows">
        <!-- LEFT: app name -->
        <template #titlebar-leading>
            <div class="appname">Horologion</div>
        </template>

        <!-- RIGHT: ARK status + settings (before window controls) -->
        <template #titlebar-trailing>
            <div class="trail">
                <button type="button" class="iconbtn iconbtn--titlebar" :title="arkStatusMessage"
                    :aria-label="arkStatusMessage">
                    <span :class="arkDotClass" />
                </button>
                <button type="button" class="iconbtn iconbtn--titlebar" title="Настройки" @click="openSettingsWindow">
                    <Settings :size="16" :stroke-width="1.7" />
                </button>
            </div>
        </template>

        <DesktopContentSurface :padding-top="'0'" :padding-inline="'0'" :padding-bottom="'0'" :scrollable="false">
            <main class="content">
                <div v-if="errMsg" class="errbar">⚠ {{ errMsg }}</div>
                <RouterView />
            </main>
        </DesktopContentSurface>

    </DesktopChrome>
</template>

<style scoped>
.content.content--settings {
    /* Под settings-окно: 1rem inset со всех сторон. */
    padding: 1rem 0;
}

/* --- titlebar leading: app name --- */
.appname {
    display: inline-flex;
    align-items: center;
    height: 32px;
    font-size: 0.8125rem;
    font-weight: 600;
    letter-spacing: 0.01em;
    color: color-mix(in srgb, var(--sidebar-foreground) 55%, transparent);
    padding: 0 0.5rem;
    line-height: 1;
}

/* --- titlebar trailing: status + settings --- */
.trail {
    display: flex;
    align-items: center;
    gap: 0.25rem;
}

.iconbtn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    background: transparent;
    border: none;
    color: color-mix(in srgb, var(--sidebar-foreground) 55%, transparent);
    border-radius: 6px;
    cursor: pointer;
    text-decoration: none;
    transition:
        color 120ms cubic-bezier(0.2, 0, 0, 1),
        background-color 120ms cubic-bezier(0.2, 0, 0, 1);
}

.iconbtn:hover {
    color: var(--sidebar-foreground);
    background: color-mix(in srgb, var(--sidebar-foreground) 8%, transparent);
}

.iconbtn--active {
    color: var(--accent);
}

.iconbtn--titlebar {
    width: 32px;
    height: 32px;
    border-radius: 10px;
    flex-shrink: 0;
}

/* ARK status dot — рендерится внутри обычной .iconbtn--titlebar кнопки. */
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

/* --- content (скролл живёт здесь) --- */
.content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    /* Левый паддинг 0 — содержимое HomeView само раскладывает секции на всю
     ширину (.home__pomo сам себе фон+border). Только справа компенсируем
     8px scrollbar-gutter. */
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

.errbar {
    background: color-mix(in srgb, var(--destructive) 22%, var(--background));
    color: color-mix(in srgb, var(--destructive) 95%, var(--foreground));
    border: 1px solid color-mix(in srgb, var(--destructive) 40%, transparent);
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    margin: 0.75rem 1rem 0 1rem;
    font-size: 0.8125rem;
}
</style>

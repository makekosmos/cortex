<script setup lang="ts">
import { computed } from "vue";
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
</script>

<template>
    <div class="app">
        <header class="topbar">
            <button v-if="isSettingsRoute" type="button" class="iconbtn" title="Назад" @click="goBack">
                <ArrowLeft :size="16" :stroke-width="1.7" />
            </button>
            <div class="appname">{{ isSettingsRoute ? "Настройки помодоро" : "Horologion" }}</div>
            <div class="trail">
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

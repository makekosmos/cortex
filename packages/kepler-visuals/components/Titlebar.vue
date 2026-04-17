<script setup lang="ts">
import { computed } from "vue";

export type TitlebarPlatform = "mac" | "windows" | "linux";

interface Props {
    platform?: TitlebarPlatform;
    title?: string;
}

const props = withDefaults(defineProps<Props>(), {
    platform: "windows",
    title: undefined,
});

const titlebarClasses = computed(() => [
    "kepler-titlebar",
    `kepler-titlebar--${props.platform}`,
]);
</script>

<template>
    <header :class="titlebarClasses">
        <div class="kepler-titlebar__leading">
            <slot name="leading" />
        </div>

        <div class="kepler-titlebar__center">
            <slot name="center">
                <span v-if="title" class="kepler-titlebar__title">{{ title }}</span>
            </slot>
        </div>

        <div class="kepler-titlebar__trailing">
            <slot name="trailing" />
        </div>
    </header>
</template>

<style scoped>
.kepler-titlebar {
    --kepler-titlebar-height: 36px;
    --kepler-titlebar-control-size: 32px;
    --kepler-titlebar-control-radius: 10px;
    position: relative;
    z-index: 20;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 0.75rem;
    height: var(--kepler-titlebar-height);
    min-height: var(--kepler-titlebar-height);
    padding: 0 0.75rem;
    background: var(--sidebar-bg);
    color: var(--sidebar-foreground);
    -webkit-app-region: drag;
    user-select: none;
}

.kepler-titlebar--mac {
    padding-left: var(--kepler-mac-traffic-light-left-safe-area, 92px);
}

.kepler-titlebar--windows {
    padding-right: var(--kepler-windows-controls-safe-area, 156px);
}

.kepler-titlebar__leading,
.kepler-titlebar__center,
.kepler-titlebar__trailing {
    min-width: 0;
    display: flex;
    align-items: center;
    min-height: 100%;
    gap: 0.5rem;
}

.kepler-titlebar__leading {
    justify-content: flex-start;
}

.kepler-titlebar__center {
    justify-content: center;
}

.kepler-titlebar__trailing {
    justify-content: flex-end;
}

.kepler-titlebar__title {
    color: color-mix(in srgb, var(--sidebar-foreground) 82%, transparent);
    font-size: 0.8125rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
}

.kepler-titlebar :deep(button),
.kepler-titlebar :deep(a),
.kepler-titlebar :deep(input),
.kepler-titlebar :deep(select),
.kepler-titlebar :deep(textarea),
.kepler-titlebar :deep([role="button"]) {
    -webkit-app-region: no-drag;
}
</style>

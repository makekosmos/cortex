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
    --kepler-titlebar-inline-padding: 0.75rem;
    --kepler-titlebar-vertical-padding: 0.375rem;
    box-sizing: border-box;
    position: relative;
    /* Тайтлбар всегда поверх Modal backdrop/panel (Modal = 9000) и любых
       app-overlay'ов: пользователь должен видеть/нажимать наши leading
       (sidebar toggle, app name) и trailing (status, settings) даже при
       открытом модальном окне. Native window controls (titleBarOverlay)
       и так выше любого CSS-слоя. */
    z-index: 10000;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 0.75rem;
    height: calc(var(--kepler-titlebar-height) + (var(--kepler-titlebar-vertical-padding) * 2));
    min-height: calc(var(--kepler-titlebar-height) + (var(--kepler-titlebar-vertical-padding) * 2));
    padding:
        var(--kepler-titlebar-vertical-padding)
        var(--kepler-titlebar-inline-padding);
    background: var(--sidebar-bg);
    color: var(--sidebar-foreground);
    -webkit-app-region: drag;
    user-select: none;
}

.kepler-titlebar--mac {
    padding-left: var(--kepler-mac-traffic-light-left-safe-area, 92px);
}

.kepler-titlebar--windows {
    height: calc(
        env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kepler-titlebar-height))
    );
    min-height: calc(
        env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kepler-titlebar-height))
    );
    padding-top: env(titlebar-area-y, 0px);
    padding-bottom: 0;
    padding-left: max(
        var(--kepler-titlebar-inline-padding),
        calc(env(titlebar-area-x, 0px) + var(--kepler-titlebar-inline-padding))
    );
    padding-right: max(
        var(--kepler-titlebar-inline-padding),
        calc(
            100vw - env(titlebar-area-x, 0px) - env(titlebar-area-width, 100vw) +
                var(--kepler-titlebar-inline-padding)
        )
    );
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

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
    "kosmos-titlebar",
    `kosmos-titlebar--${props.platform}`,
]);
</script>

<template>
    <header :class="titlebarClasses">
        <div class="kosmos-titlebar__leading">
            <slot name="leading" />
        </div>

        <div class="kosmos-titlebar__center">
            <slot name="center">
                <span v-if="title" class="kosmos-titlebar__title">{{ title }}</span>
            </slot>
        </div>

        <div class="kosmos-titlebar__trailing">
            <slot name="trailing" />
        </div>
    </header>
</template>

<style scoped>
.kosmos-titlebar {
    --kosmos-titlebar-height: 36px;
    --kosmos-titlebar-control-size: 32px;
    --kosmos-titlebar-control-radius: 10px;
    --kosmos-titlebar-inline-padding: 0.75rem;
    --kosmos-titlebar-vertical-padding: 0.375rem;
    box-sizing: border-box;
    position: relative;
    z-index: 20;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 0.75rem;
    height: calc(var(--kosmos-titlebar-height) + (var(--kosmos-titlebar-vertical-padding) * 2));
    min-height: calc(var(--kosmos-titlebar-height) + (var(--kosmos-titlebar-vertical-padding) * 2));
    padding:
        var(--kosmos-titlebar-vertical-padding)
        var(--kosmos-titlebar-inline-padding);
    background: var(--sidebar-bg);
    color: var(--sidebar-foreground);
    -webkit-app-region: drag;
    user-select: none;
}

.kosmos-titlebar--mac {
    padding-left: var(--kosmos-mac-traffic-light-left-safe-area, 92px);
}

.kosmos-titlebar--windows {
    height: calc(
        env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-titlebar-height))
    );
    min-height: calc(
        env(titlebar-area-y, 0px) + env(titlebar-area-height, var(--kosmos-titlebar-height))
    );
    padding-top: env(titlebar-area-y, 0px);
    padding-bottom: 0;
    padding-left: max(
        var(--kosmos-titlebar-inline-padding),
        calc(env(titlebar-area-x, 0px) + var(--kosmos-titlebar-inline-padding))
    );
    padding-right: max(
        var(--kosmos-titlebar-inline-padding),
        calc(
            100vw - env(titlebar-area-x, 0px) - env(titlebar-area-width, 100vw) +
                var(--kosmos-titlebar-inline-padding)
        )
    );
}

.kosmos-titlebar__leading,
.kosmos-titlebar__center,
.kosmos-titlebar__trailing {
    min-width: 0;
    display: flex;
    align-items: center;
    min-height: 100%;
    gap: 0.5rem;
}

.kosmos-titlebar__leading {
    justify-content: flex-start;
}

.kosmos-titlebar__center {
    justify-content: center;
}

.kosmos-titlebar__trailing {
    justify-content: flex-end;
}

.kosmos-titlebar__title {
    color: color-mix(in srgb, var(--sidebar-foreground) 82%, transparent);
    font-size: 0.8125rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    white-space: nowrap;
}

.kosmos-titlebar :deep(button),
.kosmos-titlebar :deep(a),
.kosmos-titlebar :deep(input),
.kosmos-titlebar :deep(select),
.kosmos-titlebar :deep(textarea),
.kosmos-titlebar :deep([role="button"]) {
    -webkit-app-region: no-drag;
}
</style>

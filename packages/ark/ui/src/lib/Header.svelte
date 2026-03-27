<script lang="ts">
    import type { ViewMode } from "./stores";

    interface Props {
        serverUrl: string;
        isConnected: boolean;
        viewMode: ViewMode;
        totalEvents: number;
        onConnect: () => void;
        onChangeView: (view: ViewMode) => void;
        onOpenSettings: () => void;
        onOpenPlugins: () => void;
    }

    let {
        serverUrl,
        isConnected,
        viewMode = "overview",
        totalEvents = 0,
        onConnect,
        onChangeView,
        onOpenSettings,
        onOpenPlugins,
    }: Props = $props();

    const views: { mode: ViewMode; label: string; icon: string }[] = [
        {
            mode: "overview",
            label: "Обзор",
            icon: "M4 5a1 1 0 011-1h14a1 1 0 011 1v2a1 1 0 01-1 1H5a1 1 0 01-1-1V5zM4 13a1 1 0 011-1h6a1 1 0 011 1v6a1 1 0 01-1 1H5a1 1 0 01-1-1v-6zM16 13a1 1 0 011-1h2a1 1 0 011 1v6a1 1 0 01-1 1h-2a1 1 0 01-1-1v-6z",
        },
        {
            mode: "events",
            label: "События",
            icon: "M4 6h16M4 10h16M4 14h16M4 18h16",
        },
    ];
</script>

<header class="flex items-center justify-between px-6 h-[60px] bg-bg-secondary border-b border-border shrink-0">
    <div class="flex items-center gap-6 min-w-0">
        {#if isConnected}
            <div class="flex items-center gap-3 min-w-0">
                <span class="font-mono text-sm text-text truncate max-w-[360px]">{serverUrl}</span>
                <span class="text-xs text-text-muted px-2 py-0.5 bg-bg-card border border-border">
                    {totalEvents.toLocaleString()} событий
                </span>
            </div>
        {/if}
    </div>

    {#if isConnected}
        <nav class="flex gap-1">
            {#each views as view (view.mode)}
                <button
                    class="flex items-center gap-2 px-4 py-2 text-sm transition-all
                   border border-transparent
                   {viewMode === view.mode
                        ? 'bg-bg-card border-border text-text'
                        : 'text-text-secondary hover:bg-bg-card hover:text-text'}"
                    onclick={() => onChangeView(view.mode)}
                >
                    <svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                        <path d={view.icon} />
                    </svg>
                    <span>{view.label}</span>
                </button>
            {/each}
        </nav>
    {/if}

    <div class="flex items-center gap-2">
        {#if !isConnected}
            <button
                class="flex items-center gap-2 px-4 py-2 bg-accent text-sm font-medium text-white
                 hover:bg-accent-hover transition-colors"
                onclick={() => onConnect()}
            >
                <svg class="w-[18px] h-[18px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <path d="M12 2a7 7 0 017 7v1h1a2 2 0 012 2v8a2 2 0 01-2 2H4a2 2 0 01-2-2v-8a2 2 0 012-2h1V9a7 7 0 017-7z" />
                </svg>
                <span>Подключиться</span>
            </button>
        {/if}

        <button
            class="p-2 text-text-secondary hover:text-text hover:bg-bg-card
                   border border-transparent hover:border-border transition-all"
            onclick={() => onOpenPlugins()}
            aria-label="Плагины"
        >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
            </svg>
        </button>

        <button
            class="p-2 text-text-secondary hover:text-text hover:bg-bg-card
                   border border-transparent hover:border-border transition-all"
            onclick={() => onOpenSettings()}
            aria-label="Настройки"
        >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M12 15a3 3 0 100-6 3 3 0 000 6z" />
                <path
                    d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"
                />
            </svg>
        </button>
    </div>
</header>

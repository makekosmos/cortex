<script lang="ts">
    import { onMount } from "svelte";
    import { getPlugins, syncPlugin, type PluginServerInfo } from "../api";
    import { plugins, updatePlugin } from "../stores";
    import type { PluginConfig } from "../types";

    interface Props {
        onClose: () => void;
    }

    let { onClose }: Props = $props();

    let selectedPluginId = $state<string | null>(null);
    let selectedPlugin = $derived.by(() => {
        if (!selectedPluginId) return null;
        return $plugins.find((p) => p.id === selectedPluginId) ?? null;
    });
    let editingInterval = $state(false);
    let intervalValue = $state(60);
    let isRefreshing = $state(false);
    let refreshError = $state<string | null>(null);
    let isSyncing = $state(false);
    let syncError = $state<string | null>(null);

    function mergePluginsFromServer(serverPlugins: PluginServerInfo[]) {
        plugins.update((local) => {
            const byId: Record<string, PluginServerInfo> = Object.create(null);
            for (const sp of serverPlugins) byId[sp.id] = sp;

            const merged = local.map((p) => {
                const sp = byId[p.id];
                if (!sp) return p;
                delete byId[p.id];
                return {
                    ...p,
                    name: sp.name,
                    description: sp.description,
                    icon: sp.icon,
                    requiresApiKey: sp.requires_api_key,
                    apiKeyConfigured: sp.api_key_configured,
                    lastSyncAt: sp.last_sync_at,
                    lastSyncStatus: (sp.last_sync_status as PluginConfig["lastSyncStatus"]) ?? p.lastSyncStatus,
                    lastSyncMessage: sp.last_sync_message,
                };
            });

            for (const id in byId) {
                const sp = byId[id];
                merged.push({
                    id: sp.id,
                    name: sp.name,
                    description: sp.description,
                    icon: sp.icon,
                    enabled: false,
                    syncIntervalMinutes: 60,
                    lastSyncAt: sp.last_sync_at,
                    lastSyncStatus: (sp.last_sync_status as PluginConfig["lastSyncStatus"]) ?? "never",
                    lastSyncMessage: sp.last_sync_message,
                    limits: [],
                    requiresApiKey: sp.requires_api_key,
                    apiKeyConfigured: sp.api_key_configured,
                });
            }

            return merged;
        });
    }

    async function refreshPlugins() {
        try {
            isRefreshing = true;
            refreshError = null;
            const serverPlugins = await getPlugins();
            mergePluginsFromServer(serverPlugins);
        } catch (e) {
            refreshError = e instanceof Error ? e.message : String(e);
        } finally {
            isRefreshing = false;
        }
    }

    function selectPlugin(plugin: PluginConfig) {
        selectedPluginId = plugin.id;
        intervalValue = plugin.syncIntervalMinutes;
        editingInterval = false;
    }

    function togglePlugin(plugin: PluginConfig) {
        updatePlugin(plugin.id, { enabled: !plugin.enabled });
    }

    function saveInterval() {
        if (selectedPlugin && intervalValue >= 5) {
            updatePlugin(selectedPlugin.id, { syncIntervalMinutes: intervalValue });
            editingInterval = false;
        }
    }

    // Keep the editor input in sync with the selected plugin unless user is editing.
    $effect(() => {
        if (selectedPlugin && !editingInterval) {
            intervalValue = selectedPlugin.syncIntervalMinutes;
        }
    });

    async function handleSyncNow() {
        if (!selectedPlugin) return;
        if (!selectedPlugin.apiKeyConfigured) return;
        if (isSyncing) return;

        try {
            isSyncing = true;
            syncError = null;
            updatePlugin(selectedPlugin.id, {
                lastSyncStatus: "running",
                lastSyncMessage: "running...",
            });

            await syncPlugin(selectedPlugin.id, { days: 30, full_sync: false });
            await refreshPlugins();
        } catch (e) {
            syncError = e instanceof Error ? e.message : String(e);
            updatePlugin(selectedPlugin.id, {
                lastSyncStatus: "error",
                lastSyncMessage: syncError,
            });
        } finally {
            isSyncing = false;
        }
    }

    function formatLastSync(date: string | null): string {
        if (!date) return "Никогда";
        const d = new Date(date);
        const now = new Date();
        const diff = now.getTime() - d.getTime();
        const minutes = Math.floor(diff / 60000);
        const hours = Math.floor(minutes / 60);
        const days = Math.floor(hours / 24);

        if (minutes < 1) return "Только что";
        if (minutes < 60) return `${minutes} мин. назад`;
        if (hours < 24) return `${hours} ч. назад`;
        return `${days} дн. назад`;
    }

    function getStatusColor(status: PluginConfig["lastSyncStatus"]): string {
        switch (status) {
            case "success":
                return "text-green";
            case "error":
                return "text-red";
            case "running":
                return "text-accent";
            default:
                return "text-text-muted";
        }
    }

    function getStatusText(status: PluginConfig["lastSyncStatus"]): string {
        switch (status) {
            case "success":
                return "Успешно";
            case "error":
                return "Ошибка";
            case "running":
                return "Синхронизация...";
            default:
                return "Не синхронизировано";
        }
    }

    function getStatusIcon(status: PluginConfig["lastSyncStatus"]): string {
        switch (status) {
            case "success":
                return "M5 13l4 4L19 7";
            case "error":
                return "M6 18L18 6M6 6l12 12";
            case "running":
                return "M12 6v6l4 2";
            default:
                // Circle (not synced yet)
                return "M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0";
        }
    }

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) {
            onClose();
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") {
            if (editingInterval) {
                editingInterval = false;
            } else if (selectedPlugin) {
                selectedPluginId = null;
            } else {
                onClose();
            }
        }
    }

    onMount(() => {
        refreshPlugins();
    });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div
    class="fixed inset-0 bg-black/80 z-50 flex items-center justify-center p-4"
    onclick={handleBackdropClick}
>
    <div
        class="bg-bg-secondary border border-border w-full max-w-2xl max-h-[80vh] flex flex-col animate-in fade-in zoom-in-95 duration-200"
    >
        <!-- Header -->
        <div class="flex items-center justify-between px-6 py-4 border-b border-border shrink-0">
            <div class="flex items-center gap-3">
                {#if selectedPlugin}
                    <button
                        class="p-1 text-text-muted hover:text-text transition-colors"
                        onclick={() => (selectedPluginId = null)}
                        aria-label="Назад к списку плагинов"
                    >
                        <svg
                            class="w-5 h-5"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                        >
                            <path d="M15 18l-6-6 6-6" />
                        </svg>
                    </button>
                {/if}
                <h2 class="text-lg font-medium text-text">
                    {selectedPlugin ? selectedPlugin.name : "Плагины"}
                </h2>
            </div>
            <button
                class="p-1 text-text-muted hover:text-text transition-colors"
                onclick={onClose}
                aria-label="Закрыть"
            >
                <svg
                    class="w-5 h-5"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                >
                    <path d="M6 18L18 6M6 6l12 12" />
                </svg>
            </button>
        </div>

        <!-- Content -->
        <div class="flex-1 overflow-y-auto p-6">
            {#if refreshError}
                <div class="mb-4 p-3 bg-bg-card border border-border text-sm text-red">
                    {refreshError}
                </div>
            {/if}
            {#if selectedPlugin}
                <!-- Plugin Detail View -->
                <div class="space-y-6">
                    <!-- Description -->
                    <div>
                        <p class="text-text-secondary">{selectedPlugin.description}</p>
                    </div>

                    <!-- Status -->
                    <div class="p-4 bg-bg-card border border-border space-y-3">
                        <div class="flex items-center justify-between">
                            <span class="text-sm text-text-muted">Статус</span>
                            <div
                                class="flex items-center gap-2 {getStatusColor(
                                    selectedPlugin.lastSyncStatus,
                                )}"
                            >
                                <svg
                                    class="w-4 h-4"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                >
                                    <path d={getStatusIcon(selectedPlugin.lastSyncStatus)} />
                                </svg>
                                <span class="text-sm"
                                    >{getStatusText(selectedPlugin.lastSyncStatus)}</span
                                >
                            </div>
                        </div>
                        <div class="flex items-center justify-between">
                            <span class="text-sm text-text-muted">Последняя синхронизация</span>
                            <span class="text-sm text-text">
                                {formatLastSync(selectedPlugin.lastSyncAt)}
                            </span>
                        </div>
                        {#if selectedPlugin.lastSyncMessage}
                            <div class="pt-2 border-t border-border">
                                <p class="text-xs text-text-muted">
                                    {selectedPlugin.lastSyncMessage}
                                </p>
                            </div>
                        {/if}
                    </div>

                    <!-- Configuration -->
                    <div class="space-y-4">
                        <h3 class="text-sm font-medium text-text-secondary uppercase tracking-wide">
                            Настройки
                        </h3>

                        <!-- Enabled Toggle -->
                        <div
                            class="flex items-center justify-between p-4 bg-bg-card border border-border"
                        >
                            <div>
                                <div class="text-sm font-medium text-text">Включен</div>
                                <div class="text-xs text-text-muted">
                                    Автоматическая синхронизация
                                </div>
                            </div>
                            <button
                                class="relative w-12 h-6 rounded-full transition-colors {selectedPlugin.enabled
                                    ? 'bg-accent'
                                    : 'bg-border'}"
                                onclick={() => togglePlugin(selectedPlugin!)}
                                aria-label="Переключить плагин"
                            >
                                <span
                                    class="absolute top-1 left-1 w-4 h-4 bg-white rounded-full transition-transform duration-200 {selectedPlugin.enabled
                                        ? 'translate-x-6'
                                        : 'translate-x-0'}"
                                ></span>
                            </button>
                        </div>

                        <!-- Sync Interval -->
                        <div
                            class="flex items-center justify-between p-4 bg-bg-card border border-border"
                        >
                            <div>
                                <div class="text-sm font-medium text-text">
                                    Интервал синхронизации
                                </div>
                                <div class="text-xs text-text-muted">
                                    Как часто проверять новые данные
                                </div>
                            </div>
                            {#if editingInterval}
                                <div class="flex items-center gap-2">
                                    <input
                                        type="number"
                                        min="5"
                                        max="1440"
                                        bind:value={intervalValue}
                                        class="w-20 px-2 py-1 bg-bg text-text text-sm border border-border focus:border-accent outline-none"
                                    />
                                    <span class="text-sm text-text-muted">мин</span>
                                    <button
                                        class="px-2 py-1 text-xs bg-accent text-white hover:bg-accent-hover transition-colors"
                                        onclick={saveInterval}
                                    >
                                        Сохранить
                                    </button>
                                </div>
                            {:else}
                                <button
                                    class="text-sm text-text hover:text-accent transition-colors"
                                    onclick={() => (editingInterval = true)}
                                >
                                    {selectedPlugin.syncIntervalMinutes} минут
                                </button>
                            {/if}
                        </div>

                        <!-- API Key Status -->
                        {#if selectedPlugin.requiresApiKey}
                            <div
                                class="flex items-center justify-between p-4 bg-bg-card border border-border"
                            >
                                <div>
                                    <div class="text-sm font-medium text-text">API-ключ</div>
                                    <div class="text-xs text-text-muted">
                                        Требуется для авторизации
                                    </div>
                                </div>
                                <div class="flex items-center gap-2">
                                    {#if selectedPlugin.apiKeyConfigured}
                                        <span class="text-sm text-green">Настроен</span>
                                    {:else}
                                        <span class="text-sm text-red">Не настроен</span>
                                    {/if}
                                </div>
                            </div>
                        {/if}

                        <!-- Manual Sync -->
                        <div
                            class="flex items-center justify-between p-4 bg-bg-card border border-border"
                        >
                            <div>
                                <div class="text-sm font-medium text-text">Синхронизация</div>
                                <div class="text-xs text-text-muted">
                                    Запустить импорт вручную (последние 30 дней)
                                </div>
                            </div>
                            <button
                                class="px-3 py-2 text-sm border border-border hover:bg-bg-hover transition-colors disabled:opacity-40 disabled:hover:bg-transparent"
                                disabled={isSyncing || (selectedPlugin.requiresApiKey && !selectedPlugin.apiKeyConfigured)}
                                onclick={handleSyncNow}
                            >
                                {isSyncing ? "Синхронизация..." : "Синхронизировать"}
                            </button>
                        </div>

                        {#if syncError}
                            <div class="text-xs text-red">{syncError}</div>
                        {/if}
                    </div>

                    <!-- Limits -->
                    {#if selectedPlugin.limits.length > 0}
                        <div class="space-y-4">
                            <h3
                                class="text-sm font-medium text-text-secondary uppercase tracking-wide"
                            >
                                Лимиты API
                            </h3>
                            <div class="space-y-2">
                                {#each selectedPlugin.limits as limit (limit.name)}
                                    <div class="p-3 bg-bg-card border border-border">
                                        <div class="flex items-center justify-between mb-1">
                                            <span class="text-sm font-medium text-text"
                                                >{limit.name}</span
                                            >
                                            <span class="text-xs font-mono text-accent"
                                                >{limit.value}</span
                                            >
                                        </div>
                                        <p class="text-xs text-text-muted">{limit.description}</p>
                                    </div>
                                {/each}
                            </div>
                        </div>
                    {/if}
                </div>
            {:else}
                <!-- Plugin List -->
                <div class="space-y-3">
                    {#each $plugins as plugin (plugin.id)}
                        <button
                            class="w-full flex items-center gap-4 p-4 bg-bg-card border border-border
                                   hover:bg-bg-hover transition-colors text-left"
                            onclick={() => selectPlugin(plugin)}
                        >
                            <!-- Icon -->
                            <div
                                class="w-10 h-10 flex items-center justify-center bg-bg-secondary border border-border shrink-0"
                            >
                                {#if plugin.icon === "clock"}
                                    <svg
                                        class="w-5 h-5 text-accent"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="1.5"
                                    >
                                        <circle cx="12" cy="12" r="10" />
                                        <path d="M12 6v6l4 2" />
                                    </svg>
                                {:else}
                                    <svg
                                        class="w-5 h-5 text-accent"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="1.5"
                                    >
                                        <path
                                            d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"
                                        />
                                    </svg>
                                {/if}
                            </div>

                            <!-- Info -->
                            <div class="flex-1 min-w-0">
                                <div class="flex items-center gap-2">
                                    <span class="font-medium text-text">{plugin.name}</span>
                                    {#if plugin.enabled}
                                        <span
                                            class="px-1.5 py-0.5 text-[10px] bg-accent/20 text-accent"
                                        >
                                            АКТИВЕН
                                        </span>
                                    {/if}
                                </div>
                                <p class="text-sm text-text-muted truncate">{plugin.description}</p>
                            </div>

                            <!-- Status indicator -->
                            <div class="shrink-0 {getStatusColor(plugin.lastSyncStatus)}">
                                <svg
                                    class="w-5 h-5"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                >
                                    <path d={getStatusIcon(plugin.lastSyncStatus)} />
                                </svg>
                            </div>

                            <!-- Arrow -->
                            <svg
                                class="w-5 h-5 text-text-muted shrink-0"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                            >
                                <path d="M9 18l6-6-6-6" />
                            </svg>
                        </button>
                    {/each}

                    {#if $plugins.length === 0}
                        <div class="text-center py-8">
                            <p class="text-text-muted">Нет доступных плагинов</p>
                        </div>
                    {/if}
                </div>
            {/if}
        </div>

        <!-- Footer -->
        <div class="px-6 py-4 border-t border-border flex justify-end shrink-0">
            <button
                class="px-4 py-2 text-sm text-text-secondary hover:text-text
                       border border-border hover:bg-bg-card transition-colors"
                onclick={onClose}
            >
                Закрыть
            </button>
        </div>
    </div>
</div>

<script lang="ts">
    import { onMount } from "svelte";
    import { getDbStats, queryEvents } from "./lib/api";
    import {
        serverUrl,
        apiKey,
        dbStats,
        isLoading,
        error,
        selectedCategory,
        selectedEventType,
        searchQuery,
        events,
        selectedEvent,
        viewMode,
        totalEvents,
        showSettings,
        showPlugins,
        isConnected,
        clearStoredCredentials,
        resetUIState,
        type ViewMode,
    } from "./lib/stores";
    import type { Event } from "./lib/types";

    import Header from "./lib/Header.svelte";
    import Sidebar from "./lib/Sidebar.svelte";
    import SearchBar from "./lib/SearchBar.svelte";
    import EventList from "./lib/EventList.svelte";
    import EventDetail from "./lib/EventDetail.svelte";
    import ChipVisualization from "./lib/ChipVisualization.svelte";
    import WelcomeScreen from "./lib/WelcomeScreen.svelte";
    import VirtualEventList from "./lib/components/VirtualEventList.svelte";
    import SettingsModal from "./lib/components/SettingsModal.svelte";
    import PluginsModal from "./lib/components/PluginsModal.svelte";
    import type { Filter } from "./lib/types";

    let loadingEvents = $state(false);
    let currentOffset = $state(0);
    const pageSize = 50;

    // Use new virtualized list
    let useVirtualList = $state(true);
    let virtualListRef: VirtualEventList | null = $state(null);

    async function connect() {
        try {
            $isLoading = true;
            $error = null;
            resetUIState();

            const stats = await getDbStats();
            $dbStats = stats;

            // Only used by the non-virtual list path.
            if (!useVirtualList && $viewMode === "events") {
                await loadEvents(true);
            }
        } catch (e) {
            $dbStats = null;
            $error = e instanceof Error ? e.message : String(e);
        } finally {
            $isLoading = false;
        }
    }

    // Auto-connect on startup if a key is already stored in the browser.
    onMount(() => {
        if ($apiKey) {
            connect();
        }
    });

    // Build filters for virtual list
    let activeFilters = $derived.by(() => {
        const filters: Filter[] = [];
        if ($selectedCategory) {
            filters.push({
                column: "category",
                operator: "Equals",
                value: $selectedCategory,
            });
        }
        if ($selectedEventType) {
            filters.push({
                column: "event_type",
                operator: "Equals",
                value: $selectedEventType,
            });
        }
        return filters;
    });

    async function loadEvents(reset: boolean = false) {
        if (loadingEvents) return;

        loadingEvents = true;

        try {
            if (reset) {
                currentOffset = 0;
                $events = [];
            }

            const params: Record<string, unknown> = {
                limit: pageSize,
                offset: currentOffset,
            };

            if ($selectedCategory) {
                params.category = $selectedCategory;
            }

            if ($selectedEventType) {
                params.event_type = $selectedEventType;
            }

            if ($searchQuery) {
                params.search = $searchQuery;
            }

            const newEvents = await queryEvents(params);

            if (reset) {
                $events = newEvents;
            } else {
                $events = [...$events, ...newEvents];
            }

            currentOffset += newEvents.length;
        } catch (e) {
            $error = e instanceof Error ? e.message : String(e);
        } finally {
            loadingEvents = false;
        }
    }

    function handleViewChange(mode: ViewMode) {
        $viewMode = mode;
        if (!useVirtualList && mode === "events") {
            loadEvents(true);
        }
    }

    function handleCategorySelect(category: string | null) {
        $selectedCategory = category;
        $selectedEventType = null;
        $selectedEvent = null;
        if (!useVirtualList && $viewMode === "events") {
            loadEvents(true);
        }
    }

    function handleEventTypeSelect(data: { category: string; eventType: string } | null) {
        if (data) {
            $selectedCategory = data.category;
            $selectedEventType = data.eventType;
        } else {
            $selectedEventType = null;
        }
        $selectedEvent = null;
        if (!useVirtualList && $viewMode === "events") {
            loadEvents(true);
        }
    }

    function handleSearch(query: string) {
        $searchQuery = query;
        // Virtual list handles its own data loading via reactive filters/search
        if (!useVirtualList) {
            loadEvents(true);
        }
    }

    function handleEventSelect(event: Event) {
        $selectedEvent = event;
    }

    function handleLoadMore() {
        loadEvents(false);
    }

    function handleChipSelect(category: string) {
        $selectedCategory = category;
        $viewMode = "events";
        if (!useVirtualList) {
            loadEvents(true);
        }
    }

    function handleOpenSettings() {
        $showSettings = true;
    }

    function handleCloseSettings() {
        $showSettings = false;
    }

    async function handleSaveSettings(url: string, key: string) {
        serverUrl.set(url);
        apiKey.set(key);
        $showSettings = false;
        await connect();
    }

    function handleDisconnect() {
        clearStoredCredentials();
        resetUIState();
        $viewMode = "overview";
        $showSettings = false;
        $showPlugins = false;
    }

    function handleOpenPlugins() {
        $showPlugins = true;
    }

    function handleClosePlugins() {
        $showPlugins = false;
    }
</script>

<div class="flex flex-col h-full overflow-hidden">
    <Header
        serverUrl={$serverUrl}
        isConnected={$isConnected}
        viewMode={$viewMode}
        totalEvents={$totalEvents}
        onConnect={handleOpenSettings}
        onChangeView={handleViewChange}
        onOpenSettings={handleOpenSettings}
        onOpenPlugins={handleOpenPlugins}
    />

    <main class="flex-1 flex overflow-hidden">
        {#if !$isConnected}
            <WelcomeScreen onConnect={handleOpenSettings} />
        {:else if $viewMode === "overview"}
            <div class="flex-1 flex items-center justify-center p-8">
                <ChipVisualization
                    categories={$dbStats?.categories || []}
                    totalEvents={$totalEvents}
                    onSelect={handleChipSelect}
                />
            </div>
        {:else if $viewMode === "events"}
            <Sidebar
                categories={$dbStats?.categories || []}
                selectedCategory={$selectedCategory}
                selectedEventType={$selectedEventType}
                totalEvents={$totalEvents}
                onSelectCategory={handleCategorySelect}
                onSelectEventType={handleEventTypeSelect}
            />

            <div class="flex-1 flex flex-col overflow-hidden">
                <div class="flex items-center gap-4 p-4 border-b border-border">
                    <div class="flex-1 max-w-md">
                        <SearchBar value={$searchQuery} onSearch={handleSearch} />
                    </div>
                    <div class="text-sm text-text-muted whitespace-nowrap">
                        {$totalEvents.toLocaleString()} events
                    </div>
                </div>

                <div class="flex-1 flex overflow-hidden">
                    <div
                        class="flex-1 min-w-75 max-w-125 border-r border-border overflow-hidden flex flex-col"
                    >
                        {#if useVirtualList}
                            <VirtualEventList
                                bind:this={virtualListRef}
                                filters={activeFilters}
                                search={$searchQuery}
                                selectedId={$selectedEvent?.id || null}
                                onSelect={handleEventSelect}
                            />
                        {:else}
                            <EventList
                                events={$events}
                                loading={loadingEvents}
                                selectedId={$selectedEvent?.id || null}
                                onSelect={handleEventSelect}
                                onLoadMore={handleLoadMore}
                            />
                        {/if}
                    </div>

                    <div class="flex-1 overflow-hidden">
                        <EventDetail event={$selectedEvent} />
                    </div>
                </div>
            </div>
        {/if}
    </main>

    <!-- Settings Modal -->
    {#if $showSettings}
        <SettingsModal
            serverUrl={$serverUrl}
            apiKey={$apiKey}
            onClose={handleCloseSettings}
            onSave={handleSaveSettings}
            onDisconnect={handleDisconnect}
        />
    {/if}

    <!-- Plugins Modal -->
    {#if $showPlugins}
        <PluginsModal onClose={handleClosePlugins} />
    {/if}

    <!-- Error Toast -->
    {#if $error}
        <div
            class="fixed bottom-4 left-1/2 -translate-x-1/2 flex items-center gap-4 px-4 py-3
                bg-red rounded-lg text-white text-sm animate-in slide-in-from-bottom duration-300"
        >
            <span>{$error}</span>
            <button
                class="opacity-70 hover:opacity-100 transition-opacity"
                onclick={() => ($error = null)}
                aria-label="Dismiss error"
            >
                <svg
                    class="w-4.5 h-4.5"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                >
                    <path d="M6 18L18 6M6 6l12 12" />
                </svg>
            </button>
        </div>
    {/if}

    <!-- Loading Overlay -->
    {#if $isLoading}
        <div class="fixed inset-0 flex flex-col items-center justify-center gap-4 bg-black/80 z-50">
            <div
                class="w-10 h-10 border-[3px] border-border border-t-accent rounded-full animate-spin"
            ></div>
            <span class="text-text-secondary">Loading...</span>
        </div>
    {/if}
</div>

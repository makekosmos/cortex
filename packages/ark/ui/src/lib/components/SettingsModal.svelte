<script lang="ts">
    interface Props {
        serverUrl: string;
        apiKey: string | null;
        onClose: () => void;
        onSave: (serverUrl: string, apiKey: string) => void;
        onDisconnect: () => void;
    }

    let { serverUrl, apiKey, onClose, onSave, onDisconnect }: Props = $props();

    let url = $state(serverUrl);
    let key = $state(apiKey ?? "");

    function handleBackdropClick(e: MouseEvent) {
        if (e.target === e.currentTarget) {
            onClose();
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") {
            onClose();
        }
    }

    function handleSave() {
        const trimmedUrl = url.trim().replace(/\/+$/, "");
        const trimmedKey = key.trim();
        if (!trimmedUrl) return;
        if (!trimmedKey) return;
        onSave(trimmedUrl, trimmedKey);
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div
    class="fixed inset-0 bg-black/80 z-50 flex items-center justify-center p-4"
    onclick={handleBackdropClick}
>
    <div class="bg-bg-secondary border border-border w-full max-w-lg animate-in fade-in zoom-in-95 duration-200">
        <!-- Header -->
        <div class="flex items-center justify-between px-6 py-4 border-b border-border">
            <h2 class="text-lg font-medium text-text">Настройки</h2>
            <button
                class="p-1 text-text-muted hover:text-text transition-colors"
                onclick={onClose}
                aria-label="Закрыть настройки"
            >
                <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M6 18L18 6M6 6l12 12" />
                </svg>
            </button>
        </div>

        <!-- Content -->
        <div class="p-6 space-y-6">
            <div class="space-y-3">
                <h3 class="text-sm font-medium text-text-secondary uppercase tracking-wide">
                    Подключение к серверу
                </h3>

                <div class="space-y-2">
                    <label class="block text-xs text-text-muted">Адрес сервера</label>
                    <input
                        class="w-full px-3 py-2 bg-bg-card border border-border text-text text-sm outline-none focus:border-accent"
                        placeholder="http://localhost:8000"
                        bind:value={url}
                    />
                </div>

                <div class="space-y-2">
                    <label class="block text-xs text-text-muted">API ключ</label>
                    <input
                        class="w-full px-3 py-2 bg-bg-card border border-border text-text text-sm outline-none focus:border-accent"
                        placeholder="вставьте ключ"
                        bind:value={key}
                    />
                    <p class="text-xs text-text-muted">
                        Ключ хранится в браузере (localStorage) и отправляется в заголовке Authorization.
                    </p>
                </div>
            </div>
        </div>

        <!-- Footer -->
        <div class="px-6 py-4 border-t border-border flex items-center justify-between gap-3">
            <button
                class="px-4 py-2 text-sm text-text-secondary hover:text-text
                       border border-border hover:bg-bg-card transition-colors"
                onclick={onDisconnect}
            >
                Отключиться
            </button>

            <div class="flex gap-2">
                <button
                    class="px-4 py-2 text-sm text-text-secondary hover:text-text
                           border border-border hover:bg-bg-card transition-colors"
                    onclick={onClose}
                >
                    Закрыть
                </button>
                <button
                    class="px-4 py-2 text-sm bg-accent text-white font-medium
                           hover:bg-accent-hover transition-colors"
                    onclick={handleSave}
                >
                    Сохранить и подключиться
                </button>
            </div>
        </div>
    </div>
</div>

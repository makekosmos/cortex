<script lang="ts">
    interface Props {
        onConnect?: () => void;
    }

    let { onConnect }: Props = $props();
</script>

<div class="flex items-center justify-center h-full p-8">
    <div class="flex flex-col items-center text-center max-w-md">
        <!-- Logo -->
        <div class="flex flex-col items-center mb-12">
            <div
                class="relative w-20 h-20 flex items-center justify-center
                  bg-gradient-to-br from-accent to-blue rounded-2xl mb-6"
            >
                <span class="text-4xl font-bold text-black">L</span>
                <div
                    class="absolute -inset-2.5 bg-inherit rounded-inherit blur-xl opacity-40 -z-10
                    animate-pulse"
                ></div>
            </div>
            <h1 class="text-4xl font-bold text-text mb-2">Life DB</h1>
            <p class="text-lg text-text-secondary">Ваша личная база данных на сервере</p>
        </div>

        <!-- Features -->
        <div class="flex flex-col gap-4 mb-12 w-full">
            <div
                class="flex items-start gap-4 text-left p-4 bg-bg-card border border-border rounded-xl"
            >
                <div
                    class="w-10 h-10 flex items-center justify-center bg-bg-secondary rounded-lg text-accent shrink-0"
                >
                    <svg
                        class="w-5 h-5"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.5"
                    >
                        <path
                            d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4"
                        />
                    </svg>
                </div>
                <div>
                    <h3 class="text-sm font-semibold text-text mb-1">Одна база</h3>
                    <p class="text-xs text-text-muted">
                        Данные хранятся в одной базе на вашем VPS
                    </p>
                </div>
            </div>

            <div
                class="flex items-start gap-4 text-left p-4 bg-bg-card border border-border rounded-xl"
            >
                <div
                    class="w-10 h-10 flex items-center justify-center bg-bg-secondary rounded-lg text-accent shrink-0"
                >
                    <svg
                        class="w-5 h-5"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.5"
                    >
                        <path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                    </svg>
                </div>
                <div>
                    <h3 class="text-sm font-semibold text-text mb-1">Мгновенный поиск</h3>
                    <p class="text-xs text-text-muted">Полнотекстовый поиск по всем событиям</p>
                </div>
            </div>

            <div
                class="flex items-start gap-4 text-left p-4 bg-bg-card border border-border rounded-xl"
            >
                <div
                    class="w-10 h-10 flex items-center justify-center bg-bg-secondary rounded-lg text-accent shrink-0"
                >
                    <svg
                        class="w-5 h-5"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.5"
                    >
                        <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                </div>
                <div>
                    <h3 class="text-sm font-semibold text-text mb-1">50+ лет надёжности</h3>
                    <p class="text-xs text-text-muted">
                        Создано для долгосрочного архивного хранения
                    </p>
                </div>
            </div>
        </div>

        <!-- CTA -->
        <button
            class="flex items-center gap-3 px-8 py-4 bg-accent rounded-xl text-base font-semibold text-white
             hover:bg-accent-hover transition-all hover:-translate-y-0.5 hover:shadow-[0_10px_30px_rgba(155,35,53,0.3)]"
            onclick={() => onConnect?.()}
        >
            <svg
                class="w-6 h-6"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
            >
                <path
                    d="M12 2a7 7 0 017 7v1h1a2 2 0 012 2v8a2 2 0 01-2 2H4a2 2 0 01-2-2v-8a2 2 0 012-2h1V9a7 7 0 017-7z"
                />
            </svg>
            <span>Подключиться к серверу</span>
        </button>

        <p class="mt-4 text-sm text-text-muted">Укажите адрес сервера и ключ доступа</p>
    </div>
</div>

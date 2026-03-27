<script lang="ts">
    interface Props {
        value: string;
        placeholder?: string;
        onSearch?: (query: string) => void;
    }

    let {
        value = $bindable(""),
        placeholder = "Search events...",
        onSearch,
    }: Props = $props();

    let debounceTimer: ReturnType<typeof setTimeout>;

    function handleInput(e: Event) {
        const target = e.target as HTMLInputElement;
        value = target.value;

        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => {
            onSearch?.(value);
        }, 300);
    }

    function handleClear() {
        value = "";
        onSearch?.("");
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === "Escape") {
            handleClear();
        }
    }
</script>

<div
    class="flex items-center gap-2 px-3 py-2 bg-bg-card border border-border rounded-lg
            transition-all focus-within:border-accent focus-within:shadow-[0_0_0_2px_rgba(0,212,170,0.3)]"
>
    <svg
        class="w-[18px] h-[18px] text-text-muted shrink-0"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
    >
        <path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
    </svg>

    <input
        type="text"
        class="flex-1 bg-transparent border-none outline-none text-sm text-text placeholder:text-text-muted"
        {placeholder}
        {value}
        oninput={handleInput}
        onkeydown={handleKeydown}
    />

    {#if value}
        <button
            class="w-5 h-5 flex items-center justify-center text-text-muted hover:text-text transition-colors"
            onclick={handleClear}
            aria-label="Clear search"
        >
            <svg
                class="w-3.5 h-3.5"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
            >
                <path d="M6 18L18 6M6 6l12 12" />
            </svg>
        </button>
    {/if}
</div>

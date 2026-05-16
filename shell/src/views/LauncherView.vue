<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted, nextTick } from "vue";
import type { Component } from "vue";
import {
    Settings as SettingsIcon,
    Database as DatabaseIcon,
    ArrowUpCircle,
    Loader2,
} from "lucide-vue-next";
import BuiltInIcon from "../components/BuiltInIcon.vue";
import type { CommandRecord, UpdateState } from "@shared/ipc-types";

interface BuiltInIconConfig {
    icon: Component;
    from: string;
    to: string;
}

const BUILTIN_ICONS: Record<string, BuiltInIconConfig> = {
    "settings:open": {
        icon: SettingsIcon,
        from: "oklch(0.42 0 0)",
        to: "oklch(0.26 0 0)",
    },
    "dashboard:open": {
        icon: DatabaseIcon,
        from: "oklch(0.62 0.16 165)",
        to: "oklch(0.42 0.14 175)",
    },
};

function builtInIconFor(cmd: CommandRecord): BuiltInIconConfig | null {
    return BUILTIN_ICONS[cmd.id] ?? null;
}

const query = ref("");
const commands = ref<CommandRecord[]>([]);
const selectedIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLDivElement | null>(null);

interface ScoredCommand {
    cmd: CommandRecord;
    score: number;
}

function scoreCommand(cmd: CommandRecord, q: string): number {
    if (!q) return 0;
    const ql = q.toLowerCase();
    const t = cmd.title.toLowerCase();
    const s = (cmd.subtitle ?? "").toLowerCase();
    const titleIdx = t.indexOf(ql);
    const subIdx = s.indexOf(ql);
    if (titleIdx < 0 && subIdx < 0) return -1;
    // Раньше = выше; title match лучше subtitle match.
    if (titleIdx === 0) return 1000;
    if (titleIdx > 0) return 500 - titleIdx;
    return 100 - subIdx;
}

const filtered = computed<CommandRecord[]>(() => {
    const q = query.value.trim();
    if (!q) return commands.value;
    return commands.value
        .map<ScoredCommand>((cmd) => ({ cmd, score: scoreCommand(cmd, q) }))
        .filter((x) => x.score >= 0)
        .sort((a, b) => b.score - a.score)
        .map((x) => x.cmd);
});

const RECENTS_KEY = "kepler.launcher.recents";
const RECENTS_LIMIT = 5;

function loadRecents(): string[] {
    try {
        const raw = localStorage.getItem(RECENTS_KEY);
        if (!raw) return [];
        const arr = JSON.parse(raw);
        return Array.isArray(arr) ? arr.filter((x): x is string => typeof x === "string") : [];
    } catch {
        return [];
    }
}

const recentIds = ref<string[]>(loadRecents());

// --- Update banner state ----------------------------------------------------
const updateState = ref<UpdateState>({ kind: "idle" });
let unsubUpdateState: (() => void) | null = null;

const updateBanner = computed<
    | null
    | {
        title: string;
        subtitle: string;
        icon: Component;
        spinning: boolean;
        clickable: boolean;
    }
>(() => {
    const s = updateState.value;
    if (s.kind === "downloaded") {
        return {
            title: `Обновить Kepler до ${s.version}`,
            subtitle: "Установить и перезапустить",
            icon: ArrowUpCircle,
            spinning: false,
            clickable: true,
        };
    }
    if (s.kind === "downloading") {
        return {
            title: `Скачивается Kepler ${s.version}`,
            subtitle: `${Math.round(s.percent)}%`,
            icon: Loader2,
            spinning: true,
            clickable: false,
        };
    }
    if (s.kind === "available") {
        return {
            title: `Доступно обновление Kepler ${s.version}`,
            subtitle: "Скачивается в фоне",
            icon: ArrowUpCircle,
            spinning: false,
            clickable: false,
        };
    }
    return null;
});

async function onBannerClick() {
    const b = updateBanner.value;
    if (!b?.clickable) return;
    await window.kepler.settings.update.install();
}

function recordRecent(id: string) {
    const next = [id, ...recentIds.value.filter((x) => x !== id)].slice(0, RECENTS_LIMIT);
    recentIds.value = next;
    try {
        localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
    } catch {
        /* storage quota — ignore */
    }
}

const groupedNoQuery = computed(() => {
    if (query.value.trim()) return null;
    const byId = new Map(commands.value.map((c) => [c.id, c]));
    const recent: CommandRecord[] = [];
    for (const id of recentIds.value) {
        const c = byId.get(id);
        if (c) recent.push(c);
    }
    // «Все» намеренно содержит все команды — в том числе те, что уже есть в
    // «Недавние». Это дублирование запрошено: список «Все» должен быть полным.
    return { recent, all: commands.value };
});

function onInput() {
    selectedIndex.value = 0;
}

// Шаблон уважает «виртуальный» banner-item впереди: selectedIndex 0 — это
// banner, далее recent, далее all. flatList используется для invocation
// (banner не реальная команда, потому фильтруется).
function totalRows(): number {
    const banner = updateBanner.value ? 1 : 0;
    if (groupedNoQuery.value) {
        return banner + groupedNoQuery.value.recent.length + groupedNoQuery.value.all.length;
    }
    return banner + filtered.value.length;
}

function rowAt(idx: number): { kind: "banner" } | { kind: "cmd"; cmd: CommandRecord } | null {
    const banner = updateBanner.value ? 1 : 0;
    if (banner && idx === 0) return { kind: "banner" };
    const i = idx - banner;
    if (groupedNoQuery.value) {
        const rec = groupedNoQuery.value.recent;
        const all = groupedNoQuery.value.all;
        if (i < rec.length) return { kind: "cmd", cmd: rec[i]! };
        const j = i - rec.length;
        if (j < all.length) return { kind: "cmd", cmd: all[j]! };
        return null;
    }
    const c = filtered.value[i];
    return c ? { kind: "cmd", cmd: c } : null;
}

async function invokeSelected() {
    const row = rowAt(selectedIndex.value);
    if (!row) return;
    if (row.kind === "banner") {
        await onBannerClick();
        return;
    }
    recordRecent(row.cmd.id);
    await window.kepler.commands.invoke(row.cmd.id);
    query.value = "";
    selectedIndex.value = 0;
}

function moveSelection(delta: number) {
    const n = totalRows();
    if (n === 0) return;
    const next = selectedIndex.value + delta;
    // Clamp без wrap — упереться в границы.
    selectedIndex.value = Math.max(0, Math.min(n - 1, next));
    void nextTick(() => {
        const list = listRef.value;
        if (!list) return;
        // Для самого верхнего ряда (banner / первый item) — упираемся в top.
        if (selectedIndex.value === 0) {
            list.scrollTop = 0;
            return;
        }
        const selectedEl = list.querySelector<HTMLElement>(".result.selected");
        if (!selectedEl) return;
        // Когда выделение — первый <li> в своей <ul>, прокручиваем к заголовку
        // секции (sibling <ul> → previousElementSibling = .section-label).
        const isFirstInUl = selectedEl.parentElement?.firstElementChild === selectedEl;
        if (isFirstInUl) {
            const header = selectedEl.parentElement!
                .previousElementSibling as HTMLElement | null;
            if (header?.classList.contains("section-label")) {
                header.scrollIntoView({ block: "start" });
                return;
            }
        }
        selectedEl.scrollIntoView({ block: "nearest" });
    });
}

function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
        e.preventDefault();
        void window.kepler.window.hide();
    } else if (e.key === "ArrowDown") {
        e.preventDefault();
        moveSelection(1);
    } else if (e.key === "ArrowUp") {
        e.preventDefault();
        moveSelection(-1);
    } else if (e.key === "Enter") {
        e.preventDefault();
        void invokeSelected();
    }
}

async function refreshCommands() {
    try {
        const all = await window.kepler.commands.list();
        commands.value = all.filter((c) => c.category !== "action");
    } catch (e) {
        console.warn("commands.list failed", e);
        commands.value = [];
    }
}

let offShow = () => { };
let offCommandsUpdated = () => { };

onMounted(async () => {
    offShow = window.kepler.window.onShow(() => {
        query.value = "";
        selectedIndex.value = 0;
        void refreshCommands();
        void nextTick(() => inputRef.value?.focus());
    });
    offCommandsUpdated = window.kepler.commands.onUpdated(() => {
        void refreshCommands();
    });
    try {
        updateState.value = await window.kepler.settings.update.state();
    } catch {
        /* main may not be ready yet — ignore */
    }
    unsubUpdateState = window.kepler.settings.update.onStateChanged((s) => {
        updateState.value = s;
    });
    void refreshCommands();
    void nextTick(() => inputRef.value?.focus());
});

onUnmounted(() => {
    offShow();
    offCommandsUpdated();
    unsubUpdateState?.();
});

</script>

<template>
    <div class="launcher" @keydown="onKey">
        <input ref="inputRef" v-model="query" class="search" type="text"
            placeholder="Поиск команд: pomo, заметка, открыть delphi…" spellcheck="false" autocomplete="off"
            autocorrect="off" autocapitalize="off" @input="onInput" />
        <div ref="listRef" class="list kosmos-scroll">
            <template v-if="updateBanner">
                <div class="section-label">Обновление</div>
                <ul class="results">
                    <li class="result update-tile" :class="{ selected: selectedIndex === 0, disabled: !updateBanner.clickable }"
                        @click="() => { selectedIndex = 0; void invokeSelected(); }">
                        <span class="update-icon">
                            <component :is="updateBanner.icon" :size="14" :stroke-width="2"
                                :class="{ spin: updateBanner.spinning }" />
                        </span>
                        <span class="title">{{ updateBanner.title }}</span>
                        <span class="kind-label">{{ updateBanner.subtitle }}</span>
                    </li>
                </ul>
            </template>
            <template v-if="groupedNoQuery">
                <template v-if="groupedNoQuery.recent.length > 0">
                    <div class="section-label">Недавние</div>
                    <ul class="results">
                        <li v-for="(cmd, idx) in groupedNoQuery.recent" :key="`recent-${cmd.id}`" class="result"
                            :class="{ selected: (updateBanner ? 1 : 0) + idx === selectedIndex }"
                            @click="() => { selectedIndex = (updateBanner ? 1 : 0) + idx; void invokeSelected(); }">
                            <BuiltInIcon v-if="builtInIconFor(cmd)" :icon="builtInIconFor(cmd)!.icon"
                                :from="builtInIconFor(cmd)!.from" :to="builtInIconFor(cmd)!.to" />
                            <img v-else-if="cmd.icon" :src="cmd.icon" class="icon" alt="" />
                            <BuiltInIcon v-else />
                            <span class="title">{{ cmd.title }}</span>
                            <span v-if="cmd.appName && cmd.kind === 'command'" class="app-name">{{ cmd.appName }}</span>
                            <span class="kind-label">{{ cmd.kind === 'command' ? 'Команда' : 'Приложение' }}</span>
                        </li>
                    </ul>
                </template>
                <template v-if="groupedNoQuery.all.length > 0">
                    <div class="section-label">Все</div>
                    <ul class="results">
                        <li v-for="(cmd, idx) in groupedNoQuery.all" :key="`all-${cmd.id}`" class="result"
                            :class="{ selected: (updateBanner ? 1 : 0) + groupedNoQuery.recent.length + idx === selectedIndex }"
                            @click="() => { selectedIndex = (updateBanner ? 1 : 0) + groupedNoQuery!.recent.length + idx; void invokeSelected(); }">
                            <BuiltInIcon v-if="builtInIconFor(cmd)" :icon="builtInIconFor(cmd)!.icon"
                                :from="builtInIconFor(cmd)!.from" :to="builtInIconFor(cmd)!.to" />
                            <img v-else-if="cmd.icon" :src="cmd.icon" class="icon" alt="" />
                            <BuiltInIcon v-else />
                            <span class="title">{{ cmd.title }}</span>
                            <span v-if="cmd.appName && cmd.kind === 'command'" class="app-name">{{ cmd.appName }}</span>
                            <span class="kind-label">{{ cmd.kind === 'command' ? 'Команда' : 'Приложение' }}</span>
                        </li>
                    </ul>
                </template>
            </template>
            <template v-else>
                <div v-if="filtered.length === 0" class="empty">Ничего не найдено</div>
                <ul v-else class="results">
                    <li v-for="(cmd, idx) in filtered" :key="cmd.id" class="result"
                        :class="{ selected: (updateBanner ? 1 : 0) + idx === selectedIndex }"
                        @click="() => { selectedIndex = (updateBanner ? 1 : 0) + idx; void invokeSelected(); }">
                        <img v-if="cmd.icon" :src="cmd.icon" class="icon" alt="" />
                        <span v-else class="icon icon-placeholder" aria-hidden="true" />
                        <span class="title">{{ cmd.title }}</span>
                        <span class="subtitle">{{ cmd.subtitle }}</span>
                    </li>
                </ul>
            </template>
        </div>
    </div>
</template>

<style scoped>
.launcher {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, oklch(0.04 0 0) 75%, transparent);
}

.search {
    width: 100%;
    height: 64px;
    padding: 0 22px;
    border: none;
    outline: none;
    background: transparent;
    color: var(--foreground);
    font-size: 18px;
    font-weight: 400;
    flex-shrink: 0;
}

.search::placeholder {
    color: color-mix(in srgb, var(--foreground) 36%, transparent);
}

.list {
    flex: 1;
    overflow-y: auto;
    border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
    padding: 8px 0;
}

.section-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: color-mix(in srgb, var(--foreground) 45%, transparent);
    padding: 4px 16px 8px;
}

.results {
    margin: 0;
    padding: 0;
    list-style: none;
}

.result {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: transparent;
    cursor: pointer;
    transition: all 0.12s cubic-bezier(0.4, 0, 0.2, 1);
    margin: 1px 0;
}

.icon {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    flex-shrink: 0;
    object-fit: cover;
}

.icon-placeholder {
    background: transparent;
}

.update-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 6px;
    background: oklch(0.55 0.15 250);
    color: oklch(0.98 0 0);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 12%, transparent);
    flex-shrink: 0;
}

.update-tile.disabled {
    cursor: default;
}

.spin {
    animation: kepler-spin 1s linear infinite;
}

@keyframes kepler-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
}

.icon-builtin {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    background: linear-gradient(to bottom left,
            oklch(0.42 0 0),
            oklch(0.26 0 0));
    color: oklch(0.96 0 0);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
}

.result:hover {
    background: color-mix(in srgb, oklch(1 0 0) 3.5%, transparent);
}

.result.selected {
    background: color-mix(in srgb, oklch(1 0 0) 8%, transparent);
    border-color: color-mix(in srgb, oklch(1 0 0) 12%, transparent);
}

.title {
    color: var(--foreground);
    font-size: 14px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 0 1 auto;
    min-width: 0;
}

.subtitle {
    color: color-mix(in srgb, var(--foreground) 50%, transparent);
    font-size: 12px;
    flex-shrink: 0;
    margin-left: 12px;
}

.app-name {
    color: color-mix(in srgb, var(--foreground) 42%, transparent);
    font-size: 14px;
    font-weight: 400;
    flex-shrink: 0;
    margin-left: 10px;
}

.kind-label {
    color: color-mix(in srgb, var(--foreground) 32%, transparent);
    font-size: 12px;
    flex-shrink: 0;
    margin-left: auto;
    padding-left: 12px;
}

.empty {
    padding: 32px 22px;
    text-align: center;
    color: color-mix(in srgb, var(--foreground) 40%, transparent);
    font-size: 13px;
}
</style>

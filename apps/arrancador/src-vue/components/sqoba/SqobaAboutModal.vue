<script setup lang="ts">
import {
  File as FileIcon,
  RefreshCw,
  Save,
  Search,
  Sparkles,
  X,
} from "lucide-vue-next";

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();

const cards = [
  {
    title: "Automatic save discovery",
    description:
      "SQOBA uses a manifest sourced from Ludusavi and PCGamingWiki patterns to locate saves with less manual setup.",
    icon: Search,
    className:
      "bg-gradient-to-b from-emerald-500/28 via-emerald-500/8 to-background/20 border-emerald-400/25 shadow-[0_30px_90px_rgba(16,185,129,0.18)]",
  },
  {
    title: "Folders and files",
    description:
      "You can point SQOBA at a folder or at a specific file when the game stores saves in mixed locations.",
    icon: FileIcon,
    className:
      "bg-gradient-to-b from-sky-500/26 via-indigo-500/10 to-background/20 border-sky-400/25 shadow-[0_30px_90px_rgba(56,189,248,0.16)]",
  },
  {
    title: "{PATHTOGAME} templates",
    description:
      "Relative save paths survive game moves better because the path can resolve from the executable directory.",
    icon: Sparkles,
    className:
      "bg-gradient-to-b from-amber-500/28 via-orange-500/10 to-background/20 border-amber-400/25 shadow-[0_30px_90px_rgba(245,158,11,0.16)]",
  },
  {
    title: "Backup history",
    description:
      "Backups store file manifests so restoring a previous save set is predictable and easier to audit.",
    icon: Save,
    className:
      "bg-gradient-to-b from-rose-500/22 via-fuchsia-500/10 to-background/20 border-rose-400/25 shadow-[0_30px_90px_rgba(244,63,94,0.14)]",
  },
  {
    title: "Compression",
    description:
      "Optional compression reduces backup size while preserving the SQOBA archive format.",
    icon: Sparkles,
    className:
      "bg-gradient-to-b from-teal-500/26 via-cyan-500/10 to-background/20 border-teal-400/25 shadow-[0_30px_90px_rgba(20,184,166,0.14)]",
  },
  {
    title: "Offline fallback",
    description:
      "Even without a network connection, SQOBA can keep working from the embedded snapshot and local cache.",
    icon: RefreshCw,
    className:
      "bg-gradient-to-b from-slate-500/18 via-zinc-500/10 to-background/20 border-white/12 shadow-[0_30px_90px_rgba(148,163,184,0.10)]",
  },
] as const;
</script>

<template>
  <Teleport to="body">
    <div
      v-if="props.open"
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm"
    >
      <button
        type="button"
        aria-label="Close SQOBA info"
        class="absolute inset-0"
        @click="emit('close')"
      />

      <div class="relative z-10 flex max-h-[90vh] w-full max-w-5xl flex-col overflow-hidden rounded-2xl border bg-background/85 shadow-[0_30px_80px_rgba(0,0,0,0.55)]">
        <div class="flex items-start justify-between gap-4 border-b border-border/60 p-5">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <div class="flex h-9 w-9 items-center justify-center rounded-xl border border-white/10 bg-gradient-to-br from-emerald-500/20 via-sky-500/10 to-transparent shadow-[0_10px_30px_rgba(8,12,24,0.25)]">
                <Sparkles class="h-4 w-4 text-foreground" />
              </div>
              <div>
                <div class="text-lg font-bold tracking-tight">SQOBA</div>
                <div class="text-xs text-muted-foreground">
                  Save-path detection and backup handling in one workflow.
                </div>
              </div>
            </div>
          </div>

          <button
            type="button"
            class="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-border/70 bg-card/80 transition-colors hover:bg-accent/70"
            @click="emit('close')"
          >
            <X class="h-4 w-4" />
          </button>
        </div>

        <div class="flex-1 overflow-auto p-5">
          <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            <div
              v-for="(card, index) in cards"
              :key="card.title"
              class="animate-in fade-in slide-in-from-bottom-2 duration-300"
              :style="{ animationDelay: `${index * 60}ms` }"
            >
              <div
                class="relative flex min-h-[220px] flex-col overflow-hidden rounded-xl border p-5 shadow-[0_18px_55px_rgba(8,12,24,0.30)] backdrop-blur-xl transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[0_24px_80px_rgba(8,12,24,0.38)]"
                :class="card.className"
              >
                <div class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_15%_0%,rgba(255,255,255,0.22),transparent_55%),radial-gradient(circle_at_80%_85%,rgba(255,255,255,0.12),transparent_60%)] opacity-70" />
                <div class="pointer-events-none absolute -top-14 -right-14 h-40 w-40 rounded-full bg-white/10 blur-2xl" />
                <div class="pointer-events-none absolute -bottom-20 -left-20 h-56 w-56 rounded-full bg-white/5 blur-3xl" />

                <div class="relative flex flex-1 flex-col">
                  <div class="flex items-center justify-between gap-3">
                    <div class="flex h-12 w-12 items-center justify-center rounded-xl border border-white/12 bg-white/5 shadow-[0_12px_30px_rgba(0,0,0,0.22)]">
                      <component :is="card.icon" class="h-5 w-5 text-foreground" />
                    </div>
                    <span class="rounded-full border border-white/10 bg-white/5 px-2.5 py-1 text-[10px] text-muted-foreground">
                      SQOBA
                    </span>
                  </div>

                  <div class="mt-4 text-base font-semibold tracking-tight">
                    {{ card.title }}
                  </div>
                  <div class="mt-2 text-xs leading-relaxed text-muted-foreground">
                    {{ card.description }}
                  </div>

                  <div class="mt-auto pt-4">
                    <div class="h-px w-full bg-gradient-to-r from-white/20 via-white/0 to-transparent" />
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div class="mt-5 rounded-xl border bg-muted/30 p-4">
            <div class="text-sm font-semibold">Quick tip</div>
            <div class="mt-1 text-xs leading-relaxed text-muted-foreground">
              If a game moves to a different folder, store the save path with
              <span class="mx-1 font-mono">{PATHTOGAME}</span>
              so the path can resolve from the executable location instead of a fixed absolute path.
            </div>
          </div>
        </div>
      </div>
    </div>
  </Teleport>
</template>

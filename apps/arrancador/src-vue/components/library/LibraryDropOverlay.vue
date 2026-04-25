<template>
  <div class="pointer-events-none fixed inset-0 z-50">
    <div class="absolute inset-0 bg-background/40 backdrop-blur-sm" />
    <div class="absolute inset-3 rounded-2xl border-2 border-dashed border-primary/70 shadow-[0_0_0_1px_rgba(255,255,255,0.06),0_30px_80px_rgba(8,12,24,0.55)] sm:inset-6">
      <div class="absolute inset-0 flex items-center justify-center">
        <div class="rounded-2xl border border-border/60 bg-card/80 px-6 py-4 text-center shadow-[0_18px_45px_rgba(8,12,24,0.45)] backdrop-blur-xl">
          <div class="text-sm font-semibold text-foreground">Отпустите, чтобы добавить игру</div>
          <div class="text-xs text-muted-foreground">Поддерживаются .exe и .lnk</div>
        </div>
      </div>
    </div>
  </div>
</template>

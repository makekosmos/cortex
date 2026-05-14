<script setup lang="ts">
// SVG sphere icon с псевдо-3D radial gradient. Используется на Welcome screen.
defineProps<{
  size?: number;
}>();
</script>

<template>
  <div class="logo" :style="{ width: `${size ?? 64}px`, height: `${size ?? 64}px` }">
    <svg viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
      <defs>
        <radialGradient id="kosmos-sphere-grad" cx="35%" cy="30%" r="65%">
          <stop offset="0%" stop-color="#c0c0c0" />
          <stop offset="35%" stop-color="#7a7a7a" />
          <stop offset="100%" stop-color="#1a1a1a" />
        </radialGradient>
        <radialGradient id="kosmos-sphere-highlight" cx="32%" cy="25%" r="22%">
          <stop offset="0%" stop-color="rgba(255,255,255,0.55)" />
          <stop offset="100%" stop-color="rgba(255,255,255,0)" />
        </radialGradient>
      </defs>
      <rect x="0" y="0" width="64" height="64" rx="14" ry="14" fill="#0d0d0d" />
      <circle cx="32" cy="32" r="22" fill="url(#kosmos-sphere-grad)" />
      <circle cx="32" cy="32" r="22" fill="url(#kosmos-sphere-highlight)" />
    </svg>
  </div>
</template>

<style scoped>
.logo {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.logo svg {
  width: 100%;
  height: 100%;
  display: block;
}
</style>

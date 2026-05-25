<script setup lang="ts">
import { computed, onMounted, ref } from "vue";

interface ManifestPreview {
  manifest: {
    id: string;
    name: string;
    version?: string;
    description?: string;
    author?: string;
    permissions?: string[];
    keplerApiVersion?: string;
  };
  iconDataUri: string | null;
  apiCompatError: string | null;
  isUpgrade: boolean;
  currentVersion: string | null;
}

const sourcePath = ref<string>("");
const preview = ref<ManifestPreview | null>(null);
const loading = ref<boolean>(true);
const error = ref<string>("");
const installing = ref<boolean>(false);
const installed = ref<boolean>(false);

function readSourcePathFromHash(): string {
  const hash = window.location.hash;
  const q = hash.indexOf("?");
  if (q < 0) return "";
  const params = new URLSearchParams(hash.slice(q + 1));
  return params.get("path") ?? "";
}

async function load() {
  loading.value = true;
  error.value = "";
  preview.value = null;
  try {
    sourcePath.value = readSourcePathFromHash();
    if (!sourcePath.value) {
      error.value = "Путь к .kext не передан";
      return;
    }
    const p = await window.kepler.extension.installPreview(sourcePath.value);
    preview.value = p;
  } catch (e) {
    error.value = (e as Error).message || String(e);
  } finally {
    loading.value = false;
  }
}

async function onConfirm() {
  if (!preview.value || installing.value) return;
  installing.value = true;
  error.value = "";
  try {
    await window.kepler.extension.installDo(sourcePath.value);
    installed.value = true;
    setTimeout(() => {
      window.close();
    }, 1200);
  } catch (e) {
    error.value = (e as Error).message || String(e);
  } finally {
    installing.value = false;
  }
}

function onCancel() {
  window.close();
}

const buttonLabel = computed(() =>
  installing.value
    ? "Установка…"
    : installed.value
      ? "Установлено"
      : preview.value?.isUpgrade
        ? "Обновить"
        : "Установить",
);

const canInstall = computed(
  () => !!preview.value && !preview.value.apiCompatError && !installing.value && !installed.value,
);

onMounted(() => {
  void load();
});
</script>

<template>
  <div class="install" tabindex="0" @keydown.esc="onCancel">
    <header class="header">
      <h1>Установка расширения</h1>
    </header>

    <div v-if="loading" class="state empty">Чтение .kext…</div>

    <div v-else-if="error && !preview" class="state error-state">
      <div class="state-title">Не удалось открыть .kext</div>
      <div class="state-message">{{ error }}</div>
      <div class="footer">
        <button class="btn" type="button" @click="onCancel">Закрыть</button>
      </div>
    </div>

    <template v-else-if="preview">
      <div class="body">
        <div class="head-row">
          <img v-if="preview.iconDataUri" class="icon" :src="preview.iconDataUri" alt="" />
          <div v-else class="icon icon-fallback">{{ preview.manifest.name.slice(0, 1) }}</div>
          <div class="head-text">
            <div class="name">{{ preview.manifest.name }}</div>
            <div class="meta">
              <span class="version">v{{ preview.manifest.version ?? "—" }}</span>
              <span v-if="preview.manifest.author" class="author">
                · {{ preview.manifest.author }}
              </span>
            </div>
          </div>
        </div>

        <div v-if="preview.isUpgrade" class="banner upgrade">
          Обновление: установлено v{{ preview.currentVersion ?? "—" }} → v{{
            preview.manifest.version ?? "—"
          }}
        </div>

        <div v-if="preview.apiCompatError" class="banner incompat">
          {{ preview.apiCompatError }}
        </div>

        <div v-if="preview.manifest.description" class="description">
          {{ preview.manifest.description }}
        </div>

        <div class="rows">
          <div class="row">
            <div class="row-label">ID</div>
            <code class="row-value">{{ preview.manifest.id }}</code>
          </div>
          <div class="row" v-if="preview.manifest.keplerApiVersion">
            <div class="row-label">Требуется Kepler API</div>
            <code class="row-value">{{ preview.manifest.keplerApiVersion }}</code>
          </div>
          <div class="row" v-if="preview.manifest.permissions?.length">
            <div class="row-label">Разрешения</div>
            <div class="row-value perms">
              <code v-for="p in preview.manifest.permissions" :key="p" class="perm">
                {{ p }}
              </code>
            </div>
          </div>
        </div>

        <div v-if="error" class="banner incompat">{{ error }}</div>
      </div>

      <div class="footer">
        <button class="btn ghost" type="button" @click="onCancel">Отмена</button>
        <button class="btn primary" type="button" :disabled="!canInstall" @click="onConfirm">
          {{ buttonLabel }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.install {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  background: color-mix(in srgb, oklch(0.04 0 0) 75%, transparent);
  outline: none;
  color: var(--foreground);
}

.header {
  padding: 14px 22px;
  border-bottom: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  -webkit-app-region: drag;
}

.header h1 {
  margin: 0;
  font-size: 13px;
  font-weight: 500;
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
}

.body {
  flex: 1;
  padding: 18px 22px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow-y: auto;
}

.head-row {
  display: flex;
  gap: 14px;
  align-items: center;
}

.icon {
  width: 56px;
  height: 56px;
  border-radius: 12px;
  object-fit: cover;
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.icon-fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 22px;
  font-weight: 600;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
}

.head-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.name {
  font-size: 17px;
  font-weight: 600;
}

.meta {
  font-size: 12px;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
}

.banner {
  padding: 10px 12px;
  border-radius: 8px;
  font-size: 12px;
  line-height: 1.4;
}

.banner.upgrade {
  background: color-mix(in srgb, oklch(0.7 0.18 250) 14%, transparent);
  color: color-mix(in srgb, oklch(0.85 0.12 250) 100%, transparent);
}

.banner.incompat {
  background: color-mix(in srgb, oklch(0.65 0.22 25) 14%, transparent);
  color: oklch(0.85 0.18 25);
}

.description {
  font-size: 13px;
  color: color-mix(in srgb, var(--foreground) 75%, transparent);
  line-height: 1.45;
}

.rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.row {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  font-size: 12px;
}

.row-label {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  flex-shrink: 0;
}

.row-value {
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 11px;
  color: color-mix(in srgb, var(--foreground) 85%, transparent);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  padding: 2px 6px;
  border-radius: 5px;
  text-align: right;
  word-break: break-all;
}

.perms {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  background: transparent;
  padding: 0;
  justify-content: flex-end;
}

.perm {
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  padding: 2px 6px;
  border-radius: 5px;
}

.footer {
  padding: 14px 22px;
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
}

.btn {
  font: inherit;
  font-size: 13px;
  padding: 7px 16px;
  border-radius: 7px;
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
  -webkit-app-region: no-drag;
}

.btn:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 12%, transparent);
}

.btn.primary {
  background: oklch(0.7 0.18 250);
  border-color: oklch(0.7 0.18 250);
  color: oklch(0.12 0 0);
  font-weight: 500;
}

.btn.primary:hover:not(:disabled) {
  background: oklch(0.76 0.18 250);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn.ghost {
  background: transparent;
}

.state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 32px;
  text-align: center;
}

.state.empty {
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
  font-size: 13px;
}

.state-title {
  font-size: 14px;
  font-weight: 600;
  color: oklch(0.85 0.18 25);
}

.state-message {
  font-size: 12px;
  color: color-mix(in srgb, var(--foreground) 65%, transparent);
  max-width: 380px;
}
</style>

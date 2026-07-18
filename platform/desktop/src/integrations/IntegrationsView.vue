<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Modal } from "@kosmos/visuals";
import IntegrationSettingsPanel from "./IntegrationSettingsPanel.vue";
import hevyIcon from "./assets/hevy.svg";
import togglTrackIcon from "./assets/toggl-track.svg";
import leetcodeIcon from "./assets/leetcode.svg";
import codewarsIcon from "./assets/codewars.svg";

type ProviderId = "hevy" | "toggl" | "leetcode" | "codewars";

interface ProviderSnapshot {
  id: ProviderId;
  hasCredential: boolean;
}

interface IntegrationsSnapshot {
  providers: ProviderSnapshot[];
}

interface IntegrationCard {
  id: ProviderId;
  label: string;
  icon: string;
}

const cards: IntegrationCard[] = [
  {
    id: "hevy",
    label: "Hevy",
    icon: hevyIcon,
  },
  {
    id: "toggl",
    label: "Toggl Track",
    icon: togglTrackIcon,
  },
  {
    id: "leetcode",
    label: "LeetCode",
    icon: leetcodeIcon,
  },
  {
    id: "codewars",
    label: "Codewars",
    icon: codewarsIcon,
  },
];

const snapshot = ref<IntegrationsSnapshot | null>(null);
const loading = ref(true);
const error = ref("");
const selectedProvider = ref<ProviderId | null>(null);
const selectedCard = computed(() => cards.find((card) => card.id === selectedProvider.value));

function isConnected(provider: ProviderId): boolean {
  return (
    snapshot.value?.providers.some((item) => item.id === provider && item.hasCredential) ?? false
  );
}

async function load(): Promise<void> {
  loading.value = true;
  error.value = "";
  try {
    snapshot.value = (await window.kepler.ark.request(
      "integrations.list",
      {},
    )) as IntegrationsSnapshot;
  } catch (cause) {
    error.value = cause instanceof Error ? cause.message : "Не удалось загрузить интеграции";
  } finally {
    loading.value = false;
  }
}

function open(provider: ProviderId): void {
  selectedProvider.value = provider;
}

function close(): void {
  selectedProvider.value = null;
  void load();
}

onMounted(load);
</script>

<template>
  <main class="integrations-view kosmos-scroll">
    <header class="integrations-view__header">
      <h1>Интеграции</h1>
      <p>Подключайте внешние сервисы и управляйте получением данных.</p>
    </header>

    <div v-if="loading" class="integrations-view__state">Загрузка…</div>
    <div v-else-if="error" class="integrations-view__state integrations-view__state--error">
      {{ error }}
    </div>

    <section v-else class="integration-grid" aria-label="Доступные интеграции">
      <button
        v-for="card in cards"
        :key="card.id"
        type="button"
        class="integration-card"
        :data-testid="`integration-card-${card.id}`"
        :aria-label="`${card.label}: ${isConnected(card.id) ? 'Подключено' : 'Не подключено'}`"
        @click="open(card.id)"
      >
        <img class="integration-card__icon" :src="card.icon" alt="" aria-hidden="true" />
        <span
          :class="[
            'integration-card__status',
            { 'integration-card__status--connected': isConnected(card.id) },
          ]"
        >
          {{ isConnected(card.id) ? "Подключено" : "Не подключено" }}
        </span>
      </button>
    </section>

    <Modal
      :open="selectedProvider !== null"
      :title="selectedCard ? `Настройка ${selectedCard.label}` : 'Настройка интеграции'"
      width="min(720px, 94vw)"
      @close="close"
    >
      <IntegrationSettingsPanel v-if="selectedProvider" :provider-id="selectedProvider" compact />
    </Modal>
  </main>
</template>

<style scoped>
.integrations-view {
  height: 100%;
  overflow-y: auto;
  padding: 22px 24px 28px;
  color: var(--foreground);
}

.integrations-view__header h1 {
  margin: 0;
  font-size: 1.35rem;
}

.integrations-view__header p,
.integrations-view__state {
  margin: 5px 0 0;
  color: var(--muted-foreground);
  font-size: 0.75rem;
  line-height: 1.5;
}

.integrations-view__state {
  margin-top: 20px;
}

.integrations-view__state--error {
  color: var(--destructive);
}

.integration-grid {
  --integration-border: color-mix(in srgb, var(--foreground) 14%, transparent);
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  margin-top: 22px;
  border: 1px solid var(--integration-border);
}

.integration-card {
  display: flex;
  min-height: 176px;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: 18px;
  padding: 24px;
  border: 0;
  border-right: 1px solid var(--integration-border);
  border-radius: 0;
  background: transparent;
  color: var(--foreground);
  text-align: center;
  transition:
    background-color 140ms ease,
    color 140ms ease;
}

.integration-card:last-child {
  border-right: 0;
}

.integration-card:hover {
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
}

.integration-card:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

.integration-card__icon {
  display: block;
  width: 64px;
  height: 64px;
  object-fit: contain;
}

.integration-card__status {
  color: var(--muted-foreground);
  font-size: 0.75rem;
  line-height: 1.4;
  white-space: nowrap;
}

.integration-card__status--connected {
  color: var(--accent);
}

@media (max-width: 780px) {
  .integration-grid {
    grid-template-columns: 1fr;
  }

  .integration-card {
    border-right: 0;
    border-bottom: 1px solid var(--integration-border);
  }

  .integration-card:last-child {
    border-bottom: 0;
  }
}
</style>

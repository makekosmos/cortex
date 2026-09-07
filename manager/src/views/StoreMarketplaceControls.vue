<script setup lang="ts">
import type { StoreListing, InstalledStoreItem } from "../manager-api";
import type { MarketplaceFilters, MarketplaceTab } from "../store-catalog-helpers";
import StoreListingCard from "./StoreListingCard.vue";

defineProps<{
  tabs: readonly { id: MarketplaceTab; label: string }[];
  activeTab: MarketplaceTab;
  filters: MarketplaceFilters;
  options: {
    platforms: string[];
    kinds: string[];
    categories: string[];
    data: string[];
    fidelity: string[];
  };
  recommendations: StoreListing[];
  installedFor: (listing: StoreListing) => InstalledStoreItem | undefined;
  catalogAvailable: boolean;
  installing: (listing: StoreListing) => boolean;
  feedback: (listing: StoreListing) => { kind: "success" | "error"; message: string } | undefined;
}>();
const emit = defineEmits<{
  tab: [MarketplaceTab];
  install: [StoreListing];
  open: [InstalledStoreItem];
  external: [StoreListing];
  details: [StoreListing];
}>();
</script>

<template>
  <header class="store-marketplace-header">
    <div>
      <p class="eyebrow">Marketplace</p>
      <h1>Приложения и интеграции</h1>
    </div>
    <nav class="store-tabs" aria-label="Разделы Marketplace">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        type="button"
        :class="{ active: activeTab === tab.id }"
        @click="emit('tab', tab.id)"
      >
        {{ tab.label }}
      </button>
    </nav>
  </header>
  <section class="store-filters" aria-label="Фильтры Marketplace">
    <label
      >Платформа<select v-model="filters.platform">
        <option value="all">Все</option>
        <option v-for="value in options.platforms" :key="value" :value="value">{{ value }}</option>
      </select></label
    >
    <label
      >Тип<select v-model="filters.kind">
        <option value="all">Все</option>
        <option v-for="value in options.kinds" :key="value" :value="value">{{ value }}</option>
      </select></label
    >
    <label
      >Категория<select v-model="filters.category">
        <option value="all">Все</option>
        <option v-for="value in options.categories" :key="value" :value="value">{{ value }}</option>
      </select></label
    >
    <label
      >Данные<select v-model="filters.data">
        <option value="all">Все</option>
        <option v-for="value in options.data" :key="value" :value="value">{{ value }}</option>
      </select></label
    >
    <label
      >Точность<select v-model="filters.fidelity">
        <option value="all">Все</option>
        <option v-for="value in options.fidelity" :key="value" :value="value">{{ value }}</option>
      </select></label
    >
    <label
      >Состояние<select v-model="filters.install">
        <option value="all">Все</option>
        <option value="installed">Установлены</option>
        <option value="available">Не установлены</option>
        <option value="update">Есть обновления</option>
      </select></label
    >
  </section>
  <section
    v-if="activeTab === 'discover' && recommendations.length"
    class="data-recommendation"
    aria-label="Для ваших данных"
  >
    <h2>Для ваших данных</h2>
    <p>Локальный ARK нашёл совместимые приложения.</p>
    <div class="store-grid">
      <StoreListingCard
        v-for="listing in recommendations.slice(0, 3)"
        :key="`recommendation-${listing.id}`"
        :listing="listing"
        :installed="installedFor(listing)"
        :catalog-available="catalogAvailable"
        :installing="installing(listing)"
        :feedback="feedback(listing)"
        @install="emit('install', $event)"
        @open="emit('open', $event)"
        @external="emit('external', $event)"
        @details="emit('details', $event)"
      />
    </div>
  </section>
</template>

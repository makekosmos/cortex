<script setup lang="ts">
import { Button, SettingsList, SettingsRow } from "@kosmos/visuals";
import type { StoreListing } from "../manager-api";
import { appIcon } from "../app-icons";

defineProps<{
  listing: StoreListing;
  summary: string;
  metadata: Array<{ label: string; value: string }>;
  permissions: string[];
  actionLabel: string;
  actionDisabled?: boolean;
  busy: boolean;
}>();
const emit = defineEmits<{ action: [] }>();
</script>

<template>
  <section class="store-detail-app-header">
    <img
      v-if="appIcon(listing.id, listing.icon_url)"
      :src="appIcon(listing.id, listing.icon_url)!"
      alt=""
    />
    <div class="store-detail-app-copy">
      <h1>{{ listing.name }}</h1>
      <p>{{ summary }}</p>
    </div>
    <Button
      size="sm"
      variant="surface"
      :disabled="actionDisabled || busy"
      @click="emit('action')"
      >{{ busy ? "Установка…" : actionLabel }}</Button
    >
  </section>
  <div class="store-detail-gallery" aria-label="Превью приложения">
    <div v-for="index in 2" :key="index" class="store-detail-shot">
      <img
        v-if="appIcon(listing.id, listing.icon_url)"
        :src="appIcon(listing.id, listing.icon_url)!"
        alt=""
      />
    </div>
  </div>
  <dl class="store-detail-metadata">
    <div v-for="item in metadata" :key="item.label">
      <dt>{{ item.label }}</dt>
      <dd>{{ item.value }}</dd>
    </div>
  </dl>
  <SettingsList><SettingsRow title="О приложении" :description="summary" stacked /></SettingsList>
  <SettingsList>
    <SettingsRow title="Разрешения" description="Приложению потребуется:" />
    <SettingsRow
      v-if="!permissions.length"
      title=""
      description="Эффективные разрешения появятся после установки."
    />
    <SettingsRow
      v-for="permission in permissions"
      :key="permission"
      title=""
      :description="permission"
    />
  </SettingsList>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { Button } from "@kosmos/visuals";
import { Package as PackageIcon } from "@lucide/vue";
import type { InstalledStoreItem, StoreListing } from "../manager-api";
import { appIcon } from "../app-icons";

const props = defineProps<{
  listing: StoreListing;
  installed?: InstalledStoreItem;
  catalogAvailable: boolean;
  installing?: boolean;
  feedback?: { kind: "success" | "error"; message: string };
  featured?: boolean;
  development?: boolean;
}>();
const emit = defineEmits<{
  install: [StoreListing];
  open: [InstalledStoreItem];
  external: [StoreListing];
  details: [StoreListing];
  development: [StoreListing];
}>();

const iconFailed = ref(false);
const icon = computed(() =>
  appIcon(props.listing.id, props.listing.icon_url, props.installed?.icon_path),
);
const canOpen = computed(
  () =>
    props.development ||
    (props.listing.kind === "external-app" && props.catalogAvailable) ||
    props.installed?.kind === "app",
);
const canInstall = computed(
  () =>
    !props.development &&
    (!props.installed || Boolean(props.installed.update_version)) &&
    props.catalogAvailable &&
    props.listing.distribution &&
    "package_id" in props.listing.distribution,
);

function open() {
  if (props.development) emit("development", props.listing);
  else if (props.listing.kind === "external-app") emit("external", props.listing);
  else if (props.installed?.kind === "app") emit("open", props.installed);
}

function details() {
  emit("details", props.listing);
}
</script>

<template>
  <article
    class="store-card"
    :class="{
      'store-card-clickable': true,
      'store-card-featured': featured,
    }"
    role="button"
    tabindex="0"
    :aria-label="`Подробнее: ${listing.name}`"
    @click="details"
    @keydown.enter="details"
    @keydown.space.prevent="details"
  >
    <div class="store-card-main">
      <span
        v-if="icon && !iconFailed"
        class="store-card-icon-frame"
        :class="`store-card-icon-frame--${listing.id.replaceAll('.', '-')}`"
      >
        <img class="store-card-icon" :src="icon" alt="" @error="iconFailed = true" />
      </span>
      <span v-else class="store-card-icon-frame store-card-icon-fallback" aria-hidden="true">
        <PackageIcon :size="40" />
      </span>
      <div class="store-card-copy">
        <div class="store-card-title">
          <h2>{{ listing.name }}</h2>
          <small v-if="development" class="store-card-development">Разработка</small>
        </div>
        <p v-if="featured && listing.description" class="store-card-description">
          {{ listing.description }}
        </p>
      </div>
    </div>

    <div class="store-card-action" @click.stop>
      <Button
        v-if="canInstall"
        size="sm"
        variant="surface"
        block
        :loading="installing"
        :aria-busy="installing"
        @click="emit('install', listing)"
        >{{
          installing ? "Установка…" : installed?.update_version ? "Обновить" : "Установить"
        }}</Button
      >
      <Button v-else-if="canOpen" size="sm" block variant="surface" @click="open">Открыть</Button>
      <Button v-else size="sm" block variant="surface" disabled>
        {{ installed ? "Установлено" : "Недоступно" }}
      </Button>
      <small
        v-if="feedback"
        class="store-card-feedback"
        :class="feedback.kind === 'error' ? 'error' : 'success'"
        role="status"
      >
        {{ feedback.message }}
      </small>
    </div>
  </article>
</template>

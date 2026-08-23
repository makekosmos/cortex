<script setup lang="ts">
import { computed, ref } from "vue";
import { Button } from "@kosmos/visuals";
import { Package as PackageIcon } from "@lucide/vue";
import type { InstalledStoreItem, StoreListing } from "../manager-api";

const props = defineProps<{
  listing: StoreListing;
  installed?: InstalledStoreItem;
  catalogAvailable: boolean;
  installing?: boolean;
  feedback?: { kind: "success" | "error"; message: string };
  featured?: boolean;
}>();
const emit = defineEmits<{
  install: [StoreListing];
  open: [InstalledStoreItem];
  external: [StoreListing];
}>();

const iconFailed = ref(false);
const icon = computed(() => {
  const local = props.installed?.icon_path?.trim();
  if (local) return `file:///${local.replace(/\\/g, "/")}`;
  const remote = props.listing.icon_url?.trim();
  return remote && /^(https:|http:|file:|data:image\/|\/)/.test(remote) ? remote : null;
});
const canOpen = computed(
  () => props.listing.kind === "external-app" || props.installed?.kind === "app",
);
const canInstall = computed(
  () =>
    (!props.installed || Boolean(props.installed.update_version)) &&
    props.catalogAvailable &&
    props.listing.distribution &&
    "package_id" in props.listing.distribution,
);

function open() {
  if (props.listing.kind === "external-app") emit("external", props.listing);
  else if (props.installed?.kind === "app") emit("open", props.installed);
}
</script>

<template>
  <article
    class="store-card"
    :class="{
      'store-card-clickable': canOpen,
      'store-card-featured': featured,
    }"
    :role="canOpen ? 'button' : undefined"
    :tabindex="canOpen ? 0 : undefined"
    :aria-label="canOpen ? `Открыть ${listing.name}` : undefined"
    @click="open"
    @keydown.enter="open"
    @keydown.space.prevent="open"
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
        <h2>{{ listing.name }}</h2>
        <p class="store-card-publisher">{{ listing.publisher || "Kosmos" }}</p>
        <p v-if="featured && listing.description" class="store-card-description">
          {{ listing.description }}
        </p>
      </div>
    </div>

    <div class="store-card-action" @click.stop>
      <Button
        v-if="canInstall"
        size="sm"
        block
        :loading="installing"
        :aria-busy="installing"
        @click="emit('install', listing)"
        >{{
          installing ? "Установка…" : installed?.update_version ? "Обновить" : "Установить"
        }}</Button
      >
      <Button v-else-if="canOpen" size="sm" block @click="open">Открыть</Button>
      <Button v-else size="sm" block variant="ghost" disabled>
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

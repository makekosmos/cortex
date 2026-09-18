<script setup lang="ts">
import { Button, Modal } from "@kosmos/visuals";
import type { DisclosureSection } from "../disclosure-helpers";

defineProps<{
  open: boolean;
  name: string;
  sections: DisclosureSection[];
  unavailable?: boolean;
  busy?: boolean;
}>();
const emit = defineEmits<{ confirm: []; decline: [] }>();
</script>

<template>
  <Modal
    :open="open"
    :title="`Подключение ${name}`"
    width="min(560px, 94vw)"
    @close="emit('decline')"
  >
    <article class="stack disclosure-panel">
      <p class="muted disclosure-intro">
        Проверьте, к каким данным приложение получит доступ, перед подключением.
      </p>
      <p v-if="busy" class="muted">Загрузка разрешений…</p>
      <template v-else>
        <p v-if="unavailable" class="error">
          Не удалось загрузить список разрешений — подтвердите подключение, только если доверяете
          приложению.
        </p>
        <p v-else-if="!sections.length" class="muted">
          Приложение не заявляет доступ к данным Kosmos.
        </p>
        <section v-for="section in sections" :key="section.title" class="disclosure-section">
          <h3>{{ section.title }}</h3>
          <ul>
            <li v-for="line in section.lines" :key="line">{{ line }}</li>
          </ul>
        </section>
      </template>
      <div class="actions">
        <Button variant="surface" size="sm" @click="emit('decline')">Отмена</Button>
        <Button variant="surface" size="sm" :disabled="busy" @click="emit('confirm')">
          Разрешить и продолжить
        </Button>
      </div>
    </article>
  </Modal>
</template>

<style scoped>
.disclosure-intro {
  margin: 0;
}

.disclosure-section h3 {
  margin: 0;
  font-size: 0.8125rem;
  font-weight: 600;
}

.disclosure-section ul {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 6px 0 0;
  padding: 0;
  list-style: none;
  color: var(--muted-foreground);
  font-size: 0.75rem;
  line-height: 1.45;
}
</style>

<script setup lang="ts">
import { ref } from "vue";
import Modal from "../../components/Modal.vue";

const openBasic = ref(false);
const openCustomHeader = ref(false);
const openFooter = ref(false);
const openWide = ref(false);
</script>

<template>
  <Story title="Modal" group="popovers" :layout="{ type: 'single', iframe: true }">
    <Variant title="Базовая (title + body)">
      <div class="story-canvas">
        <button class="open-btn" type="button" @click="openBasic = true">Открыть</button>
        <Modal :open="openBasic" title="Подтвердить удаление" @close="openBasic = false">
          <p>Объект будет помечен как `is_trashed`. Sync синхронизирует это на остальные устройства.</p>
        </Modal>
      </div>
    </Variant>

    <Variant title="С footer-слотом">
      <div class="story-canvas">
        <button class="open-btn" type="button" @click="openFooter = true">Открыть</button>
        <Modal :open="openFooter" title="Удалить задачу?" @close="openFooter = false">
          <p>Действие нельзя отменить. Trashed-объекты очищаются раз в 30 дней.</p>
          <template #footer>
            <button class="ghost" type="button" @click="openFooter = false">Отмена</button>
            <button class="danger" type="button" @click="openFooter = false">Удалить</button>
          </template>
        </Modal>
      </div>
    </Variant>

    <Variant title="С кастомным header-слотом">
      <div class="story-canvas">
        <button class="open-btn" type="button" @click="openCustomHeader = true">Открыть</button>
        <Modal :open="openCustomHeader" @close="openCustomHeader = false">
          <template #header>
            <div style="display:flex;align-items:center;gap:0.5rem;">
              <span style="width:8px;height:8px;border-radius:999px;background:var(--accent);" />
              <strong>Кастомный header</strong>
            </div>
          </template>
          <p>Слот `header` позволяет полностью переопределить шапку.</p>
        </Modal>
      </div>
    </Variant>

    <Variant title="Без закрытия по backdrop">
      <div class="story-canvas">
        <button class="open-btn" type="button" @click="openWide = true">Открыть (640px)</button>
        <Modal
          :open="openWide"
          title="Принудительный выбор"
          width="640px"
          :close-on-backdrop="false"
          @close="openWide = false"
        >
          <p>Backdrop игнорирует клики. Закрытие — только через крестик или Escape.</p>
          <template #footer>
            <button class="primary" type="button" @click="openWide = false">OK</button>
          </template>
        </Modal>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.open-btn,
.primary {
  padding: 0.5rem 1rem;
  border-radius: var(--radius-button);
  background: var(--accent);
  color: var(--accent-foreground);
  border: none;
  font-weight: 600;
  cursor: pointer;
}
.ghost {
  padding: 0.4rem 0.85rem;
  border-radius: var(--radius-input);
  background: transparent;
  border: 1px solid var(--border);
  color: var(--foreground);
  cursor: pointer;
}
.danger {
  padding: 0.4rem 0.85rem;
  border-radius: var(--radius-input);
  background: var(--destructive);
  color: #fff;
  border: none;
  font-weight: 600;
  cursor: pointer;
}
</style>

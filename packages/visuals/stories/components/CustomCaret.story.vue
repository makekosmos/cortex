<script setup lang="ts">
import CustomCaret from "../../components/CustomCaret.vue";
</script>

<template>
  <Story title="CustomCaret" group="primitives" :layout="{ type: 'single', iframe: true }">
    <Variant title="Inputs + contenteditable">
      <div class="story-canvas">
        <p class="story-label">
          Глобальный кастомный каретка: тёплый цвет `var(--accent)`, идёт через
          Teleport в body. Слушает фокус/выбор по всему документу. Работает с
          `&lt;input&gt;`, `&lt;textarea&gt;`, `[contenteditable]` (кроме ProseMirror —
          там свой каретка). Native caret скрыт через `caret-color: transparent`.
        </p>

        <CustomCaret />

        <div class="story-col" style="max-width: 480px;">
          <label class="story-label">input[type=text]</label>
          <input class="input" placeholder="Печатай — каретка следует за курсором" />

          <label class="story-label">textarea</label>
          <textarea class="input" rows="3">Многострочное поле — каретка</textarea>

          <label class="story-label">[contenteditable]</label>
          <div class="input" contenteditable="true">
            Этот блок editable — каретка адаптируется к высоте текста.
          </div>
        </div>
      </div>
    </Variant>
  </Story>
</template>

<style scoped>
.input {
  width: 100%;
  padding: 0.55rem 0.75rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--background);
  color: var(--foreground);
  font-family: inherit;
  font-size: 0.875rem;
  outline: none;
}
.input:focus,
.input:focus-visible {
  border-color: var(--accent);
}
</style>

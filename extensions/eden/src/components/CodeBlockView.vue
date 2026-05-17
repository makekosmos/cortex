<template>
  <NodeViewWrapper class="code-block-node">
    <div class="code-block-actions" contenteditable="false">
      <div class="code-block-actions-left">
        <button class="code-block-lang-btn" type="button" @click.stop="toggleMenu">
          {{ getCodeLanguageDisplayName(language) }}
          <svg
            class="code-block-lang-arrow"
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <polyline points="6 9 12 15 18 9" />
          </svg>
        </button>
        <div v-if="isMenuOpen" class="code-tools-menu" @click.stop>
          <input
            ref="langSearchRef"
            v-model="langSearch"
            class="code-tools-search"
            type="text"
            placeholder="Search..."
            @keydown.esc="closeMenu"
          />
          <div class="code-tools-languages">
            <button
              v-for="lang in filteredLanguages"
              :key="lang.id"
              :class="['code-language-option', lang.id === language && 'is-active']"
              type="button"
              @click="selectLanguage(lang.id)"
            >
              {{ lang.name }}
            </button>
          </div>
        </div>
      </div>
      <div class="code-block-actions-right">
        <button
          class="code-block-toolbar-btn"
          type="button"
          :title="isWrapped ? 'Disable wrapping' : 'Enable wrapping'"
          @click.stop="updateAttributes({ wrap: !isWrapped })"
        >
          {{ isWrapped ? "Unwrap" : "Wrap" }}
        </button>
        <button
          class="code-block-toolbar-btn"
          type="button"
          title="Copy code"
          @click.stop="copyCode"
        >
          Copy
        </button>
      </div>
    </div>
    <pre :data-wrap="isWrapped ? 'true' : 'false'">
      <NodeViewContent as="code" :class="`language-${language}`" />
    </pre>
  </NodeViewWrapper>
</template>

<script setup lang="ts">
import { ref, computed, watchEffect, nextTick } from "vue";
import { NodeViewWrapper, NodeViewContent, nodeViewProps } from "@tiptap/vue-3";
import {
  codeBlockLanguages,
  resolveLanguageId,
  getCodeLanguageDisplayName,
} from "@/lib/codeBlocks";

const props = defineProps(nodeViewProps);

const isMenuOpen = ref(false);
const langSearch = ref("");
const langSearchRef = ref<HTMLInputElement | null>(null);

const language = computed(() =>
  resolveLanguageId((props.node.attrs as { language?: string | null }).language),
);
const isWrapped = computed(() => (props.node.attrs as { wrap?: boolean }).wrap !== false);

const filteredLanguages = computed(() => {
  if (!langSearch.value) return codeBlockLanguages;
  const q = langSearch.value.toLowerCase();
  return codeBlockLanguages.filter(
    (lang) => lang.name.toLowerCase().includes(q) || lang.id.toLowerCase().includes(q),
  );
});

function toggleMenu() {
  isMenuOpen.value = !isMenuOpen.value;
  langSearch.value = "";
}

function closeMenu() {
  isMenuOpen.value = false;
  langSearch.value = "";
}

function selectLanguage(id: string) {
  props.updateAttributes({ language: id });
  closeMenu();
}

function copyCode() {
  void navigator.clipboard.writeText(props.node.textContent);
}

// Auto-focus search input when menu opens
watchEffect(() => {
  if (isMenuOpen.value) {
    nextTick(() => langSearchRef.value?.focus());
  }
});

// Click-outside to close menu
watchEffect((onCleanup) => {
  if (!isMenuOpen.value) return;
  const close = () => closeMenu();
  const timer = setTimeout(() => document.addEventListener("click", close), 0);
  onCleanup(() => {
    clearTimeout(timer);
    document.removeEventListener("click", close);
  });
});
</script>

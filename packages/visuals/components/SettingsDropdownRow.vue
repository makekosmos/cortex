<script setup lang="ts" generic="T extends string | number">
// SettingsDropdownRow — атомарная композиция SettingsRow + Dropdown.
// Устраняет повторяющийся паттерн <SettingsRow><template #control><div>
// <Dropdown .../></div></template></SettingsRow>, который встречался ≥ 10 раз
// между shell/SettingsView и arrancador/SettingsPage.

import { computed } from "vue";
import SettingsRow from "./SettingsRow.vue";
import Dropdown from "./Dropdown.vue";

interface Option<V> {
  value: V;
  label: string;
  description?: string;
  disabled?: boolean;
}

interface Props<V> {
  title: string;
  description?: string;
  modelValue: V | null;
  options: ReadonlyArray<Option<V>>;
  placeholder?: string;
  searchable?: boolean | "auto";
  searchPlaceholder?: string;
  disabled?: boolean;
  muted?: boolean;
}

const props = withDefaults(defineProps<Props<T>>(), {
  placeholder: undefined,
  searchable: "auto",
  searchPlaceholder: undefined,
  disabled: false,
  muted: false,
  description: undefined,
});

const emit = defineEmits<{
  "update:modelValue": [v: T];
}>();

const mutableOptions = computed(() => props.options.slice() as Option<T>[]);
</script>

<template>
  <SettingsRow :title="title" :description="description" :muted="muted">
    <template #control>
      <div>
        <Dropdown
          :model-value="modelValue"
          :options="mutableOptions"
          :placeholder="placeholder"
          :searchable="searchable"
          :search-placeholder="searchPlaceholder"
          :disabled="disabled"
          :match-trigger-width="false"
          @update:model-value="(v: T) => emit('update:modelValue', v)"
        >
          <template v-if="$slots['trigger-leading']" #trigger-leading="ctx">
            <slot name="trigger-leading" v-bind="ctx" />
          </template>
          <template v-if="$slots['option-leading']" #option-leading="ctx">
            <slot name="option-leading" v-bind="ctx" />
          </template>
        </Dropdown>
      </div>
    </template>
  </SettingsRow>
</template>

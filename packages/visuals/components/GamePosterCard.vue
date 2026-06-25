<script setup lang="ts">
import { computed, type Component } from "vue";
import { gamePosterCardClasses } from "../patterns";

interface GamePosterCardProps {
  to: string;
  title: string;
  eyebrow?: string | null;
  coverSrc?: string | null;
  className?: string;
  linkComponent?: string | Component;
  linkProps?: Record<string, unknown>;
}

const props = withDefaults(defineProps<GamePosterCardProps>(), {
  eyebrow: null,
  coverSrc: null,
  className: "",
  linkComponent: "a",
  linkProps: () => ({}),
});

const rootClassName = computed(() =>
  [gamePosterCardClasses.root, props.className].filter(Boolean).join(" "),
);

const linkAttrs = computed(() =>
  props.linkComponent === "a"
    ? {
        href: props.to,
        title: props.title,
        "aria-label": props.title,
      }
    : {
        to: props.to,
        title: props.title,
        "aria-label": props.title,
        ...props.linkProps,
      },
);
</script>

<template>
  <component :is="linkComponent" v-bind="linkAttrs" :class="rootClassName">
    <div :class="gamePosterCardClasses.media">
      <img
        v-if="coverSrc"
        :src="coverSrc"
        :alt="title"
        :class="gamePosterCardClasses.image"
        loading="lazy"
        decoding="async"
      />
      <div v-else :class="gamePosterCardClasses.placeholder">
        <slot name="placeholder" />
      </div>
      <div :class="gamePosterCardClasses.scrim" aria-hidden="true" />
      <div :class="gamePosterCardClasses.overlay" aria-hidden="true" />
    </div>

    <div :class="gamePosterCardClasses.content">
      <div v-if="eyebrow" :class="gamePosterCardClasses.eyebrow">
        {{ eyebrow }}
      </div>
      <div :class="gamePosterCardClasses.title">
        {{ title }}
      </div>
    </div>
  </component>
</template>

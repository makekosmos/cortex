import { shallowRef } from "vue";

const open = shallowRef(false);

export function useQuickEntry() {
  function show() {
    open.value = true;
  }

  function hide() {
    open.value = false;
  }

  function toggle() {
    open.value = !open.value;
  }

  return { open, show, hide, toggle };
}

import {
  computed,
  nextTick,
  onMounted,
  onUnmounted,
  ref,
  shallowRef,
  watch,
} from "vue";

// Module-level singleton — shared across all component instances
const sidebarHidden = shallowRef(false);

// Initialize from persisted config on module load
try {
  const raw = localStorage.getItem("delphi-sidebar-config");
  if (raw) {
    const cfg = JSON.parse(raw) as { hidden?: boolean };
    sidebarHidden.value = cfg.hidden ?? false;
  }
} catch {
  // ignore
}

export function setSidebarHidden(hidden: boolean) {
  sidebarHidden.value = hidden;
}

export function useSidebarState() {
  const wrapClass = "mx-auto w-full";
  const titleWrapRef = ref<HTMLElement | null>(null);
  const titleGroupRef = ref<HTMLElement | null>(null);
  const titleTranslateX = shallowRef(0);
  const titleGroupClass = "flex items-center gap-2.5 whitespace-nowrap will-change-transform";
  const titleGroupStyle = computed(() => ({
    left: "1.75rem",
    transform: `translate3d(${titleTranslateX.value}px, 0, 0)`,
    transition: "transform 0.35s cubic-bezier(0.22, 1, 0.36, 1)",
  }));
  const titleClass = computed(() =>
    sidebarHidden.value ? "text-center" : "text-left",
  );

  function updateTitleTranslate() {
    const wrapEl = titleWrapRef.value;
    const groupEl = titleGroupRef.value;
    if (!wrapEl || !groupEl) {
      titleTranslateX.value = 0;
      return;
    }

    if (!sidebarHidden.value) {
      titleTranslateX.value = 0;
      return;
    }

    const startLeft = 28;
    const wrapWidth = wrapEl.clientWidth;
    const groupWidth = groupEl.offsetWidth;
    titleTranslateX.value = Math.max((wrapWidth - groupWidth) / 2 - startLeft, 0);
  }

  let resizeObserver: ResizeObserver | null = null;

  onMounted(async () => {
    await nextTick();
    updateTitleTranslate();

    resizeObserver = new ResizeObserver(() => {
      updateTitleTranslate();
    });

    if (titleWrapRef.value) resizeObserver.observe(titleWrapRef.value);
    if (titleGroupRef.value) resizeObserver.observe(titleGroupRef.value);
  });

  onUnmounted(() => {
    resizeObserver?.disconnect();
    resizeObserver = null;
  });

  watch(sidebarHidden, async () => {
    await nextTick();
    updateTitleTranslate();
  });

  // Animate max-width so the zen ↔ normal transition is smooth.
  const wrapStyle = computed(() => ({
    maxWidth: sidebarHidden.value ? "var(--bringhurst-wide)" : "100%",
    transition: "max-width 0.35s cubic-bezier(0.22, 1, 0.36, 1)",
  }));

  return {
    sidebarHidden,
    wrapClass,
    wrapStyle,
    titleWrapRef,
    titleGroupRef,
    titleGroupClass,
    titleGroupStyle,
    titleClass,
  };
}

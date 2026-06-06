// useKeplerUpdate — composable, оборачивающий взаимодействие с
// `window.kepler.settings.update`. Выделен из SettingsView, чтобы
// несколько UI-фрагментов (UpdateBanner + AboutTab) делили один источник
// истины об апдейтах, не дублируя refs.
//
// Поведение строго соответствует исходному инлайн-коду в SettingsView:
// `state()` для refresh, `check()` ставит `updateChecking`, `install()` —
// триггер перезапуска. Сам подписчик на push-события навешивается отдельно
// (в SettingsView через `onUpdateState`), потому что owner-у удобнее
// контролировать lifecycle.

import { computed, ref } from "vue";
import type { UpdateState } from "@shared/ipc-types";

export interface UpdateBannerDescriptor {
  text: string;
  clickable: boolean;
  progress?: number;
}

export function useKeplerUpdate() {
  const updateState = ref<UpdateState>({ kind: "idle" });
  const updateChecking = ref<boolean>(false);

  async function refreshUpdateState() {
    try {
      updateState.value = await window.kepler.settings.update.state();
    } catch (e) {
      console.warn("update state fetch failed", e);
    }
  }

  async function onCheckUpdates() {
    if (updateChecking.value) return;
    updateChecking.value = true;
    try {
      updateState.value = await window.kepler.settings.update.check();
    } catch (e) {
      console.warn("update check failed", e);
    } finally {
      updateChecking.value = false;
    }
  }

  async function onInstallUpdate() {
    try {
      await window.kepler.settings.update.install();
    } catch (e) {
      console.warn("update install failed", e);
    }
  }

  const updateBanner = computed<UpdateBannerDescriptor | null>(() => {
    const s = updateState.value;
    if (s.kind === "available") {
      return {
        text: `Доступно обновление Kepler ${s.version}`,
        clickable: false,
      };
    }
    if (s.kind === "downloading") {
      return {
        text: `Скачивание Kepler ${s.version} (${s.percent}%)`,
        clickable: false,
        progress: s.percent,
      };
    }
    if (s.kind === "downloaded") {
      return {
        text: `Обновление Kepler ${s.version} готово — нажмите чтобы перезапустить`,
        clickable: true,
      };
    }
    return null;
  });

  const checkResultLabel = computed<string>(() => {
    const s = updateState.value;
    if (s.kind === "checking") return "Проверяем…";
    if (s.kind === "not-available") {
      return `Последняя версия (проверено ${new Date(s.checkedAt).toLocaleTimeString("ru")})`;
    }
    if (s.kind === "error") return `Ошибка: ${s.message}`;
    return "";
  });

  return {
    updateState,
    updateChecking,
    refreshUpdateState,
    onCheckUpdates,
    onInstallUpdate,
    updateBanner,
    checkResultLabel,
  };
}

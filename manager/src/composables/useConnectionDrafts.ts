import { ref } from "vue";
import type { IntegrationProvider } from "../manager-api";

export type ConnectionSetting = NonNullable<IntegrationProvider["settingSchema"]>[number];

export function useConnectionDrafts() {
  const credential = ref<Record<string, string>>({});
  const settingDraft = ref<Record<string, string>>({});

  function settingKey(provider: IntegrationProvider, setting: ConnectionSetting) {
    return `${provider.id}:${setting.key}`;
  }
  function settingValue(provider: IntegrationProvider, setting: ConnectionSetting) {
    return (
      settingDraft.value[settingKey(provider, setting)] ??
      provider.settingValues?.[setting.key] ??
      ""
    );
  }
  function setSettingValue(
    provider: IntegrationProvider,
    setting: ConnectionSetting,
    value: string,
  ) {
    settingDraft.value[settingKey(provider, setting)] = value;
  }
  function schema(provider: IntegrationProvider) {
    return provider.settingSchema ?? [];
  }
  function canSave(provider: IntegrationProvider) {
    return schema(provider).length > 0
      ? schema(provider).some((setting) => settingValue(provider, setting).trim())
      : Boolean(credential.value[provider.id]?.trim());
  }
  return { credential, settingKey, settingValue, setSettingValue, schema, canSave };
}

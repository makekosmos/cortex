// Recording-pill-only readiness helpers. Settings ownership moved to Manager.
export type DictationConfigData = {
  provider: string;
  model: string;
  localModelId: string;
  localModelPath: string | null;
};
export type DictationLocalModelsSnapshot = {
  commandInstalled?: boolean;
  models?: Array<{
    id: string;
    name: string;
    downloaded: boolean;
    transcriptionSupported?: boolean;
  }>;
};

export function normalizeDictationConfig(
  config?: Partial<DictationConfigData> | null,
): DictationConfigData {
  return {
    provider: config?.provider ?? "groq",
    model: config?.model ?? "whisper-large-v3-turbo",
    localModelId: config?.localModelId ?? "small",
    localModelPath: config?.localModelPath ?? null,
  };
}

export function buildDictationVoiceModelValue(
  config: Pick<DictationConfigData, "provider" | "model" | "localModelId">,
): string {
  return config.provider === "local"
    ? `local:${config.localModelId}`
    : config.provider === "groq"
      ? `groq:${config.model}`
      : "";
}

export function resolveAvailableDictationVoiceModelValue(
  config: Pick<DictationConfigData, "provider" | "model" | "localModelId">,
  args: { hasApiKey: boolean; localModels: DictationLocalModelsSnapshot | null },
): string | null {
  const current = buildDictationVoiceModelValue(config);
  const local = args.localModels?.commandInstalled
    ? args.localModels.models?.find(
        (model) => model.downloaded && model.transcriptionSupported !== false,
      )
    : undefined;
  if (current === `groq:${config.model}` && args.hasApiKey) return current;
  if (current === `local:${config.localModelId}` && local?.id === config.localModelId)
    return current;
  if (args.hasApiKey) return "groq:whisper-large-v3-turbo";
  return local ? `local:${local.id}` : null;
}

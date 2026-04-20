export const UNTITLED_ENTRY_PLACEHOLDER = "Без названия";
export const GENERATED_UNTITLED_ENTRY_PREFIX = "Новая заметка";
export const UNTITLED_ENTRY_TITLE_FLAG = "__untitledTitle";

type HeaderPropsSource = Record<string, unknown> | string | null | undefined;

function hasUntitledEntryTitleFlag(source: HeaderPropsSource): boolean {
  if (!source) {
    return false;
  }

  if (typeof source === "string") {
    try {
      return hasUntitledEntryTitleFlag(JSON.parse(source) as Record<string, unknown>);
    } catch {
      return false;
    }
  }

  return source[UNTITLED_ENTRY_TITLE_FLAG] === true;
}

function stripUntitledEntryTitleFlag(headerProps: Record<string, unknown>) {
  if (!(UNTITLED_ENTRY_TITLE_FLAG in headerProps)) {
    return headerProps;
  }

  const { [UNTITLED_ENTRY_TITLE_FLAG]: _removed, ...rest } = headerProps;
  return rest;
}

export function createUntitledEntryHeaderProps(): Record<string, boolean> {
  return {
    [UNTITLED_ENTRY_TITLE_FLAG]: true,
  };
}

export function isGeneratedUntitledEntryTitle(
  _title: string | null | undefined,
  headerPropsSource?: HeaderPropsSource,
): boolean {
  if (headerPropsSource !== undefined) {
    return hasUntitledEntryTitleFlag(headerPropsSource);
  }

  return false;
}

export function getEntryDisplayTitle(
  title: string | null | undefined,
  headerPropsSource?: HeaderPropsSource,
): string {
  const normalizedTitle = typeof title === "string" ? title.trim() : "";
  if (!normalizedTitle || isGeneratedUntitledEntryTitle(normalizedTitle, headerPropsSource)) {
    return UNTITLED_ENTRY_PLACEHOLDER;
  }

  return normalizedTitle;
}

export function getEditableEntryTitle(
  title: string | null | undefined,
  headerPropsSource?: HeaderPropsSource,
): string {
  if (isGeneratedUntitledEntryTitle(title, headerPropsSource)) {
    return "";
  }

  return title ?? "";
}

export function resolveStoredEntryTitle(
  editedTitle: string,
  persistedTitle: string,
  persistedHeaderPropsSource?: HeaderPropsSource,
): string {
  const normalizedEditedTitle = editedTitle.trim();
  if (normalizedEditedTitle) {
    return normalizedEditedTitle;
  }

  if (isGeneratedUntitledEntryTitle(persistedTitle, persistedHeaderPropsSource)) {
    return persistedTitle;
  }

  return normalizedEditedTitle;
}

export function syncUntitledEntryTitleFlag(
  headerProps: Record<string, unknown>,
  editedTitle: string,
  persistedTitle: string,
  persistedHeaderPropsSource?: HeaderPropsSource,
): Record<string, unknown> {
  const normalizedEditedTitle = editedTitle.trim();
  if (normalizedEditedTitle) {
    return stripUntitledEntryTitleFlag(headerProps);
  }

  if (isGeneratedUntitledEntryTitle(persistedTitle, persistedHeaderPropsSource)) {
    return {
      ...stripUntitledEntryTitleFlag(headerProps),
      [UNTITLED_ENTRY_TITLE_FLAG]: true,
    };
  }

  return stripUntitledEntryTitleFlag(headerProps);
}

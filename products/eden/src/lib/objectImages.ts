import { SYSTEM_TYPE_IMAGE_ID } from "@/lib/systemTypes";
import { toDisplayImageSrc } from "@/lib/localImages";

function parseHeaderProps(entry: Entry): Record<string, unknown> {
  try {
    const parsed = JSON.parse(entry.header_props_json || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function firstImageCandidate(value: unknown): string {
  if (Array.isArray(value)) {
    return String(value.find((item) => String(item ?? "").trim().length > 0) ?? "");
  }

  return String(value ?? "");
}

export function resolveObjectImageSrc(value: unknown, entriesById: Map<string, Entry>): string {
  const candidate = firstImageCandidate(value).trim();
  if (!candidate) return "";

  const linkedEntry = entriesById.get(candidate);
  if (linkedEntry?.type_id === SYSTEM_TYPE_IMAGE_ID) {
    const linkedProps = parseHeaderProps(linkedEntry);
    return toDisplayImageSrc(String(linkedProps.image ?? ""));
  }

  return toDisplayImageSrc(candidate);
}

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

function isDirectImageRef(value: string): boolean {
  return (
    /^[a-z][a-z0-9+.-]*:/i.test(value) || /^[A-Za-z]:[\\/]/.test(value) || value.startsWith("\\\\")
  );
}

export function resolveObjectImageSrc(value: unknown, entriesById: Map<string, Entry>): string {
  const candidate = firstImageCandidate(value).trim();
  if (!candidate) return "";

  const linkedEntry = entriesById.get(candidate);
  if (linkedEntry?.type_id === SYSTEM_TYPE_IMAGE_ID) {
    const linkedProps = parseHeaderProps(linkedEntry);
    const image = String(linkedProps.image ?? "").trim();
    const sourcePath = String(linkedProps.source_path ?? "").trim();
    return toDisplayImageSrc(isDirectImageRef(image) ? image : sourcePath || image);
  }

  return toDisplayImageSrc(candidate);
}

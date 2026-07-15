import { z } from "zod";
import { parseNoteTypeDefinition, type NoteType, type NoteTypeField } from "./typedNotes";

function createDefaultHeaderProps(noteType: NoteType | null) {
  if (!noteType) {
    return {};
  }

  const definition = parseNoteTypeDefinition(noteType.schema_json);

  return Object.fromEntries(
    definition.fields.map((field) => {
      switch (field.kind) {
        case "boolean":
          return [field.id, false];
        case "number":
          return [field.id, ""];
        case "multi_select":
          return [field.id, []];
        case "relation":
          return [field.id, field.multiple === false ? "" : []];
        default:
          return [field.id, ""];
      }
    }),
  );
}

function isPersonLikeNoteType(noteType: NoteType | null): boolean {
  if (!noteType) {
    return false;
  }

  try {
    const definition = parseNoteTypeDefinition(noteType.schema_json);
    const fieldIds = new Set(definition.fields.map((field) => field.id));
    return (
      noteType.slug === "person" ||
      (fieldIds.has("first_name") && fieldIds.has("last_name") && fieldIds.has("patronymic"))
    );
  } catch {
    return false;
  }
}

function splitPersonTitle(title: string): {
  firstName: string;
  lastName: string;
  patronymic: string;
} {
  const parts = title.trim().split(/\s+/).filter(Boolean);

  return {
    firstName: parts[0] ?? "",
    lastName: parts[1] ?? "",
    patronymic: parts.slice(2).join(" "),
  };
}

export function createHeaderPropsForTypeChange(noteType: NoteType | null, sourceTitle: string) {
  const props = createDefaultHeaderProps(noteType);
  if (!isPersonLikeNoteType(noteType)) {
    return props;
  }

  const { firstName, lastName, patronymic } = splitPersonTitle(sourceTitle);

  return {
    ...props,
    first_name: firstName,
    last_name: lastName,
    patronymic,
  };
}

function coerceHeaderFieldValue(field: NoteTypeField, value: unknown) {
  switch (field.kind) {
    case "text":
    case "url":
    case "long_text":
    case "date":
    case "image":
      if (typeof value === "string") {
        return value;
      }

      if (typeof value === "number" && Number.isFinite(value)) {
        return String(value);
      }

      return "";
    case "number":
      if (typeof value === "number" && Number.isFinite(value)) {
        return value;
      }

      if (typeof value === "string") {
        const trimmed = value.trim();
        return /^$|^-?\d+(\.\d+)?$/.test(trimmed) ? trimmed : "";
      }

      return "";
    case "boolean":
      if (value === true || value === false) {
        return value;
      }

      if (value === 1 || value === "1" || value === "true") {
        return true;
      }

      if (value === 0 || value === "0" || value === "false") {
        return false;
      }

      return false;
    case "multi_select":
      if (Array.isArray(value)) {
        return value
          .filter((item): item is string => typeof item === "string")
          .map((item) => item.trim())
          .filter(Boolean);
      }

      if (typeof value === "string") {
        return value
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean);
      }

      return [];
    case "relation":
      if (field.multiple === false) {
        if (typeof value === "string") {
          return value.trim();
        }

        if (Array.isArray(value)) {
          const firstValue = value.find((item): item is string => typeof item === "string");
          return firstValue?.trim() ?? "";
        }

        return "";
      }

      if (Array.isArray(value)) {
        return value
          .filter((item): item is string => typeof item === "string")
          .map((item) => item.trim())
          .filter(Boolean);
      }

      if (typeof value === "string") {
        const trimmed = value.trim();
        return trimmed ? [trimmed] : [];
      }

      return [];
    case "select":
      if (typeof value === "string") {
        return value;
      }

      if (typeof value === "number" && Number.isFinite(value)) {
        return String(value);
      }

      if (Array.isArray(value)) {
        const firstValue = value.find((item): item is string => typeof item === "string");
        return firstValue ?? "";
      }

      return "";
    default:
      return value;
  }
}

function normalizeHeaderPropsRecord(noteType: NoteType | null, rawProps: Record<string, unknown>) {
  if (!noteType) {
    return rawProps;
  }

  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const normalized: Record<string, unknown> = { ...rawProps };

  for (const field of definition.fields) {
    normalized[field.id] = coerceHeaderFieldValue(field, rawProps[field.id]);
  }

  return normalized;
}

export function normalizeHeaderProps(noteType: NoteType | null, rawProps: unknown) {
  if (!rawProps || typeof rawProps !== "object" || Array.isArray(rawProps)) {
    return createDefaultHeaderProps(noteType);
  }

  return {
    ...createDefaultHeaderProps(noteType),
    ...normalizeHeaderPropsRecord(noteType, rawProps as Record<string, unknown>),
  };
}

function buildHeaderPropsSchema(noteType: NoteType | null) {
  if (!noteType) {
    return z.record(z.string(), z.unknown());
  }

  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const shape: Record<string, z.ZodTypeAny> = {};

  for (const field of definition.fields) {
    let schema: z.ZodTypeAny;

    switch (field.kind) {
      case "number":
        schema = z.union([z.number(), z.string().regex(/^$|^-?\d+(\.\d+)?$/)]);
        break;
      case "date":
        schema = z.string();
        break;
      case "boolean":
        schema = z.boolean();
        break;
      case "multi_select":
        schema = z.array(z.string());
        break;
      case "relation":
        schema = field.multiple === false ? z.string() : z.array(z.string());
        break;
      default:
        schema = z.string();
        break;
    }

    shape[field.id] = field.required ? schema : schema.optional();
  }

  return z.object(shape).passthrough();
}

export function validateHeaderProps(noteType: NoteType | null, rawProps: unknown) {
  return buildHeaderPropsSchema(noteType).safeParse(normalizeHeaderProps(noteType, rawProps));
}

export function safeParseHeaderProps(
  noteType: NoteType | null,
  rawJson: string | null | undefined,
) {
  try {
    const parsed = rawJson ? JSON.parse(rawJson) : {};
    const result = validateHeaderProps(noteType, parsed);

    if (result.success) {
      return result.data as Record<string, unknown>;
    }
  } catch {
    return createDefaultHeaderProps(noteType);
  }

  return createDefaultHeaderProps(noteType);
}

import { describe, expect, test } from "vitest";
import { objectIconUri } from "../src/lib/iconResolver";
import {
  SYSTEM_TYPE_BOOK,
  SYSTEM_TYPE_BOOK_ID,
  SYSTEM_TYPE_IMAGE,
  SYSTEM_TYPE_IMAGE_ID,
  SYSTEM_TYPES,
  isSystemType,
} from "../src/lib/systemTypes";
import {
  parseHeaderTemplate,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  resolveNoteTypeFields,
} from "../src/lib/typedNotes";

describe("Eden system type book", () => {
  test("is registered with the book metadata contract", () => {
    expect(SYSTEM_TYPE_BOOK_ID).toBe("book_obj");
    expect(isSystemType(SYSTEM_TYPE_BOOK_ID)).toBe(true);
    expect(SYSTEM_TYPES.map((noteType) => noteType.id)).toContain(SYSTEM_TYPE_BOOK_ID);
    expect(SYSTEM_TYPE_BOOK).toMatchObject({
      name: "Книга",
      slug: "book_obj",
      icon: "book",
      color: null,
    });

    const definition = parseNoteTypeDefinition(SYSTEM_TYPE_BOOK.schema_json);
    expect(definition.fields).toEqual([
      expect.objectContaining({ id: "cover_image", kind: "image", required: false }),
      expect.objectContaining({ id: "author", kind: "text", required: false }),
      expect.objectContaining({ id: "isbn", kind: "text", required: false }),
      expect.objectContaining({ id: "page_count", kind: "number", required: false }),
      expect.objectContaining({
        id: "language",
        kind: "select",
        required: false,
        options: expect.arrayContaining(["Русский", "Английский"]),
      }),
      expect.objectContaining({ id: "publisher", kind: "text", required: false }),
      expect.objectContaining({ id: "published_date", kind: "text", required: false }),
      expect.objectContaining({ id: "source_url", kind: "url", required: false }),
    ]);

    const headerTemplate = parseHeaderTemplate(SYSTEM_TYPE_BOOK.header_template_json);
    expect(headerTemplate.imageFieldId).toBe("cover_image");

    const uiSchema = parseNoteTypeUiSchema(SYSTEM_TYPE_BOOK.ui_schema_json);
    expect(uiSchema).toMatchObject({
      featured_fields: ["author"],
      visible_fields: [
        "cover_image",
        "author",
        "isbn",
        "page_count",
        "language",
        "publisher",
        "published_date",
        "source_url",
      ],
      field_order: [
        "cover_image",
        "author",
        "isbn",
        "page_count",
        "language",
        "publisher",
        "published_date",
        "source_url",
      ],
      collection_name: "Книги",
    });
  });
});

describe("Eden system type image", () => {
  test("is registered as a system type and exposes the image schema contract", () => {
    expect(SYSTEM_TYPE_IMAGE_ID).toBe("image_obj");
    expect(isSystemType(SYSTEM_TYPE_IMAGE_ID)).toBe(true);
    expect(SYSTEM_TYPES.map((noteType) => noteType.id)).toContain(SYSTEM_TYPE_IMAGE_ID);

    const definition = parseNoteTypeDefinition(SYSTEM_TYPE_IMAGE.schema_json);
    expect(definition.fields.map((field) => field.id)).toEqual([
      "image",
      "file_name",
      "mime_type",
      "size_bytes",
      "width",
      "height",
      "resolution",
      "source_path",
      "alt_text",
    ]);
    expect(definition.fields.find((field) => field.id === "image")).toMatchObject({
      kind: "image",
      required: true,
      visible: true,
      read_only: false,
    });
    expect(definition.fields.find((field) => field.id === "source_path")).toMatchObject({
      visible: false,
      read_only: true,
      system: true,
    });

    const uiSchema = parseNoteTypeUiSchema(SYSTEM_TYPE_IMAGE.ui_schema_json);
    expect(uiSchema.collection_name).toBe("Изображения");
    expect(uiSchema.visible_fields).toEqual([
      "image",
      "file_name",
      "size_bytes",
      "width",
      "height",
      "resolution",
      "alt_text",
    ]);
    expect(uiSchema.hidden_fields).toEqual([
      "created_at",
      "updated_at",
      "deleted_at",
      "mime_type",
      "source_path",
    ]);
    expect(uiSchema.read_only_fields).toEqual([
      "mime_type",
      "source_path",
      "size_bytes",
      "width",
      "height",
      "resolution",
    ]);

    const resolved = resolveNoteTypeFields(SYSTEM_TYPE_IMAGE);
    expect(resolved.find((field) => field.id === "image")).toMatchObject({
      visible: true,
      read_only: false,
    });
    expect(resolved.find((field) => field.id === "mime_type")).toMatchObject({
      visible: false,
      read_only: true,
    });
    expect(resolved.find((field) => field.id === "resolution")).toMatchObject({
      visible: true,
      read_only: true,
    });
  });

  test("image icon resolves to a dedicated SVG mapping", () => {
    const svg = decodeURIComponent(objectIconUri("image").split(",")[1] ?? "");

    expect(svg).toContain("<svg");
    expect(svg).toContain("<rect");
    expect(svg).toContain("<circle");
  });
});

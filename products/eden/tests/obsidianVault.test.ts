import { describe, expect, test } from "bun:test";
import type { NoteType } from "../src/lib/typedNotes.js";
import {
  buildObsidianRelatedImportPlan,
  createObsidianVaultImportDrafts,
  exportObsidianVaultMarkdownFiles,
} from "../src/lib/obsidianVault.js";

function makeNoteType(overrides: Partial<NoteType> = {}): NoteType {
  return {
    id: "custom-note",
    name: "Custom Note",
    slug: "custom-note",
    icon: null,
    color: null,
    schema_json: JSON.stringify({ fields: [] }),
    header_template_json: JSON.stringify({
      kind: "default",
      primaryFieldIds: [],
      secondaryFieldIds: [],
      imageFieldId: null,
    }),
    ui_schema_json: JSON.stringify({}),
    created_at: 0,
    updated_at: 0,
    ...overrides,
  };
}

describe("obsidianVault import", () => {
  test("treats files without frontmatter as body markdown and infers the title from the filename", () => {
    const drafts = createObsidianVaultImportDrafts({
      files: [
        {
          path: "vault/notes/My Note.md",
          relativePath: "notes/My Note.md",
          name: "My Note.md",
          content: "First line\n\nSecond line",
        },
      ],
      noteTypes: [makeNoteType()],
      defaultNoteTypeId: "note_obj",
    });

    expect(drafts).toHaveLength(1);
    expect(drafts[0]).toMatchObject({
      sourcePath: "vault/notes/My Note.md",
      relativePath: "notes/My Note.md",
      name: "My Note.md",
      title: "My Note",
      typeId: "note_obj",
      bodyMarkdown: "First line\n\nSecond line",
      headerProps: {},
      titleReferences: [],
      imageReferences: [],
    });
  });

  test("extracts wikilinks from body and frontmatter and keeps non-reserved frontmatter in headerProps", () => {
    const drafts = createObsidianVaultImportDrafts({
      files: [
        {
          path: "vault/notes/Obsidian Note.md",
          relativePath: "notes/Obsidian Note.md",
          name: "Obsidian Note.md",
          content: [
            "---",
            "title: Vault Note",
            "eden:",
            "  id: note-1",
            "  type: custom-note",
            'project: "Alpha"',
            "links:",
            "  related:",
            '    - "[[Front Link]]"',
            'notes: "See [[Front Matter Link]]"',
            "---",
            "",
            "Intro [[Body Link|alias]]",
            '![](attachments/photo.png "Photo")',
          ].join("\n"),
        },
      ],
      noteTypes: [makeNoteType()],
      defaultNoteTypeId: "note_obj",
    });

    expect(drafts).toHaveLength(1);
    expect(drafts[0]).toMatchObject({
      entryId: "note-1",
      title: "Vault Note",
      typeId: "custom-note",
      bodyMarkdown: '\nIntro [[Body Link|alias]]\n![](attachments/photo.png "Photo")',
      headerProps: {
        project: "Alpha",
        notes: "See [[Front Matter Link]]",
      },
      titleReferences: ["Body Link", "Front Link", "Front Matter Link"],
      imageReferences: ["attachments/photo.png"],
    });
  });

  test("accepts loose Obsidian frontmatter and image paths with spaces", () => {
    const drafts = createObsidianVaultImportDrafts({
      files: [
        {
          path: "vault/Дневник/2026-06-12.md",
          relativePath: "Дневник/2026-06-12.md",
          name: "2026-06-12.md",
          content: [
            "\uFEFF---   ",
            "title: 2026-06-12",
            'tags: ["дневник", "важное"]',
            'aliases: [Сегодня, "Daily note"]',
            'Автор: "[[Я]]"',
            "...",
            "",
            "Текст [[Project Alpha#Intro|intro]].",
            '![scan](../Медиа/My Image 01.webp "scan")',
            "![[Another Image.webp|300x200]]",
          ].join("\n"),
        },
      ],
      images: [
        {
          path: "vault/Медиа/My Image 01.webp",
          relativePath: "Медиа/My Image 01.webp",
          name: "My Image 01.webp",
          fileUrl: "file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/My%20Image%2001.webp",
          mimeType: "image/webp",
          sizeBytes: 100,
          width: 800,
          height: 600,
        },
        {
          path: "vault/Медиа/Another Image.webp",
          relativePath: "Медиа/Another Image.webp",
          name: "Another Image.webp",
          fileUrl: "file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/Another%20Image.webp",
          mimeType: "image/webp",
          sizeBytes: 200,
          width: 300,
          height: 200,
        },
      ],
      noteTypes: [makeNoteType()],
      defaultNoteTypeId: "note_obj",
    });

    expect(drafts).toHaveLength(1);
    expect(drafts[0]).toMatchObject({
      title: "2026-06-12",
      typeId: "note_obj",
      headerProps: {
        tags: ["дневник", "важное"],
        aliases: ["Сегодня", "Daily note"],
        Автор: "[[Я]]",
      },
      titleReferences: ["Project Alpha#Intro", "Я"],
      imageReferences: [
        "file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/My%20Image%2001.webp",
        "file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/Another%20Image.webp",
      ],
    });
    expect(drafts[0]?.bodyMarkdown).toContain(
      "![scan](file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/My%20Image%2001.webp)",
    );
    expect(drafts[0]?.bodyMarkdown).toContain(
      "![Another Image](file:///vault/%D0%9C%D0%B5%D0%B4%D0%B8%D0%B0/Another%20Image.webp)",
    );
  });

  test("plans wikilink relations for a second pass after all imported objects exist", () => {
    // Regression: 2026-06-12. First-pass related_notes created ARK object_links
    // before forward-linked imported notes existed, causing a SQLite FK failure.
    const plan = buildObsidianRelatedImportPlan({
      entryId: "entry-a",
      draft: {
        title: "A",
        headerProps: {
          related_notes: ["raw-obsidian-title"],
          source_path: "vault/A.md",
        },
        wikilinks: ["B#Heading", "Existing.md", "A"],
      },
      importedTitleIds: new Map([
        ["a", "entry-a"],
        ["b", "entry-b"],
      ]),
      existingTitleIds: new Map([["existing", "entry-existing"]]),
    });

    expect(plan.firstPassHeaderProps).toEqual({
      source_path: "vault/A.md",
    });
    expect(plan.secondPassRelatedIds).toEqual(["entry-b", "entry-existing"]);
  });
});

describe("obsidianVault export", () => {
  test("uses a safe filename and writes markdown frontmatter via buildEntryMarkdownDocument", () => {
    const files = exportObsidianVaultMarkdownFiles({
      entries: [
        {
          id: "entry-1",
          title: "My: Plan/2026",
          type_id: "custom-note",
          header_props_json: JSON.stringify({
            summary: "Draft",
            related_notes: ["entry-2"],
          }),
          schema_version: 2,
        },
      ],
      noteTypes: [makeNoteType()],
      bodyMarkdownLookup: new Map([["entry-1", "Hello body"]]),
      titleLookup: new Map([["entry-2", "Second Note"]]),
    });

    expect(files).toHaveLength(1);
    expect(files[0]?.relativePath).toBe("My-Plan-2026.md");
    expect(files[0]?.content).toContain("---\n");
    expect(files[0]?.content).toContain('title: "My: Plan/2026"');
    expect(files[0]?.content).toContain("type: custom-note");
    expect(files[0]?.content).toContain("summary: Draft");
    expect(files[0]?.content).toContain("links:");
    expect(files[0]?.content).toContain('- "[[entry-2|Second Note]]"');
    expect(files[0]?.content).toContain("Hello body");
  });
});

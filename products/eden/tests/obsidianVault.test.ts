import { describe, expect, test } from "bun:test";
import type { NoteType } from "../src/lib/typedNotes.js";
import {
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

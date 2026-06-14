import { describe, expect, test } from "bun:test";
import type { NoteType } from "../src/lib/typedNotes.js";
import {
  buildObsidianExportFiles,
  buildObsidianFolderPathLookup,
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

  test("preserves folder structure and excludes unselected types when exporting", () => {
    const folderPaths = buildObsidianFolderPathLookup([
      { id: "root", name: "Projects", parent_id: null },
      { id: "child", name: "2026 Alpha", parent_id: "root" },
    ]);

    const files = buildObsidianExportFiles({
      entries: [
        {
          id: "entry-a",
          title: "Сводка",
          type_id: "custom-note",
          folder_id: "child",
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
        {
          id: "entry-b",
          title: "Сводка",
          type_id: "custom-note",
          folder_id: "child",
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
        {
          id: "task-1",
          title: "Сводка",
          type_id: "task_obj",
          folder_id: "child",
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
      ],
      noteTypes: [makeNoteType(), makeNoteType({ id: "task_obj", name: "Task", slug: "task" })],
      selectedTypeIds: ["custom-note"],
      folderPathById: folderPaths,
      bodyMarkdownById: (entry) => `Body ${entry.id}`,
    });

    expect(files.map((file) => file.relativePath)).toEqual([
      "Projects/2026 Alpha/Сводка.md",
      "Projects/2026 Alpha/Сводка-2.md",
    ]);
    expect(files.every((file) => !file.relativePath.includes("task"))).toBe(true);
    expect(files[0]?.content).toContain("Body entry-a");
    expect(files[1]?.content).toContain("Body entry-b");
  });

  test("copies local file-backed assets, rewrites their markdown refs, and leaves unresolved refs unchanged", () => {
    const folderPaths = buildObsidianFolderPathLookup([
      { id: "root", name: "Projects", parent_id: null },
      { id: "child", name: "2026 Alpha", parent_id: "root" },
    ]);

    const files = buildObsidianExportFiles({
      entries: [
        {
          id: "entry-a",
          title: "Сводка",
          type_id: "custom-note",
          folder_id: "child",
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
        {
          id: "entry-b",
          title: "Сводка",
          type_id: "custom-note",
          folder_id: "child",
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
        {
          id: "task-1",
          title: "Сводка",
          type_id: "task_obj",
          folder_id: "child",
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
      ],
      noteTypes: [makeNoteType(), makeNoteType({ id: "task_obj", name: "Task", slug: "task" })],
      selectedTypeIds: ["custom-note"],
      folderPathById: folderPaths,
      bodyMarkdownById: (entry) =>
        entry.id === "entry-a"
          ? "Note body\n\n![](file:///C:/vault/attachments/diagram.png)\n![[media/photo 1.webp|cover]]\n![](../../../../evil.png)"
          : `Body ${entry.id}`,
    });

    expect(files.map((file) => file.relativePath)).toEqual([
      "Projects/2026 Alpha/Сводка.md",
      "Projects/2026 Alpha/Сводка-2.md",
      "assets/diagram.png",
      "assets/obsidian-asset-manifest.json",
    ]);
    expect(files.every((file) => !file.relativePath.includes("task"))).toBe(true);
    expect(files[0]?.content).toContain("../../assets/diagram.png");
    expect(files[0]?.content).toContain("![[media/photo 1.webp|cover]]");
    expect(files[0]?.content).toContain("![](../../../../evil.png)");
    expect(files[2]).toMatchObject({
      relativePath: "assets/diagram.png",
      sourcePath: "file:///C:/vault/attachments/diagram.png",
    });

    const manifest = JSON.parse(files[3]?.content ?? "{}");
    expect(manifest).toMatchObject({
      limitation: expect.stringContaining("safe local source path"),
      assets: [
        {
          source: "media/photo 1.webp",
          sourcePath: null,
          noteRelativePath: "Projects/2026 Alpha/Сводка.md",
          targetRelativePath: "assets/media/photo 1.webp",
          rewrittenRelativePath: null,
          copyable: false,
          limitation: expect.stringContaining("kept unchanged"),
        },
        {
          source: "../../../../evil.png",
          sourcePath: null,
          noteRelativePath: "Projects/2026 Alpha/Сводка.md",
          targetRelativePath: "assets/evil.png",
          rewrittenRelativePath: null,
          copyable: false,
          limitation: expect.stringContaining("kept unchanged"),
        },
      ],
    });
  });

  test("copies raw absolute Windows image paths and preserves their extensions in the exported vault", () => {
    const folderPaths = buildObsidianFolderPathLookup([
      { id: "root", name: "Projects", parent_id: null },
      { id: "child", name: "2026 Alpha", parent_id: "root" },
    ]);
    const rawWindowsImagePath = String.raw`C:\vault\attachments\My Image 01.webp`;

    const files = buildObsidianExportFiles({
      entries: [
        {
          id: "entry-a",
          title: "Сводка",
          type_id: "custom-note",
          folder_id: "child",
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
      ],
      noteTypes: [makeNoteType()],
      folderPathById: folderPaths,
      bodyMarkdownById: () => `Note body\n\n![](${rawWindowsImagePath})`,
    });

    expect(files.map((file) => file.relativePath)).toEqual([
      "Projects/2026 Alpha/Сводка.md",
      "assets/My Image 01.webp",
    ]);
    expect(files[0]?.content).toContain("../../assets/My Image 01.webp");
    expect(files[1]).toMatchObject({
      relativePath: "assets/My Image 01.webp",
      sourcePath: rawWindowsImagePath,
    });
    expect(files.some((file) => file.relativePath === "assets/obsidian-asset-manifest.json")).toBe(
      false,
    );
  });

  test("exports Eden image objects as real asset files from header metadata", () => {
    const sourcePath = String.raw`C:\eden\assets\stored-image`;
    const imageType = makeNoteType({
      id: "image_obj",
      name: "Image",
      slug: "image",
      schema_json: JSON.stringify({
        fields: [{ id: "image", label: "Image", kind: "image", required: true, visible: true }],
      }),
      header_template_json: JSON.stringify({
        kind: "default",
        primaryFieldIds: ["image"],
        secondaryFieldIds: ["file_name"],
        imageFieldId: "image",
      }),
    });

    const files = buildObsidianExportFiles({
      entries: [
        {
          id: "image-1",
          title: "Vacation Photo",
          type_id: "image_obj",
          folder_id: null,
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: JSON.stringify({
            image: "kosmos-local-image://file/C%3A%5Ceden%5Cassets%5Cstored-image",
            file_name: "Vacation Photo.JPG",
            mime_type: "image/jpeg",
            source_path: sourcePath,
          }),
          schema_version: 1,
          deleted_at: null,
        },
      ],
      noteTypes: [imageType],
      bodyMarkdownById: () => "",
    });

    expect(files.map((file) => file.relativePath)).toEqual([
      "Vacation-Photo.md",
      "assets/Vacation Photo.JPG",
    ]);
    expect(files[0]?.content).toContain('image: "assets/Vacation Photo.JPG"');
    expect(files[1]).toMatchObject({
      relativePath: "assets/Vacation Photo.JPG",
      sourcePath,
    });
    expect(files.some((file) => file.relativePath === "assets/obsidian-asset-manifest.json")).toBe(
      false,
    );
  });

  test("preserves asset extensions from inline image metadata objects", () => {
    const sourcePath = String.raw`C:\eden\photos\portrait-source`;
    const imageType = makeNoteType({
      id: "image_obj",
      name: "Image",
      slug: "image",
      schema_json: JSON.stringify({
        fields: [{ id: "image", label: "Image", kind: "image", required: true, visible: true }],
      }),
      header_template_json: JSON.stringify({
        kind: "default",
        primaryFieldIds: ["image"],
        secondaryFieldIds: ["file_name"],
        imageFieldId: "image",
      }),
    });

    const files = buildObsidianExportFiles({
      entries: [
        {
          id: "image-2",
          title: "Portrait",
          type_id: "image_obj",
          folder_id: null,
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: JSON.stringify({
            image: {
              source_path: sourcePath,
              file_name: "Portrait",
              mime_type: "image/png",
            },
          }),
          schema_version: 1,
          deleted_at: null,
        },
      ],
      noteTypes: [imageType],
      bodyMarkdownById: () => "",
    });

    expect(files.map((file) => file.relativePath)).toEqual(["Portrait.md", "assets/Portrait.png"]);
    expect(files[0]?.content).toContain("image: assets/Portrait.png");
    expect(files[1]).toMatchObject({
      relativePath: "assets/Portrait.png",
      sourcePath,
    });
    expect(files.some((file) => file.relativePath === "assets/obsidian-asset-manifest.json")).toBe(
      false,
    );
  });

  test("rewrites header image-object references to copied asset paths when image objects are excluded", () => {
    const sourcePath = String.raw`C:\eden\images\Profile Portrait.png`;
    const personType = makeNoteType({
      id: "person_obj",
      name: "Person",
      slug: "person",
      schema_json: JSON.stringify({
        fields: [{ id: "photo", label: "Photo", kind: "image", required: false, visible: true }],
      }),
      header_template_json: JSON.stringify({
        kind: "default",
        primaryFieldIds: [],
        secondaryFieldIds: ["photo"],
        imageFieldId: "photo",
      }),
    });
    const imageType = makeNoteType({
      id: "image_obj",
      name: "Image",
      slug: "image",
      schema_json: JSON.stringify({
        fields: [{ id: "image", label: "Image", kind: "image", required: true, visible: true }],
      }),
      header_template_json: JSON.stringify({
        kind: "default",
        primaryFieldIds: ["image"],
        secondaryFieldIds: ["file_name"],
        imageFieldId: "image",
      }),
    });

    const files = buildObsidianExportFiles({
      entries: [
        {
          id: "person-1",
          title: "Ada",
          type_id: "person_obj",
          folder_id: null,
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: JSON.stringify({ photo: "image-1" }),
          schema_version: 1,
          deleted_at: null,
        },
        {
          id: "image-1",
          title: "Profile Portrait",
          type_id: "image_obj",
          folder_id: null,
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: JSON.stringify({
            image: "file:///C:/eden/images/Profile%20Portrait.png",
            file_name: "Profile Portrait.png",
            mime_type: "image/png",
            source_path: sourcePath,
          }),
          schema_version: 1,
          deleted_at: null,
        },
      ],
      noteTypes: [personType, imageType],
      selectedTypeIds: ["person_obj"],
      bodyMarkdownById: () => "",
    });

    expect(files.map((file) => file.relativePath)).toEqual([
      "Ada.md",
      "assets/Profile Portrait.png",
    ]);
    expect(files[0]?.content).toContain('photo: "assets/Profile Portrait.png"');
    expect(files[1]).toMatchObject({
      relativePath: "assets/Profile Portrait.png",
      sourcePath,
    });
    expect(files.some((file) => file.relativePath === "assets/obsidian-asset-manifest.json")).toBe(
      false,
    );
  });

  test("deduplicates copied asset targets when different local assets collide on the same filename", () => {
    const files = buildObsidianExportFiles({
      entries: [
        {
          id: "entry-a",
          title: "A",
          type_id: "custom-note",
          folder_id: null,
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
        {
          id: "entry-b",
          title: "B",
          type_id: "custom-note",
          folder_id: null,
          content_json: "{}",
          created_at: 0,
          updated_at: 0,
          header_layout: null,
          header_props_json: "{}",
          schema_version: 1,
          deleted_at: null,
        },
      ],
      noteTypes: [makeNoteType()],
      bodyMarkdownById: (entry) =>
        entry.id === "entry-a"
          ? "![](file:///C:/vault/a/diagram.png)"
          : "![](file:///C:/vault/b/diagram.png)",
    });

    expect(files.map((file) => file.relativePath)).toEqual([
      "A.md",
      "B.md",
      "assets/diagram.png",
      "assets/diagram-2.png",
    ]);
    expect(files[0]?.content).toContain("assets/diagram.png");
    expect(files[1]?.content).toContain("assets/diagram-2.png");
    expect(files[2]).toMatchObject({ sourcePath: "file:///C:/vault/a/diagram.png" });
    expect(files[3]).toMatchObject({ sourcePath: "file:///C:/vault/b/diagram.png" });
  });
});

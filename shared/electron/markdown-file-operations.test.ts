import { afterEach, describe, expect, test } from "bun:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { performMarkdownFileOperation, type MarkdownFileDialogs } from "./markdown-file-operations";

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function fixture(): { root: string; dialogs: MarkdownFileDialogs } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-markdown-operation-"));
  roots.push(root);
  const dialogs: MarkdownFileDialogs = {
    openFile: async () => path.join(root, "note.md"),
    openDirectory: async (mode) => path.join(root, mode),
    saveFile: async () => path.join(root, "saved.md"),
  };
  return { root, dialogs };
}

describe("Markdown file operations", () => {
  test("opens and saves only bounded Markdown", async () => {
    const { root, dialogs } = fixture();
    fs.writeFileSync(path.join(root, "note.md"), "# Note");
    expect(await performMarkdownFileOperation("open", {}, dialogs)).toEqual({
      path: path.join(root, "note.md"),
      name: "note.md",
      content: "# Note",
    });
    expect(
      await performMarkdownFileOperation(
        "save",
        { suggestedName: "saved", content: "# Saved" },
        dialogs,
      ),
    ).toEqual({ path: path.join(root, "saved.md") });
    expect(fs.readFileSync(path.join(root, "saved.md"), "utf8")).toBe("# Saved");
    await expect(
      performMarkdownFileOperation("save", { content: "x".repeat(5 * 1024 * 1024 + 1) }, dialogs),
    ).rejects.toThrow("Invalid Markdown content");
  });

  test("exports safe unique files through the supplied asset boundary", async () => {
    const { root, dialogs } = fixture();
    fs.mkdirSync(path.join(root, "export"));
    const copied: string[] = [];
    expect(
      await performMarkdownFileOperation(
        "exportVault",
        {
          files: [
            { relativePath: "notes/a.md", content: "# A" },
            { relativePath: "assets/a.png", sourcePath: "approved.png" },
          ],
        },
        dialogs,
        async (source, output) => {
          copied.push(source);
          fs.writeFileSync(output, "image");
          return true;
        },
      ),
    ).toEqual({ outputDir: path.join(root, "export"), exportedCount: 2 });
    expect(copied).toEqual(["approved.png"]);
    await expect(
      performMarkdownFileOperation(
        "exportVault",
        { files: [{ relativePath: "../escape.md", content: "x" }] },
        dialogs,
      ),
    ).rejects.toThrow();
    if (process.platform === "win32") {
      await expect(
        performMarkdownFileOperation(
          "exportVault",
          {
            files: [
              { relativePath: "A.md", content: "A" },
              { relativePath: "a.md", content: "a" },
            ],
          },
          dialogs,
        ),
      ).rejects.toThrow("Duplicate Markdown vault path");
    }
  });
});

import { afterEach, describe, expect, test } from "../test-support/node-test.mjs";
import { mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  resolveMarkdownVaultSourcePath,
  safeMarkdownDefaultName,
  safeVaultOutputPath,
  scanMarkdownVault,
} from "./markdown-vault";

const tempRoots: string[] = [];

function tempDir(): string {
  const root = path.join(tmpdir(), `kosmos-markdown-vault-${Date.now()}-${tempRoots.length}`);
  mkdirSync(root, { recursive: true });
  tempRoots.push(root);
  return root;
}

afterEach(() => {
  for (const root of tempRoots.splice(0)) {
    rmSync(root, { recursive: true, force: true });
  }
});

describe("safeMarkdownDefaultName", () => {
  test("falls back for non-string and empty names", () => {
    expect(safeMarkdownDefaultName(null)).toBe("eden-object.md");
    expect(safeMarkdownDefaultName("   ")).toBe("eden-object.md");
  });

  test("strips directories, replaces illegal characters, and appends .md", () => {
    expect(safeMarkdownDefaultName("../bad:name")).toBe("bad-name.md");
    expect(safeMarkdownDefaultName("note.MD")).toBe("note.MD");
  });
});

describe("safeVaultOutputPath", () => {
  test("allows nested relative paths under the selected vault", () => {
    const root = tempDir();
    expect(safeVaultOutputPath(root, "notes/today.md")).toBe(path.join(root, "notes", "today.md"));
  });

  test("rejects paths that escape the selected vault", () => {
    const root = tempDir();
    expect(() => safeVaultOutputPath(root, "../outside.md")).toThrow();
    expect(() => safeVaultOutputPath(root, "C:/outside.md")).toThrow();
    expect(() => safeVaultOutputPath(root, "/outside.md")).toThrow();
  });

  test("rejects existing links in an export path", () => {
    const root = tempDir();
    const outside = tempDir();
    try {
      symlinkSync(outside, path.join(root, "linked"), "junction");
    } catch {
      // Some Windows test environments cannot create links without privileges.
      return;
    }
    expect(() => safeVaultOutputPath(root, "linked/escape.md")).toThrow("contains a link");
  });
});

describe("resolveMarkdownVaultSourcePath", () => {
  test("accepts absolute paths and rejects empty or relative paths", () => {
    const absolutePath = path.resolve(tempDir(), "image.png");
    expect(resolveMarkdownVaultSourcePath(absolutePath)).toBe(absolutePath);
    expect(resolveMarkdownVaultSourcePath("")).toBeNull();
    expect(resolveMarkdownVaultSourcePath("image.png")).toBeNull();
  });
});

describe("scanMarkdownVault", () => {
  test("loads markdown files and skips ignored directories", () => {
    const root = tempDir();
    mkdirSync(path.join(root, ".git"), { recursive: true });
    mkdirSync(path.join(root, "notes"), { recursive: true });
    writeFileSync(path.join(root, "notes", "a.md"), "# A", "utf8");
    writeFileSync(path.join(root, ".git", "ignored.md"), "# ignored", "utf8");

    const result = scanMarkdownVault(root);

    expect(result.files.map((file) => file.relativePath)).toEqual(["notes/a.md"]);
    expect(result.files[0]?.content).toBe("# A");
  });

  test("skips oversized vault images", () => {
    const root = tempDir();
    writeFileSync(path.join(root, "huge.png"), Buffer.alloc(10 * 1024 * 1024 + 1));
    expect(scanMarkdownVault(root).images).toEqual([]);
  });
});

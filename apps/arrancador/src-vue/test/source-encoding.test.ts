import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const rootDir = path.resolve(__dirname, "..", "..");
const ignoredDirectories = new Set([
  "node_modules",
  "out",
  "dist",
  "release",
]);
const scannedExtensions = new Set([
  ".ts",
  ".vue",
  ".md",
  ".json",
  ".mjs",
  ".html",
  ".css",
  ".yml",
  ".jsonc",
  ".ps1",
  ".bat",
]);

const mojibakeMarkers = [
  "\u0420\u045c",
  "\u0420\u00b0",
  "\u0420\u00b5",
  "\u0420\u0451",
  "\u0420\u0455",
  "\u0421\u0453",
  "\u0421\u201a",
  "\u0421\u040a",
  "\u0421\u2039",
  "\u0432\u0402",
  "\u0432\u045a",
  "\u0432\u045c",
  "\u0432\u201d",
  "\u00c3",
  "\u00c2",
];

function listSourceFiles(directory: string): string[] {
  const files: string[] = [];

  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (ignoredDirectories.has(entry.name)) {
      continue;
    }

    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...listSourceFiles(entryPath));
      continue;
    }

    if (scannedExtensions.has(path.extname(entry.name).toLowerCase())) {
      files.push(entryPath);
    }
  }

  return files;
}

describe("source encoding", () => {
  it("does not contain common mojibake marker sequences", () => {
    const offenders = listSourceFiles(rootDir).flatMap((filePath) => {
      const text = fs.readFileSync(filePath, "utf8");
      const markers = mojibakeMarkers.filter((marker) => text.includes(marker));
      return markers.length > 0
        ? [`${path.relative(rootDir, filePath)}: ${markers.join(", ")}`]
        : [];
    });

    expect(offenders).toEqual([]);
  });
});

import { copyFileSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  MARKDOWN_FILE_MAX_BYTES,
  resolveMarkdownVaultSourcePath,
  safeVaultOutputPath,
  scanMarkdownVault,
  type MarkdownVaultExportFile,
} from "./markdown-vault";

const MAX_VAULT_FILES = 5_000;
const MAX_VAULT_IMAGES = 5_000;

export type MarkdownFileOperation = "open" | "openVault" | "save" | "exportVault";
export type MarkdownFileDialogs = {
  openFile(): Promise<string | null>;
  openDirectory(mode: "import" | "export"): Promise<string | null>;
  saveFile(suggestedName: unknown): Promise<string | null>;
};
export type MarkdownAssetCopy = (
  sourcePath: string,
  outputPath: string,
) => boolean | Promise<boolean>;

const defaultAssetCopy: MarkdownAssetCopy = (sourcePathValue, outputPath) => {
  const sourcePath = resolveMarkdownVaultSourcePath(sourcePathValue);
  if (!sourcePath) return false;
  try {
    if (!statSync(sourcePath).isFile()) return false;
    copyFileSync(sourcePath, outputPath);
    return true;
  } catch {
    return false;
  }
};

function requireFile(filePath: string): void {
  if (!statSync(filePath).isFile()) throw new Error("Markdown operation expects a file");
}

function requireDirectory(directoryPath: string): void {
  if (!statSync(directoryPath).isDirectory())
    throw new Error("Markdown operation expects a directory");
}

export async function performMarkdownFileOperation(
  operation: MarkdownFileOperation,
  input: Record<string, unknown>,
  dialogs: MarkdownFileDialogs,
  copyAsset: MarkdownAssetCopy = defaultAssetCopy,
): Promise<unknown | null> {
  if (operation === "open") {
    const filePath = await dialogs.openFile();
    if (!filePath) return null;
    requireFile(filePath);
    const size = statSync(filePath).size;
    if (size > MARKDOWN_FILE_MAX_BYTES) throw new Error("Markdown file is too large");
    return {
      path: filePath,
      name: path.basename(filePath),
      content: readFileSync(filePath, "utf8"),
    };
  }

  if (operation === "openVault") {
    const rootPath = await dialogs.openDirectory("import");
    if (!rootPath) return null;
    requireDirectory(rootPath);
    return scanMarkdownVault(rootPath);
  }

  if (operation === "save") {
    const content = input.content;
    if (typeof content !== "string" || Buffer.byteLength(content) > MARKDOWN_FILE_MAX_BYTES)
      throw new Error("Invalid Markdown content");
    const filePath = await dialogs.saveFile(input.suggestedName);
    if (!filePath) return null;
    writeFileSync(filePath, content, "utf8");
    return { path: filePath };
  }

  const files = input.files;
  if (!Array.isArray(files) || files.length > MAX_VAULT_FILES + MAX_VAULT_IMAGES)
    throw new Error("Invalid Markdown vault payload");
  const outputDir = await dialogs.openDirectory("export");
  if (!outputDir) return null;
  requireDirectory(outputDir);
  let textCount = 0;
  let imageCount = 0;
  const writtenPaths = new Set<string>();
  const planned: Array<{ file: MarkdownVaultExportFile; outputPath: string }> = [];
  for (const file of files as MarkdownVaultExportFile[]) {
    if (!file || typeof file.relativePath !== "string")
      throw new Error("Invalid Markdown vault file");
    const outputPath = safeVaultOutputPath(outputDir, file.relativePath);
    const outputKey = process.platform === "win32" ? outputPath.toLowerCase() : outputPath;
    if (writtenPaths.has(outputKey)) throw new Error("Duplicate Markdown vault path");
    writtenPaths.add(outputKey);
    if (typeof file.content === "string") {
      textCount += 1;
      if (textCount > MAX_VAULT_FILES || Buffer.byteLength(file.content) > MARKDOWN_FILE_MAX_BYTES)
        throw new Error("Markdown vault text limit exceeded");
    } else if (typeof file.sourcePath === "string") {
      imageCount += 1;
      if (imageCount > MAX_VAULT_IMAGES) throw new Error("Markdown vault image limit exceeded");
    } else {
      throw new Error("Invalid Markdown vault file");
    }
    planned.push({ file, outputPath });
  }
  let exportedCount = 0;
  for (const { file, outputPath } of planned) {
    mkdirSync(path.dirname(outputPath), { recursive: true });
    if (typeof file.content === "string") {
      writeFileSync(outputPath, file.content, "utf8");
      exportedCount += 1;
    } else if (await copyAsset(file.sourcePath, outputPath)) {
      exportedCount += 1;
    }
  }
  return { outputDir, exportedCount };
}

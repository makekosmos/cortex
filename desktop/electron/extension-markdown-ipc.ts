import {
  BrowserWindow,
  dialog,
  ipcMain,
  type OpenDialogOptions,
  type SaveDialogOptions,
  type WebContents,
} from "electron";
import { copyFileSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  MARKDOWN_FILE_MAX_BYTES,
  resolveMarkdownVaultSourcePath,
  safeMarkdownDefaultName,
  safeVaultOutputPath,
  scanMarkdownVault,
  type MarkdownVaultExportFile,
  type MarkdownVaultOpenResult,
} from "./extension-markdown-vault";
import { isString } from "../src/shared/runtimeGuards";

type MarkdownSuggestedName = string | null | undefined;
type MarkdownContentInput = string | null | undefined;
type MarkdownFilesInput = MarkdownVaultExportFile[] | null | undefined;

type MarkdownCapability = "markdownFiles.open" | "markdownFiles.save";

interface ExtensionMarkdownIpcOptions {
  assertHostPermission(sender: WebContents, capability: MarkdownCapability): void;
}

function markdownDialogParent(sender: WebContents): BrowserWindow | undefined {
  return BrowserWindow.fromWebContents(sender) ?? undefined;
}

function copyMarkdownVaultAsset(sourcePathValue: string, outputPath: string): boolean {
  const sourcePath = resolveMarkdownVaultSourcePath(sourcePathValue);
  if (!sourcePath) {
    console.warn(
      "[kepler-shell] Markdown vault export asset skipped: unsafe or unsupported source path",
      sourcePathValue,
    );
    return false;
  }

  try {
    const stat = statSync(sourcePath);
    if (!stat.isFile()) {
      console.warn(
        "[kepler-shell] Markdown vault export asset skipped: source is not a file",
        sourcePath,
      );
      return false;
    }

    copyFileSync(sourcePath, outputPath);
    return true;
  } catch (error) {
    console.warn(
      "[kepler-shell] Markdown vault export asset skipped: copy failed",
      sourcePath,
      error,
    );
    return false;
  }
}

export function registerExtensionMarkdownIpc({
  assertHostPermission,
}: ExtensionMarkdownIpcOptions): void {
  ipcMain.handle(
    "kepler:extension:markdownFiles:open",
    async (e): Promise<{ path: string; name: string; content: string } | null> => {
      assertHostPermission(e.sender, "markdownFiles.open");
      const parent = markdownDialogParent(e.sender);
      const options: OpenDialogOptions = {
        title: "Импорт Markdown",
        properties: ["openFile"],
        filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
      };
      const result = parent
        ? await dialog.showOpenDialog(parent, options)
        : await dialog.showOpenDialog(options);
      if (result.canceled || result.filePaths.length === 0) return null;

      const filePath = result.filePaths[0];
      const stat = statSync(filePath);
      if (!stat.isFile()) {
        throw new Error("[kepler-shell] Markdown import expects a file");
      }
      if (stat.size > MARKDOWN_FILE_MAX_BYTES) {
        throw new Error("[kepler-shell] Markdown file is too large");
      }

      return {
        path: filePath,
        name: path.basename(filePath),
        content: readFileSync(filePath, "utf8"),
      };
    },
  );

  ipcMain.handle(
    "kepler:extension:markdownFiles:openVault",
    async (e): Promise<MarkdownVaultOpenResult | null> => {
      assertHostPermission(e.sender, "markdownFiles.open");
      const parent = markdownDialogParent(e.sender);
      const options: OpenDialogOptions = {
        title: "Импорт Obsidian vault",
        properties: ["openDirectory"],
      };
      const result = parent
        ? await dialog.showOpenDialog(parent, options)
        : await dialog.showOpenDialog(options);
      if (result.canceled || result.filePaths.length === 0) return null;

      const rootPath = result.filePaths[0];
      const stat = statSync(rootPath);
      if (!stat.isDirectory()) {
        throw new Error("[kepler-shell] Markdown vault import expects a directory");
      }

      return scanMarkdownVault(rootPath);
    },
  );

  ipcMain.handle(
    "kepler:extension:markdownFiles:save",
    async (
      e,
      suggestedName: MarkdownSuggestedName,
      content: MarkdownContentInput,
    ): Promise<{ path: string } | null> => {
      assertHostPermission(e.sender, "markdownFiles.save");
      if (!isString(content)) {
        throw new Error("[kepler-shell] Markdown export content must be a string");
      }
      const parent = markdownDialogParent(e.sender);
      const options: SaveDialogOptions = {
        title: "Экспорт Markdown",
        defaultPath: safeMarkdownDefaultName(suggestedName ?? ""),
        filters: [{ name: "Markdown", extensions: ["md"] }],
      };
      const result = parent
        ? await dialog.showSaveDialog(parent, options)
        : await dialog.showSaveDialog(options);
      if (result.canceled || !result.filePath) return null;

      writeFileSync(result.filePath, content, "utf8");
      return { path: result.filePath };
    },
  );

  ipcMain.handle(
    "kepler:extension:markdownFiles:exportVault",
    async (e, files: MarkdownFilesInput): Promise<{ outputDir: string; exportedCount: number } | null> => {
      assertHostPermission(e.sender, "markdownFiles.save");
      if (!Array.isArray(files)) {
        throw new Error("[kepler-shell] Markdown vault export files must be an array");
      }
      const parent = markdownDialogParent(e.sender);
      const options: OpenDialogOptions = {
        title: "Экспорт Eden в Obsidian vault",
        properties: ["openDirectory", "createDirectory"],
      };
      const result = parent
        ? await dialog.showOpenDialog(parent, options)
        : await dialog.showOpenDialog(options);
      if (result.canceled || result.filePaths.length === 0) return null;

      const outputDir = result.filePaths[0];
      const stat = statSync(outputDir);
      if (!stat.isDirectory()) {
        throw new Error("[kepler-shell] Markdown vault export expects a directory");
      }

      let exportedCount = 0;
      let skippedAssetCount = 0;
      const writtenPaths = new Set<string>();
      // SAFETY: the IPC payload is checked as an array before iterating its export entries.
      for (const file of files as MarkdownVaultExportFile[]) {
        if (!file || !isString(file.relativePath)) {
          continue;
        }

        const outputPath = safeVaultOutputPath(outputDir, file.relativePath);
        if (writtenPaths.has(outputPath)) {
          throw new Error("[kepler-shell] Markdown vault export path already exists in payload");
        }
        writtenPaths.add(outputPath);
        mkdirSync(path.dirname(outputPath), { recursive: true });

        if (isString(file.content)) {
          writeFileSync(outputPath, file.content, "utf8");
          exportedCount += 1;
          continue;
        }

        if (isString(file.sourcePath)) {
          if (copyMarkdownVaultAsset(file.sourcePath, outputPath)) {
            exportedCount += 1;
          } else {
            skippedAssetCount += 1;
          }
        }
      }

      if (skippedAssetCount > 0) {
        console.warn(
          `[kepler-shell] Markdown vault export completed with ${skippedAssetCount} skipped asset(s)`,
        );
      }

      return { outputDir, exportedCount };
    },
  );
}

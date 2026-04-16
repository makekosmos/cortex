import { stat } from "node:fs/promises";
import path from "node:path";

export function isHiddenSegment(segment: string) {
  return segment.startsWith(".");
}

export function isExecutableFile(fileName: string) {
  return path.extname(fileName).toLowerCase() === ".exe";
}

export function normalizeScanRoot(root: string) {
  const trimmed = root.trim();
  if (!trimmed) {
    throw new Error("Empty scan directory");
  }
  return path.resolve(trimmed);
}

export function createScanAbortError() {
  const error = new Error("Scan cancelled");
  error.name = "AbortError";
  return error;
}

export async function isReadableDirectory(target: string) {
  try {
    return (await stat(target)).isDirectory();
  } catch {
    return false;
  }
}

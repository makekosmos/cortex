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
const scannedExtensions = new Set([".ts", ".vue", ".mjs", ".js", ".tsx", ".jsx"]);
const deprecatedSourceExtensions = new Set([".tsx", ".jsx"]);
const deprecatedPackages = new Set([
  "react",
  "react-dom",
  "@vitejs/plugin-react",
  "@tauri-apps/api",
  "@tauri-apps/cli",
]);

function listFiles(
  directory: string,
  localIgnoredDirectories: ReadonlySet<string> = ignoredDirectories,
): string[] {
  const files: string[] = [];

  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    if (localIgnoredDirectories.has(entry.name)) {
      continue;
    }

    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...listFiles(entryPath, localIgnoredDirectories));
      continue;
    }

    if (scannedExtensions.has(path.extname(entry.name).toLowerCase())) {
      files.push(entryPath);
    }
  }

  return files;
}

function relative(filePath: string) {
  return path.relative(rootDir, filePath).replace(/\\/g, "/");
}

function collectPatternOffenders(
  files: readonly string[],
  patterns: readonly RegExp[],
) {
  return files.flatMap((filePath) => {
    const text = fs.readFileSync(filePath, "utf8");
    const matched = patterns.filter((pattern) => pattern.test(text));
    return matched.length > 0 ? [relative(filePath)] : [];
  });
}

function readAppFile(filePath: string) {
  return fs.readFileSync(path.join(rootDir, filePath), "utf8");
}

function countMeaningfulLines(source: string) {
  return source
    .split(/\r?\n/)
    .filter((line) => line.trim().length > 0).length;
}

function extractVueBlock(source: string, blockName: "script" | "template") {
  const match = source.match(
    new RegExp(`<${blockName}[^>]*>([\\s\\S]*?)</${blockName}>`),
  );

  return match?.[1] ?? "";
}

describe("architecture boundaries", () => {
  it("keeps removed runtime trees and source extensions out of the app", () => {
    expect(fs.existsSync(path.join(rootDir, "src-tauri"))).toBe(false);

    const files = listFiles(rootDir, ignoredDirectories);
    const deprecatedSourceFiles = files
      .filter((filePath) =>
        deprecatedSourceExtensions.has(path.extname(filePath).toLowerCase()),
      )
      .map(relative);

    expect(deprecatedSourceFiles).toEqual([]);
  });

  it("keeps deprecated framework packages out of package manifests", () => {
    const manifest = JSON.parse(
      fs.readFileSync(path.join(rootDir, "package.json"), "utf8"),
    ) as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
      scripts?: Record<string, string>;
    };
    const declaredPackages = new Set([
      ...Object.keys(manifest.dependencies ?? {}),
      ...Object.keys(manifest.devDependencies ?? {}),
    ]);
    const forbiddenPackages = [...deprecatedPackages].filter((packageName) =>
      declaredPackages.has(packageName),
    );
    const forbiddenScripts = Object.entries(manifest.scripts ?? {})
      .filter(([name, command]) => /tauri|react/i.test(`${name} ${command}`))
      .map(([name]) => name);

    expect(forbiddenPackages).toEqual([]);
    expect(forbiddenScripts).toEqual([]);
  });

  it("keeps project configuration aligned with Vue and Electron only", () => {
    const configFiles = [
      "biome.jsonc",
      "tailwind.config.ts",
      "tsconfig.json",
      "vite.config.mjs",
      "vitest.config.mjs",
      "src-vue/test/vitest.config.mjs",
    ];
    const offenders = collectPatternOffenders(
      configFiles.map((filePath) => path.join(rootDir, filePath)),
      [/\btsx\b/i, /\bjsx\b/i, /src-tauri/i, /@vitejs\/plugin-react/i],
    );

    expect(offenders).toEqual([]);
  });

  it("keeps Vue renderer code behind the preload/API boundary", () => {
    const rendererFiles = listFiles(
      path.join(rootDir, "src-vue"),
      new Set([...ignoredDirectories, "test"]),
    );
    const offenders = collectPatternOffenders(rendererFiles, [
      /from\s+["']electron["']/,
      /from\s+["']node:/,
      /from\s+["'][^"']*electron\/main/,
      /@tauri-apps\/api/,
      /\b__TAURI__\b/,
    ]);

    expect(offenders).toEqual([]);
  });

  it("keeps Electron IPC registration split into feature modules", () => {
    const ipcDir = path.join(rootDir, "electron", "main", "ipc");

    expect(fs.existsSync(path.join(ipcDir, "app-handlers.ts"))).toBe(true);
    expect(fs.existsSync(path.join(ipcDir, "backup-handlers.ts"))).toBe(true);
    expect(fs.existsSync(path.join(ipcDir, "game-handlers.ts"))).toBe(true);
    expect(fs.existsSync(path.join(ipcDir, "shell-scan-handlers.ts"))).toBe(true);

    const backend = fs.readFileSync(
      path.join(rootDir, "electron", "main", "backend.ts"),
      "utf8",
    );
    const localHandleCount = backend.match(/ipcMain\.handle\(/g)?.length ?? 0;

    expect(localHandleCount).toBe(0);
  });

  it("keeps backend lifecycle separate from runtime service composition", () => {
    const backend = fs.readFileSync(
      path.join(rootDir, "electron", "main", "backend.ts"),
      "utf8",
    );
    const runtimeServices = fs.readFileSync(
      path.join(rootDir, "electron", "main", "runtime-services.ts"),
      "utf8",
    );

    expect(backend).toContain('from "./runtime-services"');
    expect(runtimeServices).toContain("export function createRuntimeServices");

    const forbiddenBackendPatterns = [
      /createGamesService/,
      /createSettingsService/,
      /createStatsService/,
      /createMetadataService/,
      /createCatalogueService/,
      /createNotificationsService/,
      /createAchievementsService/,
      /createSystemService/,
    ];
    const backendOffenders = forbiddenBackendPatterns.filter((pattern) =>
      pattern.test(backend),
    );

    expect(backendOffenders).toEqual([]);
  });

  it("keeps high-traffic IPC modules readable", () => {
    const files = [
      "app-handlers.ts",
      "game-handlers.ts",
      "shell-scan-handlers.ts",
    ].map((filePath) => path.join(rootDir, "electron", "main", "ipc", filePath));
    const longLines = files.flatMap((filePath) =>
      fs
        .readFileSync(filePath, "utf8")
        .split(/\r?\n/)
        .flatMap((line, index) =>
          line.length > 120 ? [`${relative(filePath)}:${index + 1}`] : [],
        ),
    );

    expect(longLines).toEqual([]);
  });

  it("keeps ScanPage as a route-level composition surface", () => {
    const source = readAppFile("src-vue/pages/ScanPage.vue");
    const script = extractVueBlock(source, "script");
    const totalMeaningfulLines = countMeaningfulLines(source);
    const scriptMeaningfulLines = countMeaningfulLines(script);
    const forbiddenOrchestrationPatterns = [
      {
        label: "renderer API import",
        pattern: /from\s+["']\.\.\/\.\.\/src\/lib\/api["']/,
      },
      {
        label: "direct IPC import",
        pattern: /from\s+["']\.\.\/\.\.\/src\/lib\/ipc["']/,
      },
      { label: "scanApi calls", pattern: /\bscanApi\./ },
      {
        label: "direct game import API calls",
        pattern: /\bgamesApi\.(?:existsByPath|resolveShortcutTarget|add|addBatch)\b/,
      },
      {
        label: "scan IPC channels",
        pattern: /\binvoke\(\s*["'](?:scan_executables_stream|cancel_scan)["']/,
      },
      {
        label: "scan event subscriptions",
        pattern: /\bsubscribeAppEvent<[^>]+>\(\s*["']scan:/,
      },
    ];
    const orchestrationOffenders = forbiddenOrchestrationPatterns
      .filter(({ pattern }) => pattern.test(script))
      .map(({ label }) => label);
    const boundaryProblems = [
      ...(scriptMeaningfulLines > 220
        ? [
            `ScanPage <script setup> has ${scriptMeaningfulLines} meaningful lines; keep scan/import side effects in focused composables or helpers.`,
          ]
        : []),
      ...(totalMeaningfulLines > 420
        ? [
            `ScanPage has ${totalMeaningfulLines} meaningful lines; keep it below the route-level size budget.`,
          ]
        : []),
      ...orchestrationOffenders.map(
        (label) => `ScanPage still owns low-level orchestration: ${label}.`,
      ),
    ];

    expect(boundaryProblems).toEqual([]);
  });

  it("keeps deprecated runtimes and test APIs out of active source", () => {
    const activeFiles = listFiles(
      rootDir,
      new Set([...ignoredDirectories, "test"]),
    );
    const offenders = collectPatternOffenders(activeFiles, [
      /from\s+["']react["']/,
      /from\s+["']react-dom/,
      /from\s+["']bun:test["']/,
      /@tauri-apps\/api/,
      /\b__TAURI__\b/,
    ]);

    expect(offenders).toEqual([]);
  });
});

import { BrowserWindow } from "electron";
import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import {
  checkApiCompat,
  extensionUserDataDir,
  isHeadlessOrTest,
  isShellInDevSession,
  resolveExtensionDir,
  type ExtensionManifest,
} from "./extension-manifest";
import { keplerDataDir } from "./data-dir";
import { assertLegacyLaunchAllowed } from "./legacy-migration-journal";

interface NativeExtensionEntry {
  child: ChildProcess;
  id: string;
}

const nativeExtensions = new Map<string, NativeExtensionEntry>();

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function openIncompatibilityWindow(manifest: ExtensionManifest, reason: string): void {
  const html = `<!doctype html>
<html lang="ru">
<head>
  <meta charset="utf-8" />
  <title>Расширение несовместимо — ${escapeHtml(manifest.name)}</title>
  <style>
    :root { color-scheme: dark; }
    html,body { margin:0; padding:0; height:100%; background:#1a1a1a; color:#e6e6e6;
      font: 13px/1.5 system-ui, -apple-system, Segoe UI, sans-serif; }
    .wrap { padding: 28px 32px; max-width: 520px; margin: 0 auto; }
    h1 { font-size: 16px; font-weight: 600; margin: 0 0 12px; color: #ff9b8a; }
    p { margin: 0 0 12px; color: #cfcfcf; }
    code { background: #2a2a2a; padding: 1px 6px; border-radius: 4px; font-size: 12px; }
    button { margin-top: 14px; background:#2d2d2d; border:1px solid #3d3d3d; color:#e6e6e6;
      padding: 6px 14px; border-radius: 6px; font: inherit; }
    button:hover { background:#383838; }
  </style>
</head>
<body>
  <div class="wrap">
    <h1>Расширение несовместимо</h1>
    <p>${escapeHtml(reason)}</p>
    <p><strong>ID:</strong> <code>${escapeHtml(manifest.id)}</code></p>
    <p><strong>Версия расширения:</strong> <code>${escapeHtml(manifest.version ?? "не указана")}</code></p>
    <button onclick="window.close()">Закрыть</button>
  </div>
</body>
</html>`;
  const win = new BrowserWindow({
    width: 560,
    height: 320,
    title: `Расширение несовместимо — ${manifest.name}`,
    backgroundColor: "#1a1a1a",
    resizable: false,
    minimizable: false,
    maximizable: false,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  void win.loadURL(`data:text/html;charset=utf-8,${encodeURIComponent(html)}`);
}

function resolveNativeExecutable(manifest: ExtensionManifest, extensionDir: string): string | null {
  const native = manifest.native;
  if (!native?.executable) {
    console.warn(`[kepler-shell] native extension '${manifest.id}' has no native.executable`);
    return null;
  }
  const candidates = [native.devExecutable, native.executable].filter(
    (value): value is string => typeof value === "string" && value.length > 0,
  );
  for (const rel of candidates) {
    const resolved = path.resolve(extensionDir, rel);
    if (existsSync(resolved)) return resolved;
  }
  console.warn(
    `[kepler-shell] native extension '${manifest.id}' executable not found: ${candidates.join(", ")}`,
  );
  return null;
}

export function isNativeExtensionRunning(id: string): boolean {
  const native = nativeExtensions.get(id);
  return !!native && !native.child.killed && native.child.exitCode === null;
}

export async function stopNativeExtension(id: string): Promise<void> {
  const entry = nativeExtensions.get(id);
  if (!entry || entry.child.exitCode !== null || entry.child.killed) return;
  entry.child.kill();
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`native extension '${id}' did not stop`)),
      1000,
    );
    entry.child.once("exit", () => {
      clearTimeout(timer);
      resolve();
    });
  });
}

export async function openNativeExtension(
  id: string,
  manifest: ExtensionManifest,
  route: string | undefined,
): Promise<void> {
  assertLegacyLaunchAllowed(keplerDataDir(), id);
  const singleInstance = manifest.native?.singleInstance !== false;
  const existing = nativeExtensions.get(id);
  if (existing && existing.child.exitCode === null && !existing.child.killed) return;
  if (singleInstance && existing) nativeExtensions.delete(id);

  const incompat = checkApiCompat(manifest);
  if (incompat) {
    console.error(`[kepler-shell] ${incompat}`);
    openIncompatibilityWindow(manifest, incompat);
    return;
  }

  const extensionDir = resolveExtensionDir(id);
  if (!extensionDir) {
    console.warn(`[kepler-shell] extension dir disappeared: ${id}`);
    return;
  }
  const exe = resolveNativeExecutable(manifest, extensionDir);
  if (!exe) return;

  if (isHeadlessOrTest()) {
    console.log(`[kepler-shell] headless: skip native extension spawn '${id}'`);
    return;
  }

  const args = [
    ...(manifest.native?.args ?? []),
    "--kosmos-extension-id",
    id,
    "--kosmos-user-data-dir",
    extensionUserDataDir(id),
  ];
  const devSession = isShellInDevSession();
  if (devSession) args.push("--kosmos-dev-mode");
  if (route) args.push("--route", route);
  const child = spawn(exe, args, {
    cwd: path.dirname(exe),
    env: {
      ...process.env,
      KOSMOS_EXTENSION_DEV_MODE: devSession ? "1" : undefined,
    },
    stdio: "ignore",
    detached: false,
    windowsHide: false,
  });
  nativeExtensions.set(id, { child, id });
  child.once("exit", () => {
    const current = nativeExtensions.get(id);
    if (current?.child === child) nativeExtensions.delete(id);
  });
  child.once("error", (error) => {
    console.error(`[kepler-shell] native extension '${id}' failed:`, error);
    const current = nativeExtensions.get(id);
    if (current?.child === child) nativeExtensions.delete(id);
  });
}

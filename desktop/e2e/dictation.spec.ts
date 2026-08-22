// Phase 1 диктации: e2e гарды headless-контракта + базовый IPC контракт.
//
// Что проверяем:
//   AC10: в headless / test mode global hotkey диктации НЕ регистрируется
//         (иначе мешал бы клавиатуре машины разработчика во время CI).
//   AC10: pill window НЕ показывается визуально (можем создать, но isVisible=false).
//   AC1:  IPC `kepler:dictation:*` зарегистрированы (toggle / cancel / pill-finished).
//   AC4:  backend `dictation.get_config` возвращает дефолтный конфиг + hasApiKey=false
//         (изолированный userData → нет API key в Credential Manager этого профиля).
//
// Что НЕ проверяем e2e (требует UI session + микрофон + Groq API key):
//   AC5-9: реальный hotkey → запись → транскрипция → inject. Покрыто manual verify.
//   AC11-12: ARK guard + smoke — отдельные scripts.

import path from "node:path";
import fs from "node:fs";
import { setTimeout as timeoutAfter } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { test, expect } from "@playwright/test";
import electronBinary from "electron";
import { _electron as electron, type ElectronApplication, type Page } from "playwright";
import { waitForBackendReady } from "../../../tests/e2e/helpers/wait";
import {
  freshDataDir,
  launchKeplerWithDataDir,
  shutdownKeplerEngine,
} from "../../../tests/e2e/helpers/launch";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
type KeplerCommand = { id: string; shortcut?: string };

function makeSmokeWavBase64(): string {
  const sampleRate = 16000;
  const durationSec = 1.5;
  const samples = Math.floor(sampleRate * durationSec);
  const dataSize = samples * 2;
  const wav = Buffer.alloc(44 + dataSize);

  wav.write("RIFF", 0, "ascii");
  wav.writeUInt32LE(36 + dataSize, 4);
  wav.write("WAVEfmt ", 8, "ascii");
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(sampleRate, 24);
  wav.writeUInt32LE(sampleRate * 2, 28);
  wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34);
  wav.write("data", 36, "ascii");
  wav.writeUInt32LE(dataSize, 40);

  for (let i = 0; i < samples; i += 1) {
    const t = i / sampleRate;
    const envelope =
      (t < 0.12 ? 0 : 1) *
      (t < 0.42 ? (t - 0.12) / 0.3 : 1) *
      (t > 1.28 ? Math.max(0, (1.5 - t) / 0.22) : 1);
    const syllablePulse =
      (t >= 0.14 && t < 0.48 ? Math.sin(Math.PI * ((t - 0.14) / 0.34)) ** 2 : 0) +
      (t >= 0.62 && t < 1.0 ? Math.sin(Math.PI * ((t - 0.62) / 0.38)) ** 2 : 0) +
      (t >= 1.08 ? Math.sin(Math.PI * Math.min(1, (t - 1.08) / 0.28)) ** 2 : 0);
    const voiced = Math.sin(2 * Math.PI * 165 * t) * 0.52;
    const formant = Math.sin(2 * Math.PI * 245 * t + 0.3) * 0.28;
    const airy = Math.sin(2 * Math.PI * 420 * t + 1.2) * 0.12;
    const sample = Math.max(
      -1,
      Math.min(1, (voiced + formant + airy) * Math.max(0.35, syllablePulse) * envelope),
    );
    wav.writeInt16LE(Math.round(sample * 0x7fff), 44 + i * 2);
  }
  return wav.toString("base64");
}

function getRequiredEnv(name: string): string | null {
  const value = process.env[name]?.trim();
  return value ? value : null;
}

async function dictationRequest<T = unknown>(
  page: Page,
  operation: string,
  params: Record<string, unknown> = {},
): Promise<T> {
  return page.evaluate(
    async ({ operation, params }) => {
      const api = window as unknown as {
        kepler?: {
          ark?: {
            request?: <R = unknown>(
              op: string,
              requestParams?: Record<string, unknown>,
            ) => Promise<R>;
          };
        };
      };
      const req = api.kepler?.ark?.request;
      if (!req) throw new Error("window.kepler.ark.request is unavailable");
      return req(operation, params);
    },
    { operation, params },
  );
}

async function getDictationState(page: Page): Promise<{
  state?: string;
  hasApiKey?: boolean;
  lastError?: string | null;
  activeUuid?: string | null;
  attempts?: number;
  canRetry?: boolean;
  config?: { provider?: string; injectMode?: string };
}> {
  return dictationRequest(page, "dictation.get_state");
}

async function getDictationDiagnostics(page: Page): Promise<string> {
  const state = await getDictationState(page);
  const pending = await dictationRequest<{ items?: unknown[] }>(page, "dictation.list_pending");
  return JSON.stringify(
    {
      state: state.state ?? null,
      hasApiKey: state.hasApiKey ?? null,
      lastError: state.lastError ?? null,
      activeUuid: state.activeUuid ?? null,
      attempts: state.attempts ?? null,
      canRetry: state.canRetry ?? null,
      pendingCount: (pending.items ?? []).length,
    },
    null,
    2,
  );
}

async function setDictationHotkey(page: Page, hotkey: string): Promise<void> {
  await dictationRequest(page, "dictation.update_config", { hotkey });
}

async function listKeplerCommands(page: Page): Promise<KeplerCommand[]> {
  return page.evaluate(async () => {
    const api = window as unknown as {
      kepler?: {
        commands?: {
          list?: () => Promise<KeplerCommand[]>;
        };
      };
    };
    const list = api.kepler?.commands?.list;
    if (!list) throw new Error("window.kepler.commands.list is unavailable");
    return list();
  });
}

async function invokeKeplerCommand(page: Page, id: string): Promise<void> {
  await page.evaluate(async (commandId) => {
    const api = window as unknown as {
      kepler?: {
        commands?: {
          invoke?: (id: string) => Promise<void>;
        };
      };
    };
    const invoke = api.kepler?.commands?.invoke;
    if (!invoke) throw new Error("window.kepler.commands.invoke is unavailable");
    await invoke(commandId);
  }, id);
}

async function waitForDictationStateWithContext(
  page: Page,
  expectedState: string,
  timeoutMs: number,
): Promise<void> {
  try {
    await expect
      .poll(
        async () => {
          const state = await getDictationState(page);
          return state.state;
        },
        { timeout: timeoutMs },
      )
      .toBe(expectedState);
  } catch (error) {
    const wrapped = new Error(
      `Timed out after ${timeoutMs}ms waiting for dictation state ${expectedState}.\n` +
        (await getDictationDiagnostics(page)),
    );
    wrapped.cause = error;
    throw wrapped;
  }
}

async function submitAudioWithContext(
  page: Page,
  params: Record<string, unknown>,
  timeoutMs: number,
): Promise<{ state?: string; queued?: boolean; uuid?: string; error?: string }> {
  console.log(`[dictation-groq-smoke] submit_audio start (timeout ${timeoutMs}ms)`);
  const submission = dictationRequest<{
    state?: string;
    queued?: boolean;
    uuid?: string;
    error?: string;
  }>(page, "dictation.submit_audio", params);
  const timed = await Promise.race([
    submission.then((value) => ({ kind: "ok" as const, value })),
    timeoutAfter(timeoutMs).then(() => ({ kind: "timeout" as const })),
  ]);
  if (timed.kind === "timeout") {
    throw new Error(
      `dictation.submit_audio did not finish within ${timeoutMs}ms.\n` +
        (await getDictationDiagnostics(page)),
    );
  }
  console.log(`[dictation-groq-smoke] submit_audio done: ${JSON.stringify(timed.value)}`);
  return timed.value;
}

async function launchKepler(): Promise<ElectronApplication> {
  return launchKeplerWithDataDir(freshDataDir("dictation-command-headless"));
}

async function launchKeplerWithFakeMedia(
  dataDir: string,
  extraEnv: Record<string, string>,
): Promise<ElectronApplication> {
  const userDataDir = path.join(dataDir, "electron-userdata");
  fs.mkdirSync(userDataDir, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [
      "--use-fake-device-for-media-stream",
      "--use-fake-ui-for-media-stream",
      path.join(appRoot, "dist-electron", "main.js"),
      `--user-data-dir=${userDataDir}`,
    ],
    env: {
      ...process.env,
      NODE_ENV: "test",
      KEPLER_SKIP_SYNC: "1",
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KOSMOS_DATA_DIR: dataDir,
      KEPLER_USAGE_TRACKER: "0",
      ...extraEnv,
    },
    timeout: 20_000,
  });
}

test.describe("dictation Phase 1", () => {
  test.describe.configure({ timeout: 45_000 });

  test("AC10: globalShortcut для диктации НЕ зарегистрирован в headless mode", async () => {
    const app = await launchKepler();
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      const result = await app.evaluate(({ globalShortcut }) => {
        // Дефолтный accelerator из dictation::config — Ctrl+Shift+;
        return {
          isRegistered: globalShortcut.isRegistered("Ctrl+Shift+;"),
        };
      });
      expect(
        result.isRegistered,
        "В headless mode dictation globalShortcut должен оставаться unregistered (см. setupDictationHotkey).",
      ).toBe(false);
    } finally {
      await app.close();
    }
  });

  test("AC10: pill window не visible (создаётся lazily, но без show() в headless)", async () => {
    const app = await launchKepler();
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher, 45_000);

      // Триггерим toggle через IPC → main создаёт pill window если ещё нет.
      // В headless'е showInactive() пропускается (см. dictation-pill.ts isHeadless).
      // Тут мы НЕ вызываем toggle (он требует backend), а просто проверяем
      // что dictation pill window отсутствует или скрыт.
      const windows = await app.evaluate(({ BrowserWindow }) => {
        return BrowserWindow.getAllWindows().map((w) => ({
          title: w.getTitle(),
          isVisible: w.isVisible(),
        }));
      });
      const dictationWindows = windows.filter((w) => /dictation/i.test(w.title) || w.title === "");
      // pill ещё не создан (toggle не звали) — должен быть либо отсутствовать,
      // либо invisible.
      for (const w of dictationWindows) {
        expect(w.isVisible, "Любой dictation-like window не должен быть visible в headless").toBe(
          false,
        );
      }
    } finally {
      await app.close();
    }
  });

  test("AC1: IPC handlers kepler:dictation:* зарегистрированы", async () => {
    const app = await launchKepler();
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      // `ipcMain.handle(channel, ...)` пишет в приватный `_invokeHandlers`
      // Map, не в Node EventEmitter `_events` (там оседают только
      // `ipcMain.on()` listeners). Probe'ить нужно через `_invokeHandlers`.
      const resp = await app.evaluate(async ({ ipcMain }) => {
        const internal = ipcMain as unknown as {
          _invokeHandlers?: Map<string, unknown>;
        };
        const handlers = internal._invokeHandlers;
        const has = (ch: string): boolean => handlers?.has(ch) ?? false;
        return {
          hasToggle: has("kepler:dictation:toggle"),
          hasCancel: has("kepler:dictation:cancel"),
          hasPillFinished: has("kepler:dictation:pill-finished"),
          handlerCount: handlers?.size ?? 0,
        };
      });
      expect(resp.hasToggle, "kepler:dictation:toggle handler должен быть зарегистрирован").toBe(
        true,
      );
      expect(resp.hasCancel, "kepler:dictation:cancel handler должен быть зарегистрирован").toBe(
        true,
      );
      expect(
        resp.hasPillFinished,
        "kepler:dictation:pill-finished handler должен быть зарегистрирован",
      ).toBe(true);
    } finally {
      await app.close();
    }
  });

  test("AC2: kepler:dictation shortcut invokes Engine headlessly", async () => {
    const dataDir = freshDataDir("dictation-command-headless");
    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await setDictationHotkey(launcher, "Ctrl+Alt+D");

      await expect
        .poll(async () => {
          const list = await listKeplerCommands(launcher);
          return list.find((cmd) => cmd.id === "kepler:dictation")?.shortcut ?? null;
        })
        .toBe("Ctrl+Alt+D");

      const list = await listKeplerCommands(launcher);
      const dictation = list.find((cmd) => cmd.id === "kepler:dictation");
      expect(list.map((cmd) => cmd.id)).toContain("kepler:dictation");
      expect(dictation?.shortcut).toBe("Ctrl+Alt+D");

      await invokeKeplerCommand(launcher, "kepler:dictation");
      await waitForDictationStateWithContext(launcher, "recording", 10_000);
      await dictationRequest(launcher, "dictation.cancel");
      await waitForDictationStateWithContext(launcher, "idle", 10_000);

      const state = await app.evaluate(({ BrowserWindow, globalShortcut }) => ({
        visibleWindows: BrowserWindow.getAllWindows().filter((w) => w.isVisible()).length,
        hotkeyRegistered: globalShortcut.isRegistered("Ctrl+Shift+;"),
      }));

      expect(state.visibleWindows, "headless invoke must not show visible windows").toBe(0);
      expect(state.hotkeyRegistered, "headless invoke must not register dictation hotkey").toBe(
        false,
      );
    } finally {
      app.process().kill();
      shutdownKeplerEngine(dataDir);
    }
  });

  test("AC3: settings page shows live dictation hotkey in command row", async () => {
    const app = await launchKepler();
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await setDictationHotkey(launcher, "Ctrl+Alt+D");

      await invokeKeplerCommand(launcher, "settings:open");

      const settings = await app.waitForEvent("window", { timeout: 10_000 });
      await settings.waitForLoadState("domcontentloaded");
      const dictationTabButton = settings.getByRole("button", { name: "Диктация" });
      await expect(dictationTabButton).toBeVisible();
      await dictationTabButton.click();

      await expect(
        settings.getByText("Переключить диктовку", { exact: true }),
        "settings page must render the dictation command row",
      ).toBeVisible();
      await expect(
        settings.locator(".command-row-shortcut", { hasText: "Ctrl+Alt+D" }),
        "settings command row must show the dictation shortcut",
      ).toBeVisible();
    } finally {
      await app.close();
    }
  });
});

test.describe("dictation mock STT", () => {
  test("mock transcript path completes through host state machine in headless mode", async () => {
    const dataDir = freshDataDir("dictation-mock-stt-headless");
    const app = await launchKeplerWithDataDir(dataDir, {
      KOSMOS_TEST_DICTATION_TRANSCRIPT: "привет из теста",
    });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      const mainEnvTranscript = await app.evaluate(() => {
        return process.env.KOSMOS_TEST_DICTATION_TRANSCRIPT ?? null;
      });
      expect(mainEnvTranscript).toBe("привет из теста");

      await dictationRequest(launcher, "dictation.update_config", {
        provider: "mock",
        injectMode: "clipboard_only",
        providerEnabled: true,
        transcriptionPrompt: "привет из теста",
      });

      const configBefore = await getDictationState(launcher);
      expect(configBefore.config?.provider).toBe("mock");
      expect(configBefore.config?.injectMode).toBe("clipboard_only");
      expect(configBefore.hasApiKey).toBe(false);

      const startResp = await dictationRequest<{ state?: string }>(
        launcher,
        "dictation.start_recording",
      );
      expect(startResp.state).toBe("recording");
      await waitForDictationStateWithContext(launcher, "recording", 10_000);

      const submitResp = await dictationRequest<{
        uuid?: string;
        state?: string;
        queued?: boolean;
        error?: string;
      }>(launcher, "dictation.submit_audio", {
        audioB64: makeSmokeWavBase64(),
        durationSec: 1,
      });

      expect(submitResp.state).toBe("idle");
      expect(submitResp.queued).toBeUndefined();
      expect(submitResp.error).toBeUndefined();

      await waitForDictationStateWithContext(launcher, "idle", 10_000);

      const state = await getDictationState(launcher);
      expect(state.state).toBe("idle");
      expect(state.activeUuid ?? null).toBeNull();
      expect(state.lastError ?? null).toBeNull();
      expect(state.config?.provider).toBe("mock");
      expect(state.config?.injectMode).toBe("clipboard_only");

      const pending = await dictationRequest<{ items?: unknown[] }>(
        launcher,
        "dictation.list_pending",
      );
      expect(pending.items ?? []).toEqual([]);

      const stats = await dictationRequest<{
        totalSessions?: number;
        totalWords?: number;
        totalRecordSeconds?: number;
      }>(launcher, "dictation.get_stats");
      // `привет из теста` = 3 слова, значит mock transcript реально прошёл
      // через host path и попал в stats.
      expect(stats.totalSessions).toBe(1);
      expect(stats.totalWords).toBe(3);
      expect(stats.totalRecordSeconds).toBe(1);
    } finally {
      await app.close();
    }
  });

  test("groq provider still reports missing API key in isolated env", async () => {
    const dataDir = freshDataDir("dictation-groq-no-key");
    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await dictationRequest(launcher, "dictation.update_config", {
        provider: "groq",
        injectMode: "clipboard_only",
        providerEnabled: true,
      });

      const startResp = await dictationRequest<{ state?: string }>(
        launcher,
        "dictation.start_recording",
      );
      expect(startResp.state).toBe("recording");
      await waitForDictationStateWithContext(launcher, "recording", 10_000);

      const submitResp = await launcher.evaluate(async () => {
        try {
          const api = window as unknown as {
            kepler?: {
              ark?: {
                request?: (op: string, params?: Record<string, unknown>) => Promise<unknown>;
              };
            };
          };
          const req = api.kepler?.ark?.request;
          if (!req) throw new Error("window.kepler.ark.request is unavailable");
          await req("dictation.submit_audio", {
            audioB64: "ZmFrZS13YXYtYnl0ZXM=",
            durationSec: 1,
          });
          return { ok: true as const };
        } catch (error) {
          return { ok: false as const, error: String(error) };
        }
      });

      expect(submitResp.ok).toBe(false);
      expect(submitResp.error).toContain("API key");

      const state = await getDictationState(launcher);
      expect(state.state).toBe("error");
      expect(state.lastError ?? "").toContain("API key");

      const pending = await dictationRequest<{ items?: unknown[] }>(
        launcher,
        "dictation.list_pending",
      );
      expect((pending.items ?? []).length).toBe(1);
    } finally {
      await app.close();
    }
  });

  test("local provider path completes through host state machine in headless mode", async () => {
    const dataDir = freshDataDir("dictation-local-stt-headless");
    const app = await launchKeplerWithDataDir(dataDir, {
      KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT: "локальная диктовка",
    });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await dictationRequest(launcher, "dictation.update_config", {
        provider: "local",
        injectMode: "clipboard_only",
        providerEnabled: true,
        localEngine: "whisper.cpp",
        localModelId: "whisper-test",
        localModelPath: "C:/models/whisper-test.bin",
        localCommandPath: "C:/tools/whisper-cli.exe",
      });

      const configBefore = await getDictationState(launcher);
      expect(configBefore.config?.provider).toBe("local");
      expect(configBefore.config?.injectMode).toBe("clipboard_only");
      expect(configBefore.hasApiKey).toBe(false);

      const startResp = await dictationRequest<{ state?: string }>(
        launcher,
        "dictation.start_recording",
      );
      expect(startResp.state).toBe("recording");
      await waitForDictationStateWithContext(launcher, "recording", 10_000);

      const submitResp = await dictationRequest<{
        uuid?: string;
        state?: string;
        queued?: boolean;
        error?: string;
      }>(launcher, "dictation.submit_audio", {
        audioB64: makeSmokeWavBase64(),
        durationSec: 1,
      });

      expect(submitResp.state).toBe("idle");
      expect(submitResp.queued).toBeUndefined();
      expect(submitResp.error).toBeUndefined();

      const state = await getDictationState(launcher);
      expect(state.state).toBe("idle");
      expect(state.activeUuid ?? null).toBeNull();

      const pending = await dictationRequest<{ items?: unknown[] }>(
        launcher,
        "dictation.list_pending",
      );
      expect((pending.items ?? []).length).toBe(0);
    } finally {
      await app.close();
    }
  });

  test("local provider records through dictation pill with fake microphone", async () => {
    const dataDir = freshDataDir("dictation-local-pill-fake-media");
    const transcript = "voice from fake mic";
    const app = await launchKeplerWithFakeMedia(dataDir, {
      KOSMOS_TEST_DICTATION_PILL: "1",
      KOSMOS_TEST_LOCAL_DICTATION_TRANSCRIPT: transcript,
    });
    try {
      await app.evaluate(({ clipboard }) => clipboard.writeText(""));
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await dictationRequest(launcher, "dictation.update_config", {
        provider: "local",
        injectMode: "clipboard_only",
        providerEnabled: true,
        localEngine: "whisper.cpp",
        localModelId: "fake-media-local",
        localModelPath: "C:/models/fake-media-local.bin",
        localCommandPath: "C:/tools/whisper-cli.exe",
      });

      await invokeKeplerCommand(launcher, "kepler:dictation");
      const recordingStartedAt = Date.now();
      await waitForDictationStateWithContext(launcher, "recording", 10_000);
      await expect
        .poll(() => Date.now() - recordingStartedAt, { timeout: 2_000 })
        .toBeGreaterThanOrEqual(900);

      await invokeKeplerCommand(launcher, "kepler:dictation");

      await waitForDictationStateWithContext(launcher, "idle", 20_000);
      const clipboardText = await app.evaluate(({ clipboard }) => clipboard.readText());
      expect(clipboardText).toBe(transcript);

      const pending = await dictationRequest<{ items?: unknown[] }>(
        launcher,
        "dictation.list_pending",
      );
      expect((pending.items ?? []).length).toBe(0);

      const stats = await dictationRequest<{
        totalSessions?: number;
        totalWords?: number;
        totalRecordSeconds?: number;
      }>(launcher, "dictation.get_stats");
      expect(stats.totalSessions).toBe(1);
      expect(stats.totalWords).toBe(4);
      expect(stats.totalRecordSeconds ?? 0).toBeGreaterThan(0);
    } finally {
      await app.close();
    }
  });
});

test.describe("AI settings", () => {
  test.describe.configure({ timeout: 60_000 });

  test("advanced AI page shows local dictation model settings", async () => {
    const dataDir = freshDataDir("dictation-ai-settings-page");
    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await dictationRequest(launcher, "dictation.update_config", {
        provider: "local",
        localEngine: "whisper.cpp",
        localModelId: "whisper-test",
        localModelPath: "C:/models/whisper-test.bin",
        localCommandPath: "C:/tools/whisper-cli.exe",
      });

      await invokeKeplerCommand(launcher, "settings:open");

      const settings = await app.waitForEvent("window", { timeout: 10_000 });
      await settings.waitForLoadState("domcontentloaded");
      const aiTabButton = settings.getByRole("button", { name: "AI" });
      await expect(aiTabButton).toBeVisible();
      await aiTabButton.click();

      await expect(settings.getByText("Groq", { exact: true })).toBeVisible();
      await expect(settings.getByRole("heading", { name: "Локально" })).toBeVisible();
      await expect(settings.getByText("Папка моделей", { exact: true })).toBeVisible();
      await expect(settings.getByText("Whisper Small", { exact: true })).toBeVisible();
      await expect(settings.getByText("Whisper Large v3 Turbo", { exact: true })).toBeVisible();
      await expect(settings.getByText("Путь к модели", { exact: true })).toBeVisible();
      await expect(settings.getByText("Путь к whisper.cpp", { exact: true })).toBeVisible();
      const inputValues = await settings
        .locator("input")
        .evaluateAll((inputs) => inputs.map((input) => (input as HTMLInputElement).value));
      expect(inputValues).toContain("C:/models/whisper-test.bin");
      expect(inputValues).toContain("C:/tools/whisper-cli.exe");
    } finally {
      await app.close();
    }
  });
});

test.describe("dictation Groq smoke", () => {
  test.describe.configure({ timeout: 90_000 });

  test("opt-in real provider path works headless without microphone", async () => {
    const apiKey = getRequiredEnv("KOSMOS_TEST_GROQ_API_KEY");
    const audioB64 = getRequiredEnv("KOSMOS_TEST_DICTATION_AUDIO_B64") ?? makeSmokeWavBase64();
    const expectedSubstring =
      process.env.KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING?.trim() || null;
    test.skip(!apiKey, "Set KOSMOS_TEST_GROQ_API_KEY to run the real Groq dictation smoke.");

    const dataDir = freshDataDir("dictation-groq-opt-in-smoke");
    const app = await launchKeplerWithDataDir(dataDir, {
      KOSMOS_TEST_GROQ_API_KEY: apiKey!,
    });
    let originalClipboardText = "";
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);
      console.log("[dictation-groq-smoke] backend ready");

      originalClipboardText = await app.evaluate(({ clipboard }) => clipboard.readText());
      console.log("[dictation-groq-smoke] clipboard snapshot captured");

      await dictationRequest(launcher, "dictation.update_config", {
        provider: "groq",
        injectMode: "clipboard_only",
        providerEnabled: true,
      });
      console.log("[dictation-groq-smoke] config updated for groq");

      const configBefore = await getDictationState(launcher);
      expect(configBefore.config?.provider).toBe("groq");
      expect(configBefore.config?.injectMode).toBe("clipboard_only");
      expect(configBefore.hasApiKey).toBe(true);

      const startResp = await dictationRequest<{ state?: string }>(
        launcher,
        "dictation.start_recording",
      );
      expect(startResp.state).toBe("recording");
      await waitForDictationStateWithContext(launcher, "recording", 10_000);
      console.log("[dictation-groq-smoke] recording started");

      const submitResp = await submitAudioWithContext(
        launcher,
        {
          audioB64,
          durationSec: 1,
        },
        20_000,
      );

      expect(
        submitResp.queued,
        "real Groq smoke should complete inline instead of leaving pending work",
      ).not.toBe(true);
      expect(submitResp.state).toBe("idle");

      await waitForDictationStateWithContext(launcher, "idle", 20_000);
      console.log("[dictation-groq-smoke] returned to idle");

      const state = await getDictationState(launcher);
      expect(state.state).toBe("idle");
      expect(state.activeUuid ?? null).toBeNull();
      expect(state.lastError ?? null).toBeNull();
      expect(state.config?.provider).toBe("groq");
      expect(state.config?.injectMode).toBe("clipboard_only");

      const pending = await dictationRequest<{ items?: unknown[] }>(
        launcher,
        "dictation.list_pending",
      );
      expect(pending.items ?? []).toEqual([]);

      const stats = await dictationRequest<{
        totalSessions?: number;
        totalWords?: number;
        totalRecordSeconds?: number;
      }>(launcher, "dictation.get_stats");
      expect(stats.totalSessions).toBe(1);
      expect(stats.totalWords ?? 0).toBeGreaterThan(0);
      expect(stats.totalRecordSeconds ?? 0).toBeGreaterThan(0);

      const clipboardText = await app.evaluate(({ clipboard }) => clipboard.readText());
      expect(clipboardText.trim().length).toBeGreaterThan(0);
      if (expectedSubstring) {
        expect(clipboardText).toContain(expectedSubstring);
      }
      console.log("[dictation-groq-smoke] clipboard updated");
    } finally {
      await app.evaluate(({ clipboard }, text) => {
        clipboard.writeText(text);
      }, originalClipboardText);
      await app.close();
    }
  });
});

test.describe("dictation local smoke", () => {
  test.describe.configure({ timeout: 120_000 });

  test("opt-in real local whisper.cpp provider works headless without microphone", async () => {
    const commandPath = getRequiredEnv("KOSMOS_TEST_LOCAL_DICTATION_COMMAND_PATH");
    const modelPath = getRequiredEnv("KOSMOS_TEST_LOCAL_DICTATION_MODEL_PATH");
    const audioPath = getRequiredEnv("KOSMOS_TEST_DICTATION_AUDIO_PATH");
    const audioB64 =
      getRequiredEnv("KOSMOS_TEST_DICTATION_AUDIO_B64") ??
      (audioPath ? fs.readFileSync(audioPath).toString("base64") : null);
    const expectedSubstring =
      process.env.KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING?.trim() || null;
    test.skip(
      !commandPath || !modelPath || !audioB64,
      "Set KOSMOS_TEST_LOCAL_DICTATION_COMMAND_PATH, KOSMOS_TEST_LOCAL_DICTATION_MODEL_PATH, and KOSMOS_TEST_DICTATION_AUDIO_PATH or KOSMOS_TEST_DICTATION_AUDIO_B64 to run local dictation smoke.",
    );

    const dataDir = freshDataDir("dictation-local-opt-in-smoke");
    const app = await launchKeplerWithDataDir(dataDir);
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await dictationRequest(launcher, "dictation.update_config", {
        provider: "local",
        injectMode: "clipboard_only",
        providerEnabled: true,
        language: "en",
        localEngine: "whisper.cpp",
        localModelId: "whisper-local-smoke",
        localModelPath: modelPath!,
        localCommandPath: commandPath!,
      });

      const configBefore = await getDictationState(launcher);
      expect(configBefore.config?.provider).toBe("local");
      expect(configBefore.config?.injectMode).toBe("clipboard_only");
      expect(configBefore.hasApiKey).toBe(false);

      const startResp = await dictationRequest<{ state?: string }>(
        launcher,
        "dictation.start_recording",
      );
      expect(startResp.state).toBe("recording");
      await waitForDictationStateWithContext(launcher, "recording", 10_000);

      const submitResp = await submitAudioWithContext(
        launcher,
        {
          audioB64: audioB64!,
          durationSec: 4,
        },
        120_000,
      );
      expect(submitResp.queued).not.toBe(true);
      expect(submitResp.state).toBe("idle");

      await waitForDictationStateWithContext(launcher, "idle", 20_000);
      const state = await getDictationState(launcher);
      expect(state.state).toBe("idle");
      expect(state.activeUuid ?? null).toBeNull();

      const pending = await dictationRequest<{ items?: unknown[] }>(
        launcher,
        "dictation.list_pending",
      );
      expect((pending.items ?? []).length).toBe(0);

      if (expectedSubstring) {
        const clipboardText = await app.evaluate(({ clipboard }) => clipboard.readText());
        expect(clipboardText.toLowerCase()).toContain(expectedSubstring.toLowerCase());
      }
    } finally {
      await app.close();
    }
  });
});

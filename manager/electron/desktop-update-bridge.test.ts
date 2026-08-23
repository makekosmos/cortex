import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { describe, expect, test } from "bun:test";
import { resolveDesktopUpdateBridge } from "./desktop-update-bridge";

const instance = {
  slot: "prod",
  kind: "prod",
  userDataDir: "",
  dataDir: "",
  productName: "Kosmos",
  appId: "com.kazui.kosmos",
  hotkey: "Alt+Space",
  autoupdaterEnabled: true,
  autorunEnabled: true,
  periodicMarketplaceCheckEnabled: true,
} as const;

describe("Desktop update bridge binding", () => {
  test("accepts only the current packaged production paths", () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-manager-update-"));
    const resourcesPath = path.join(root, "resources", "components", "manager", "resources");
    const executable = path.resolve(resourcesPath, "..", "..", "..", "..", "Kosmos.exe");
    const dataDir = path.join(root, "data");
    fs.mkdirSync(resourcesPath, { recursive: true });
    fs.mkdirSync(dataDir, { recursive: true });
    fs.writeFileSync(executable, "fixture");
    const input = {
      env: {
        KOSMOS_APP_EXECUTABLE: executable,
        KOSMOS_UPDATE_STATE_FILE: path.join(dataDir, "update-state.json"),
      },
      instance,
      dataDir,
      resourcesPath,
      platform: "win32" as const,
      isPackaged: true,
      exists: fs.existsSync,
    };

    expect(resolveDesktopUpdateBridge(input)).toEqual({
      executable,
      stateFile: path.join(dataDir, "update-state.json"),
    });
    expect(resolveDesktopUpdateBridge({ ...input, env: {} })).toEqual({
      executable,
      stateFile: path.join(dataDir, "update-state.json"),
    });
    expect(
      resolveDesktopUpdateBridge({
        ...input,
        env: { ...input.env, KOSMOS_APP_EXECUTABLE: path.join(root, "other.exe") },
      }),
    ).toBeNull();
    expect(
      resolveDesktopUpdateBridge({
        ...input,
        env: { ...input.env, KOSMOS_UPDATE_STATE_FILE: path.join(root, "other-state.json") },
      }),
    ).toBeNull();
    expect(
      resolveDesktopUpdateBridge({
        ...input,
        instance: { ...instance, kind: "dev", autoupdaterEnabled: false },
      }),
    ).toBeNull();
    fs.rmSync(root, { recursive: true, force: true });
  });
});

import { ipcMain, type WebContents } from "electron";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { assertSafeUserDataName, resolveSafeUserDataPath } from "./extension-manifest";
import type { JsonValue } from "./extension-permissions";

type UserDataCapability = "userData.read" | "userData.write";

interface ExtensionUserDataIpcOptions {
  assertHostPermission(sender: WebContents, capability: UserDataCapability): void;
  userDataDirForSender(sender: WebContents): string;
}

export function registerExtensionUserDataIpc({
  assertHostPermission,
  userDataDirForSender,
}: ExtensionUserDataIpcOptions): void {
  ipcMain.handle("kepler:extension:userData:path", (e) => {
    assertHostPermission(e.sender, "userData.read");
    return userDataDirForSender(e.sender);
  });

  ipcMain.handle("kepler:extension:userData:readJson", (e, name: string): JsonValue | null => {
    assertHostPermission(e.sender, "userData.read");
    assertSafeUserDataName(name);
    const dir = userDataDirForSender(e.sender);
    const filePath = path.join(dir, name);
    if (!existsSync(filePath)) return null;
    try {
      // SAFETY: Files written by writeJson contain JSON-compatible values.
      return JSON.parse(readFileSync(filePath, "utf8")) as JsonValue;
    } catch (err) {
      console.warn(`[kepler-shell] userData.readJson failed for ${filePath}:`, err);
      return null;
    }
  });

  ipcMain.handle("kepler:extension:userData:writeJson", (e, name: string, value: JsonValue): void => {
    assertHostPermission(e.sender, "userData.write");
    assertSafeUserDataName(name);
    const dir = userDataDirForSender(e.sender);
    const filePath = path.join(dir, name);
    writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
  });

  ipcMain.handle("kepler:extension:userData:readFile", (e, name: string): string | null => {
    assertHostPermission(e.sender, "userData.read");
    assertSafeUserDataName(name);
    const dir = userDataDirForSender(e.sender);
    const filePath = path.join(dir, name);
    if (!existsSync(filePath)) return null;
    try {
      return readFileSync(filePath, "utf8");
    } catch (err) {
      console.warn(`[kepler-shell] userData.readFile failed for ${filePath}:`, err);
      return null;
    }
  });

  ipcMain.handle(
    "kepler:extension:userData:writeFile",
    (e, name: string, content: string): void => {
      assertHostPermission(e.sender, "userData.write");
      assertSafeUserDataName(name);
      const dir = userDataDirForSender(e.sender);
      const filePath = path.join(dir, name);
      writeFileSync(filePath, content, "utf8");
    },
  );

  ipcMain.handle("kepler:extension:userData:readBinary", (e, name: string): string | null => {
    assertHostPermission(e.sender, "userData.read");
    const dir = userDataDirForSender(e.sender);
    const filePath = resolveSafeUserDataPath(dir, name);
    if (!existsSync(filePath)) return null;
    try {
      return readFileSync(filePath).toString("base64");
    } catch (err) {
      console.warn(`[kepler-shell] userData.readBinary failed for ${filePath}:`, err);
      return null;
    }
  });

  ipcMain.handle(
    "kepler:extension:userData:writeBinary",
    (e, name: string, base64: string): void => {
      assertHostPermission(e.sender, "userData.write");
      const dir = userDataDirForSender(e.sender);
      const filePath = resolveSafeUserDataPath(dir, name);
      mkdirSync(path.dirname(filePath), { recursive: true });
      writeFileSync(filePath, Buffer.from(base64, "base64"));
    },
  );

  ipcMain.handle("kepler:extension:userData:deleteFile", (e, name: string): boolean => {
    assertHostPermission(e.sender, "userData.write");
    const dir = userDataDirForSender(e.sender);
    const filePath = resolveSafeUserDataPath(dir, name);
    if (!existsSync(filePath)) return false;
    rmSync(filePath, { force: true });
    return true;
  });
}

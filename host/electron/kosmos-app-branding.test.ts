import path from "node:path";
import { expect, test } from "bun:test";
import {
  kosmosAppIcon,
  kosmosAppName,
  kosmosAppShortcutIcon,
} from "./kosmos-app-branding";

test("brands legacy package windows with Kosmos names and icons", () => {
  expect(kosmosAppName("com.kosmos.eden", "Eden")).toBe("Memoria");
  expect(kosmosAppName("delphi", "Delphi")).toBe("Agenda");
  expect(kosmosAppIcon("C:\\Kosmos\\resources", "com.kosmos.eden")).toBe(
    path.join("C:\\Kosmos\\resources", "app-icons", "memoria.png"),
  );
  expect(kosmosAppName("arrancador", "Arrancador")).toBe("Arcadia");
  expect(kosmosAppIcon("C:\\Kosmos\\resources", "com.kosmos.arcadia")).toBe(
    path.join("C:\\Kosmos\\resources", "app-icons", "arcadia.png"),
  );
  expect(kosmosAppShortcutIcon("C:\\Kosmos\\resources", "com.kosmos.memoria")).toBe(
    path.join("C:\\Kosmos\\resources", "app-icons", "memoria.ico"),
  );
});

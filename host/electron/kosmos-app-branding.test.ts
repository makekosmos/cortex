import path from "node:path";
import { expect, test } from "../test-support/node-test.mjs";
import { kosmosAppIcon, kosmosAppName, kosmosAppShortcutIcon } from "./kosmos-app-branding";

test("brands current package windows with Kosmos names and icons", () => {
  expect(kosmosAppName("com.kosmos.memoria", "Memoria")).toBe("Memoria");
  expect(kosmosAppName("com.kosmos.agenda", "Agenda")).toBe("Agenda");
  expect(kosmosAppIcon("C:\\Kosmos\\resources", "com.kosmos.memoria")).toBe(
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

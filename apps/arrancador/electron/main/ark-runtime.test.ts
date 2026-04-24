import { describe, expect, it } from "vitest";
import { buildPersonalSelectedSpace } from "../../../../packages/shared-space/selectedSpace";
import { syncArkRuntimeBinding } from "./ark-runtime";

describe("syncArkRuntimeBinding", () => {
  it("reuses services when selected space points to the same ark db", () => {
    const selection = buildPersonalSelectedSpace("D:/Vaults/Main", "eden");
    const services = { marker: "same-service" };

    const result = syncArkRuntimeBinding(
      {
        arkDbPath:
          "C:\\Users\\Kazui\\AppData\\Roaming\\Kepler\\spaces\\"
          + `${selection.spaceId}\\ark.db`,
        services,
      },
      {
        appDataPath: "C:\\Users\\Kazui\\AppData\\Roaming",
        selection,
        createServices: () => ({ marker: "new-service" }),
      },
    );

    expect(result.changed).toBe(false);
    expect(result.services).toBe(services);
  });

  it("recreates services when selected space changes ark db target", () => {
    const previousSelection = buildPersonalSelectedSpace("D:/Vaults/Main", "eden");
    const nextSelection = buildPersonalSelectedSpace("D:/Vaults/Other", "eden");
    const nextServices = { marker: "next-service" };

    const result = syncArkRuntimeBinding(
      {
        arkDbPath:
          "C:\\Users\\Kazui\\AppData\\Roaming\\Kepler\\spaces\\"
          + `${previousSelection.spaceId}\\ark.db`,
        services: { marker: "previous-service" },
      },
      {
        appDataPath: "C:\\Users\\Kazui\\AppData\\Roaming",
        selection: nextSelection,
        createServices: (arkDbPath) => {
          expect(arkDbPath).toBe(
            "C:\\Users\\Kazui\\AppData\\Roaming\\Kepler\\spaces\\"
              + `${nextSelection.spaceId}\\ark.db`,
          );
          return nextServices;
        },
      },
    );

    expect(result.changed).toBe(true);
    expect(result.services).toBe(nextServices);
    expect(result.arkDbPath).toBe(
      "C:\\Users\\Kazui\\AppData\\Roaming\\Kepler\\spaces\\"
        + `${nextSelection.spaceId}\\ark.db`,
    );
  });
});

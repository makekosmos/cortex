import { describe, expect, test } from "../../test-support/node-test.mjs";
import { integrationCards } from "./connection-helpers";
import type { IntegrationProvider, StoreListing } from "./manager-api";

const provider = (id: string, label = id): IntegrationProvider => ({
  id,
  label,
  credentialLabel: "Ключ",
  credentialUrl: "",
  hasCredential: false,
  enabled: false,
  settings: {
    intervalMinutes: 0,
    syncOnStartup: false,
    importedCount: 0,
  },
});

const integration = (id: string, packageId: string, name = id): StoreListing => ({
  id,
  kind: "integration",
  name,
  availability: { platforms: ["windows"] },
  distribution: {
    package_id: packageId,
    version: "1.0.0",
    connects_to: "external.example",
  },
});

describe("integration cards", () => {
  test("merges catalog integrations with installed providers and keeps Huawei visible", () => {
    const cards = integrationCards(
      [integration("integration.codewars", "codewars", "Codewars")],
      [provider("codewars"), provider("toggl")],
      "windows",
    );

    expect(cards.map((card) => card.id)).toEqual(["codewars", "com.kosmos.huawei-health", "toggl"]);
    expect(cards[0]?.provider?.id).toBe("codewars");
  });

  test("hides the bridge from the integrations surface", () => {
    const cards = integrationCards(
      [integration("integration.bridge", "ark-markdown-bridge", "ARK Markdown Bridge")],
      [],
      "windows",
    );

    expect(cards.map((card) => card.id)).toEqual(["com.kosmos.huawei-health"]);
  });

  test("filters catalog and first-party listings by the host platform", () => {
    const cards = integrationCards(
      [
        integration("integration.codewars", "codewars", "Codewars"),
        {
          ...integration("integration.portable", "portable", "Portable"),
          availability: { platforms: ["windows", "linux"] },
        },
      ],
      [provider("toggl")],
      "linux",
    );

    // Windows-only catalog listings and the Windows-only Huawei entry hide on
    // Linux; installed providers are Engine-reported and always listed.
    expect(cards.map((card) => card.id)).toEqual(["portable", "toggl"]);
  });
});

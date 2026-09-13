import { describe, expect, test } from "../../test-support/node-test.mjs";
import { filterListings, permissionSummaries, recommendForData } from "./store-catalog-helpers";
import type { DataSummary, InstalledStoreItem, StoreListing } from "./manager-api";

const listing = (extra: Partial<StoreListing>): StoreListing => ({
  id: "com.example.app",
  name: "Приложение",
  kind: "kosmos-package",
  availability: { platforms: ["windows"] },
  categories: ["работа"],
  data_compatibility: [
    { type: "com.kosmos.note", versions: "1.0.0", roles: ["reader"], fidelity: "full" },
  ],
  distribution: { package_id: "com.example.app", version: "1.0.0" },
  ...extra,
});
const installed = (extra: Partial<InstalledStoreItem> = {}): InstalledStoreItem => ({
  id: "com.example.app",
  name: "Приложение",
  version: "1.0.0",
  kind: "app",
  publisher: "Kosmos",
  revoked: false,
  effective_grants: [
    {
      type: "com.kosmos.note",
      roles: ["reader"],
      fields_read: ["title"],
      fields_write: [],
      relations_read: [],
      relations_write: [],
    },
  ],
  ...extra,
});

describe("marketplace behavior", () => {
  test("filters by tab, platform, category, data and install state", () => {
    const other = listing({
      id: "com.other",
      name: "Другое",
      kind: "integration",
      availability: { platforms: ["linux"] },
      categories: ["спорт"],
    });
    const result = filterListings(
      [listing({}), other],
      (item) => (item.id === "com.example.app" ? installed() : undefined),
      "installed",
      {
        platform: "windows",
        kind: "all",
        category: "работа",
        data: "com.kosmos.note",
        fidelity: "full",
        install: "installed",
      },
    );
    expect(result.map((item) => item.id)).toEqual(["com.example.app"]);
  });

  test("keeps only applications in the marketplace", () => {
    const integration = listing({ id: "com.integration", kind: "integration" });
    const external = listing({ id: "com.external", kind: "external-app", distribution: undefined });
    const apps = filterListings(
      [integration, external, listing({ id: "com.app" })],
      () => undefined,
      "discover",
      {
        platform: "all",
        kind: "all",
        category: "all",
        data: "all",
        fidelity: "all",
        install: "all",
      },
    );
    expect(apps.map((item) => item.id)).toEqual(["com.app"]);
  });

  test("recommends listings matching local ARK data types", () => {
    const summary: DataSummary = {
      types: [
        {
          id: "1",
          type_id: "com.kosmos.note",
          type_version: "1.0.0",
          name: "Заметки",
          count: 2,
          logical_bytes: 10,
        },
      ],
      managed_storage_bytes: 10,
    };
    expect(
      recommendForData(
        [listing({}), listing({ id: "com.other", data_compatibility: [] })],
        summary,
      ).map((item) => item.id),
    ).toEqual(["com.example.app"]);
  });

  test("renders effective grants, not a generic permission claim", () => {
    expect(permissionSummaries(installed())).toEqual([
      "com.kosmos.note · reader · читает: title; без записи полей",
    ]);
    expect(permissionSummaries(undefined)).toEqual([]);
  });
});

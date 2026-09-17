import { describe, expect, test } from "../test-support/node-test.mjs";
import type { ManagerApi, ManagerClient, PackageDisclosure, StoreListing } from "./manager-api";
import {
  capabilityLabel,
  disclosureSections,
  needsStoreDisclosure,
  typeLabel,
} from "./disclosure-helpers";
import { usePackageDisclosure } from "./composables/usePackageDisclosure";

const fixture: PackageDisclosure = {
  id: "com.kosmos.toggl",
  name: "Toggl Track",
  version: "0.1.0",
  kind: "source",
  publisher: "kosmos",
  capabilities: [
    { capability: "network", scopes: ["https://api.track.toggl.com/"] },
    { capability: "ark.write", scopes: ["upsert_object"] },
  ],
  data: [
    {
      type: "time_entry_obj",
      versions: "*",
      actions: ["create", "update", "delete"],
      fields_read: [],
      fields_write: [],
      relations_read: [],
      relations_write: [],
    },
    {
      type: "com.kosmos.note",
      versions: "^1",
      actions: ["read"],
      fields_read: ["props.title"],
      fields_write: [],
      relations_read: [],
      relations_write: ["parent"],
    },
  ],
  mappings: [{ type: "time_entry_obj", direction: "import", fidelity: "lossless" }],
};

function listing(tier?: StoreListing["publisher_tier"]): StoreListing {
  return { id: "app.example", kind: "kosmos-package", name: "Example", publisher_tier: tier };
}

describe("permission disclosure helpers", () => {
  test("renders declared scopes and data access in plain language", () => {
    const sections = disclosureSections(fixture);
    expect(sections.map((section) => section.title)).toEqual([
      "Данные",
      "Доступы",
      "Обмен данными",
    ]);
    expect(sections[0].lines).toContain("time entry — создаёт, изменяет, удаляет");
    expect(sections[0].lines[1]).toContain("note — читает");
    expect(sections[0].lines[1]).toContain("читает props.title");
    expect(sections[0].lines[1]).toContain("связывает parent");
    expect(sections[1].lines).toContain("Выходит в интернет: https://api.track.toggl.com/");
    expect(sections[1].lines).toContain("Создаёт и изменяет данные Kosmos: upsert_object");
    expect(sections[2].lines).toContain("time entry — отдаёт данные в Kosmos (без потерь)");
  });

  test("keeps unknown values readable and drops empty sections", () => {
    expect(disclosureSections({ ...fixture, capabilities: [], data: [], mappings: [] })).toEqual(
      [],
    );
    expect(capabilityLabel("custom.cap")).toBe("custom.cap");
    expect(typeLabel("com.kosmos.note")).toBe("note");
    expect(typeLabel("workout_obj")).toBe("workout");
  });

  test("gates the disclosure on third-party store listings only", () => {
    expect(needsStoreDisclosure(listing("kosmos"))).toBe(false);
    expect(needsStoreDisclosure(listing("verified"))).toBe(true);
    expect(needsStoreDisclosure(listing("community"))).toBe(true);
    expect(needsStoreDisclosure(listing())).toBe(true);
    expect(needsStoreDisclosure(listing("community"), true)).toBe(false);
  });
});

describe("usePackageDisclosure consent flow", () => {
  const client = (result: PackageDisclosure | null) => {
    const calls: string[] = [];
    const stub: Pick<ManagerClient, "call"> = {
      call: <T>(method: keyof ManagerApi) => {
        calls.push(String(method));
        // SAFETY: the test double answers every method with the same fixture.
        return Promise.resolve(result as T | null);
      },
    };
    return { calls, stub };
  };

  test("blocks the connect until the user confirms, then remembers consent", async () => {
    const { calls, stub } = client(fixture);
    const { state, request, confirm } = usePackageDisclosure(stub);
    let ran = 0;
    const proceed = async () => {
      ran += 1;
    };
    await request({ package_id: "com.kosmos.toggl", version: "0.1.0" }, "Toggl", proceed);
    expect(state.open).toBe(true);
    expect(ran).toBe(0);
    expect(calls).toEqual(["getPackageDisclosure"]);
    expect(state.sections.length).toBeGreaterThan(0);
    expect(state.name).toBe("Toggl Track");

    await confirm();
    expect(state.open).toBe(false);
    expect(ran).toBe(1);

    await request({ package_id: "com.kosmos.toggl", version: "0.1.0" }, "Toggl", proceed);
    expect(ran).toBe(2);
    expect(state.open).toBe(false);
    expect(calls).toEqual(["getPackageDisclosure"]);
  });

  test("decline keeps the connect from completing", async () => {
    const { stub } = client(fixture);
    const { state, request, decline } = usePackageDisclosure(stub);
    let ran = 0;
    await request({ package_id: "com.kosmos.toggl", version: "0.1.0" }, "Toggl", async () => {
      ran += 1;
    });
    decline();
    expect(state.open).toBe(false);
    expect(ran).toBe(0);
  });

  test("a missing contract still requires an explicit decision", async () => {
    const { stub } = client(null);
    const { state, request, decline, confirm } = usePackageDisclosure(stub);
    let ran = 0;
    await request({ package_id: "com.kosmos.toggl", version: "0.1.0" }, "Toggl", async () => {
      ran += 1;
    });
    expect(state.open).toBe(true);
    expect(state.unavailable).toBe(true);
    decline();
    expect(ran).toBe(0);
    await request({ package_id: "com.kosmos.toggl", version: "0.1.0" }, "Toggl", async () => {
      ran += 1;
    });
    await confirm();
    expect(ran).toBe(1);
  });
});

import { describe, expect, it } from "vitest";

import {
  type HelloMessage,
  type LiveChangeMessage,
  MAX_BATCH_SIZE,
  type PeerListMessage,
  type PeerRecord,
  type PingMessage,
  type SyncEntity,
  type VersionVector,
  compareHlc,
  computeLocalExcess,
  computeVectorDiff,
  deserializeMessage,
  generateId,
  isNewerHlc,
  mergePeerRecords,
  serializeMessage,
  splitIntoBatches,
} from "../lan-protocol";

import {
  formatSpaceCode,
  generateQrPayload,
  generateSpaceCode,
  parseQrPayload,
  parseSpaceCode,
} from "../../space/space-manager";

// ---------------------------------------------------------------------------

// HLC comparison

// ---------------------------------------------------------------------------

describe("compareHlc", () => {
  it("returns negative when a is older (earlier wall time)", () => {
    const a = "2026-03-28T10:00:00.000Z:000000:device-a";

    const b = "2026-03-28T14:00:00.000Z:000000:device-b";

    expect(compareHlc(a, b)).toBeLessThan(0);
  });

  it("returns positive when a is newer (later wall time)", () => {
    const a = "2026-03-28T14:00:00.000Z:000000:device-a";

    const b = "2026-03-28T10:00:00.000Z:000000:device-b";

    expect(compareHlc(a, b)).toBeGreaterThan(0);
  });

  it("compares counter when wall times are equal", () => {
    const a = "2026-03-28T14:00:00.000Z:000005:device-a";

    const b = "2026-03-28T14:00:00.000Z:000003:device-b";

    expect(compareHlc(a, b)).toBeGreaterThan(0);
  });

  it("compares device_id when wall time and counter are equal", () => {
    const a = "2026-03-28T14:00:00.000Z:000005:device-a";

    const b = "2026-03-28T14:00:00.000Z:000005:device-b";

    expect(compareHlc(a, b)).toBeLessThan(0);
  });

  it("returns 0 for identical HLCs", () => {
    const hlc = "2026-03-28T14:00:00.000Z:000000:device-a";

    expect(compareHlc(hlc, hlc)).toBe(0);
  });
});

describe("isNewerHlc", () => {
  it("returns true when first is newer", () => {
    expect(
      isNewerHlc(
        "2026-03-29T00:00:00.000Z:000000:d1",

        "2026-03-28T00:00:00.000Z:000000:d1",
      ),
    ).toBe(true);
  });

  it("returns false when first is older", () => {
    expect(
      isNewerHlc(
        "2026-03-27T00:00:00.000Z:000000:d1",

        "2026-03-28T00:00:00.000Z:000000:d1",
      ),
    ).toBe(false);
  });

  it("returns false when equal", () => {
    const hlc = "2026-03-28T00:00:00.000Z:000000:d1";

    expect(isNewerHlc(hlc, hlc)).toBe(false);
  });
});

// ---------------------------------------------------------------------------

// Version vector diff

// ---------------------------------------------------------------------------

describe("computeVectorDiff", () => {
  it("returns all remote IDs when local vector is empty", () => {
    const local: VersionVector = {};

    const remote: VersionVector = {
      "todo-1": "2026-03-28T10:00:00.000Z:000000:d1",

      "todo-2": "2026-03-28T11:00:00.000Z:000000:d1",
    };

    const diff = computeVectorDiff(local, remote);

    expect(diff.size).toBe(2);

    expect(diff.has("todo-1")).toBe(true);

    expect(diff.has("todo-2")).toBe(true);
  });

  it("returns empty set when local has all and newer", () => {
    const local: VersionVector = {
      "todo-1": "2026-03-29T00:00:00.000Z:000000:d1",
    };

    const remote: VersionVector = {
      "todo-1": "2026-03-28T00:00:00.000Z:000000:d1",
    };

    const diff = computeVectorDiff(local, remote);

    expect(diff.size).toBe(0);
  });

  it("returns IDs where remote is newer", () => {
    const local: VersionVector = {
      "todo-1": "2026-03-28T10:00:00.000Z:000000:d1",

      "todo-2": "2026-03-29T00:00:00.000Z:000000:d1",
    };

    const remote: VersionVector = {
      "todo-1": "2026-03-29T00:00:00.000Z:000000:d1", // newer

      "todo-2": "2026-03-28T00:00:00.000Z:000000:d1", // older
    };

    const diff = computeVectorDiff(local, remote);

    expect(diff.size).toBe(1);

    expect(diff.has("todo-1")).toBe(true);
  });

  it("includes IDs only present in remote", () => {
    const local: VersionVector = {
      "todo-1": "2026-03-28T10:00:00.000Z:000000:d1",
    };

    const remote: VersionVector = {
      "todo-1": "2026-03-28T10:00:00.000Z:000000:d1",

      "todo-3": "2026-03-28T12:00:00.000Z:000000:d2",
    };

    const diff = computeVectorDiff(local, remote);

    expect(diff.size).toBe(1);

    expect(diff.has("todo-3")).toBe(true);
  });
});

describe("computeLocalExcess", () => {
  it("returns IDs local has but remote doesn't", () => {
    const local: VersionVector = {
      "todo-1": "2026-03-28T10:00:00.000Z:000000:d1",

      "todo-2": "2026-03-28T11:00:00.000Z:000000:d1",
    };

    const remote: VersionVector = {
      "todo-1": "2026-03-28T10:00:00.000Z:000000:d1",
    };

    const excess = computeLocalExcess(local, remote);

    expect(excess.size).toBe(1);

    expect(excess.has("todo-2")).toBe(true);
  });
});

// ---------------------------------------------------------------------------

// Batch splitting

// ---------------------------------------------------------------------------

describe("splitIntoBatches", () => {
  it("returns empty array for empty input", () => {
    expect(splitIntoBatches([])).toEqual([]);
  });

  it("returns single batch for small input", () => {
    const entities: SyncEntity[] = Array.from({ length: 5 }, (_, i) => ({
      type: "todo" as const,

      id: `todo-${i}`,

      data: { title: `Task ${i}` },

      hlc: `2026-03-28T10:00:00.000Z:000000:d1`,
    }));

    const batches = splitIntoBatches(entities);

    expect(batches.length).toBe(1);

    expect(batches[0].length).toBe(5);
  });

  it("splits into multiple batches when exceeding MAX_BATCH_SIZE", () => {
    const entities: SyncEntity[] = Array.from(
      { length: MAX_BATCH_SIZE + 10 },

      (_, i) => ({
        type: "todo" as const,

        id: `todo-${i}`,

        data: { title: `Task ${i}` },

        hlc: `2026-03-28T10:00:00.000Z:000000:d1`,
      }),
    );

    const batches = splitIntoBatches(entities);

    expect(batches.length).toBe(2);

    expect(batches[0].length).toBe(MAX_BATCH_SIZE);

    expect(batches[1].length).toBe(10);
  });

  it("always includes at least one entity per batch", () => {
    const bigEntity: SyncEntity = {
      type: "todo",

      id: "big",

      data: { title: "x".repeat(2_000_000) }, // > 1MB

      hlc: "2026-03-28T10:00:00.000Z:000000:d1",
    };

    const batches = splitIntoBatches([bigEntity]);

    expect(batches.length).toBe(1);

    expect(batches[0].length).toBe(1);
  });
});

// ---------------------------------------------------------------------------

// Message serialization

// ---------------------------------------------------------------------------

describe("serializeMessage / deserializeMessage", () => {
  it("roundtrips a hello message", () => {
    const msg: HelloMessage = {
      type: "hello",

      protocol_version: 1,

      device_id: "delphi-web-abc",

      device_name: "Delphi Electron",

      space_id: "abcdef1234567890",

      addresses: ["192.168.1.70:21531", "[fe80::1%en0]:21531"],
    };

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized);

    expect(deserialized).toEqual(msg);
  });

  it("roundtrips a hello message without addresses (backward compat)", () => {
    const msg: HelloMessage = {
      type: "hello",

      protocol_version: 1,

      device_id: "delphi-web-abc",

      device_name: "Delphi Electron",

      space_id: "abcdef1234567890",
    };

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized);

    expect(deserialized).toEqual(msg);
  });

  it("roundtrips a live_change message", () => {
    const msg: LiveChangeMessage = {
      type: "live_change",

      change_id: "123-abc",

      entity: {
        type: "todo",

        id: "todo-1",

        data: { title: "Test task", isCompleted: false },

        hlc: "2026-03-28T10:00:00.000Z:000000:d1",
      },
    };

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized);

    expect(deserialized).toEqual(msg);
  });

  it("roundtrips a ping message", () => {
    const msg: PingMessage = { type: "ping", ts: 1711638000000 };

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized);

    expect(deserialized).toEqual(msg);
  });

  it("roundtrips a peer_list message", () => {
    const msg: PeerListMessage = {
      type: "peer_list",

      peers: [
        {
          device_id: "device-1",

          device_name: "MacBook",

          addresses: ["192.168.1.70:21531"],

          last_seen: "2026-04-01T13:00:00Z",
        },

        {
          device_id: "device-2",

          device_name: "Pixel 7",

          addresses: ["192.168.1.151:21531", "[fe80::440]:21531"],

          last_seen: "2026-04-01T13:01:00Z",

          last_address: "192.168.1.151:21531",
        },
      ],
    };

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized);

    expect(deserialized).toEqual(msg);
  });

  it("returns null for invalid JSON", () => {
    expect(deserializeMessage("not json")).toBeNull();
  });

  it("returns null for JSON without type", () => {
    expect(deserializeMessage('{"foo":"bar"}')).toBeNull();
  });
});

// ---------------------------------------------------------------------------

// generateId

// ---------------------------------------------------------------------------

describe("generateId", () => {
  it("returns unique IDs", () => {
    const ids = new Set(Array.from({ length: 100 }, () => generateId()));

    expect(ids.size).toBe(100);
  });

  it("returns non-empty string", () => {
    expect(generateId().length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------

// mergePeerRecords

// ---------------------------------------------------------------------------

describe("mergePeerRecords", () => {
  it("adds new peers from incoming list", () => {
    const existing: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "Device 1",

        addresses: ["192.168.1.1:21531"],

        last_seen: "2026-04-01T10:00:00Z",
      },
    ];

    const incoming: PeerRecord[] = [
      {
        device_id: "d2",

        device_name: "Device 2",

        addresses: ["192.168.1.2:21531"],

        last_seen: "2026-04-01T11:00:00Z",
      },
    ];

    const merged = mergePeerRecords(existing, incoming);

    expect(merged).toHaveLength(2);

    expect(merged.find((p) => p.device_id === "d2")).toBeTruthy();
  });

  it("merges addresses (union) for same device", () => {
    const existing: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "Device 1",

        addresses: ["192.168.1.1:21531", "10.0.0.1:21531"],

        last_seen: "2026-04-01T10:00:00Z",
      },
    ];

    const incoming: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "Device 1",

        addresses: ["192.168.1.1:21531", "192.168.0.50:21531"],

        last_seen: "2026-04-01T11:00:00Z",
      },
    ];

    const merged = mergePeerRecords(existing, incoming);

    expect(merged).toHaveLength(1);

    const d1 = merged[0];

    expect(d1.addresses).toContain("192.168.1.1:21531");

    expect(d1.addresses).toContain("10.0.0.1:21531");

    expect(d1.addresses).toContain("192.168.0.50:21531");

    expect(d1.addresses).toHaveLength(3);
  });

  it("preserves newest last_seen", () => {
    const existing: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "Device 1 Old",

        addresses: ["192.168.1.1:21531"],

        last_seen: "2026-04-01T12:00:00Z",
      },
    ];

    const incoming: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "Device 1 New",

        addresses: ["192.168.1.2:21531"],

        last_seen: "2026-04-01T11:00:00Z", // older
      },
    ];

    const merged = mergePeerRecords(existing, incoming);

    expect(merged[0].last_seen).toBe("2026-04-01T12:00:00Z");

    expect(merged[0].device_name).toBe("Device 1 Old"); // kept from newer
  });

  it("updates device_name from newer record", () => {
    const existing: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "Old Name",

        addresses: [],

        last_seen: "2026-04-01T10:00:00Z",
      },
    ];

    const incoming: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "New Name",

        addresses: [],

        last_seen: "2026-04-01T12:00:00Z",
      },
    ];

    const merged = mergePeerRecords(existing, incoming);

    expect(merged[0].device_name).toBe("New Name");
  });

  it("preserves last_address from newer record", () => {
    const existing: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "D1",

        addresses: ["192.168.1.1:21531"],

        last_seen: "2026-04-01T10:00:00Z",

        last_address: "192.168.1.1:21531",
      },
    ];

    const incoming: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "D1",

        addresses: ["192.168.1.2:21531"],

        last_seen: "2026-04-01T12:00:00Z",

        last_address: "192.168.1.2:21531",
      },
    ];

    const merged = mergePeerRecords(existing, incoming);

    expect(merged[0].last_address).toBe("192.168.1.2:21531");
  });

  it("handles empty inputs", () => {
    expect(mergePeerRecords([], [])).toEqual([]);

    const record: PeerRecord = {
      device_id: "d1",

      device_name: "D1",

      addresses: ["addr"],

      last_seen: "2026-04-01T10:00:00Z",
    };

    expect(mergePeerRecords([], [record])).toHaveLength(1);

    expect(mergePeerRecords([record], [])).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------

// QR payload generation and parsing

// ---------------------------------------------------------------------------

describe("generateQrPayload / parseQrPayload", () => {
  it("generates ark:// payload with code and addresses", () => {
    const code = "ABCDEFGH1234";

    const addresses = ["192.168.1.70:21531", "[fe80::1%en0]:21531"];

    const payload = generateQrPayload(code, addresses);

    expect(payload).toContain("ark://join?");

    expect(payload).toContain("code=");

    expect(payload).toContain("addrs=");
  });

  it("roundtrips code and addresses", () => {
    const code = "ABCDEFGH1234";

    const addresses = ["192.168.1.70:21531", "10.0.0.1:21531"];

    const payload = generateQrPayload(code, addresses);

    const parsed = parseQrPayload(payload);

    expect(parsed).not.toBeNull();

    expect(parsed!.code).toBe(code);

    expect(parsed!.addresses).toEqual(addresses);
  });

  it("parses payload with empty addresses", () => {
    const code = "ABCDEFGH1234";

    const payload = generateQrPayload(code, []);

    const parsed = parseQrPayload(payload);

    expect(parsed).not.toBeNull();

    expect(parsed!.code).toBe(code);

    expect(parsed!.addresses).toEqual([]);
  });

  it("parses a raw space code as fallback", () => {
    const parsed = parseQrPayload("ABCD-EFGH-1234");

    expect(parsed).not.toBeNull();

    expect(parsed!.code).toBe("ABCDEFGH1234");

    expect(parsed!.addresses).toEqual([]);
  });

  it("returns null for invalid payload", () => {
    expect(parseQrPayload("")).toBeNull();

    expect(parseQrPayload("http://example.com")).toBeNull();

    expect(parseQrPayload("ark://join?badparam=x")).toBeNull();
  });
});

// ---------------------------------------------------------------------------

// Space code generation and parsing

// ---------------------------------------------------------------------------

describe("space code", () => {
  it("generateSpaceCode returns 12-char Base32-Crockford", () => {
    const code = generateSpaceCode();

    expect(code).toHaveLength(12);

    const crockford = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

    for (const ch of code) {
      expect(crockford).toContain(ch);
    }
  });

  it("formatSpaceCode formats 12-char as XXXX-XXXX-XXXX", () => {
    expect(formatSpaceCode("ABCDEFGH1234")).toBe("ABCD-EFGH-1234");
  });

  it("formatSpaceCode formats 7-char as XXXX-XXX (legacy)", () => {
    expect(formatSpaceCode("ABCDEFG")).toBe("ABCD-EFG");
  });

  it("parseSpaceCode parses 12-char code", () => {
    expect(parseSpaceCode("ABCD-EFGH-1234")).toBe("ABCDEFGH1234");

    expect(parseSpaceCode("abcdefgh1234")).toBe("ABCDEFGH1234");
  });

  it("parseSpaceCode parses 7-char code (legacy)", () => {
    expect(parseSpaceCode("ABCD-EFG")).toBe("ABCDEFG");
  });

  it("parseSpaceCode rejects invalid codes", () => {
    expect(parseSpaceCode("")).toBeNull();

    expect(parseSpaceCode("ABCDE")).toBeNull();

    expect(parseSpaceCode("ABCDIEFGH123")).toBeNull(); // I is invalid
  });
});

// ---------------------------------------------------------------------------

// Multi-address peer record handling

// ---------------------------------------------------------------------------

describe("multi-address peer record handling", () => {
  it("HelloMessage can include addresses", () => {
    const msg: HelloMessage = {
      type: "hello",

      protocol_version: 1,

      device_id: "test",

      device_name: "Test",

      space_id: "space",

      addresses: ["192.168.1.1:21531", "10.0.0.1:21531"],
    };

    const serialized = serializeMessage(msg);

    const deserialized = deserializeMessage(serialized) as HelloMessage;

    expect(deserialized.addresses).toEqual([
      "192.168.1.1:21531",

      "10.0.0.1:21531",
    ]);
  });

  it("PeerRecord supports multiple addresses and last_address", () => {
    const record: PeerRecord = {
      device_id: "d1",

      device_name: "Phone",

      addresses: ["192.168.1.100:21531", "[fe80::1%wlan0]:21531"],

      last_seen: "2026-04-01T10:00:00Z",

      last_address: "192.168.1.100:21531",
    };

    // Roundtrip through JSON

    const json = JSON.stringify(record);

    const parsed = JSON.parse(json) as PeerRecord;

    expect(parsed.addresses).toHaveLength(2);

    expect(parsed.last_address).toBe("192.168.1.100:21531");
  });

  it("merge with complex scenario: 3 peers, overlapping addresses", () => {
    const existing: PeerRecord[] = [
      {
        device_id: "d1",

        device_name: "MacBook",

        addresses: ["192.168.1.70:21531"],

        last_seen: "2026-04-01T10:00:00Z",
      },

      {
        device_id: "d2",

        device_name: "Pixel",

        addresses: ["192.168.1.100:21531"],

        last_seen: "2026-04-01T10:00:00Z",
      },
    ];

    const incoming: PeerRecord[] = [
      {
        device_id: "d2",

        device_name: "Pixel 7",

        addresses: ["192.168.0.50:21531", "192.168.1.100:21531"],

        last_seen: "2026-04-01T12:00:00Z",
      },

      {
        device_id: "d3",

        device_name: "iPad",

        addresses: ["192.168.1.200:21531"],

        last_seen: "2026-04-01T11:00:00Z",
      },
    ];

    const merged = mergePeerRecords(existing, incoming);

    expect(merged).toHaveLength(3);

    const d2 = merged.find((p) => p.device_id === "d2")!;

    expect(d2.device_name).toBe("Pixel 7"); // updated (newer)

    expect(d2.addresses).toContain("192.168.1.100:21531");

    expect(d2.addresses).toContain("192.168.0.50:21531");

    const d3 = merged.find((p) => p.device_id === "d3")!;

    expect(d3.device_name).toBe("iPad");
  });
});

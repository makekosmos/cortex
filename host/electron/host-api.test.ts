import { describe, expect, test } from "bun:test";

import {
  hasArkGrant,
  hasLaunchReadPermission,
  hasLauncherGrant,
  hasManifestGrant,
  isV2Launch,
  isInstalledEnabledApp,
  launchRenewalDelayMs,
  redactedCrashMetadata,
  requiredCapability,
  SAFE_ID,
} from "./host-api";

describe("desktop-host validation", () => {
  const launch = {
    id: "notes.app",
    version: "1.0.0",
    name: "Notes",
    launch_url: "https://notes.invalid/index.html",
    permissions: [{ capability: "ark.read", scopes: ["list_objects"] }],
    launch_id: "00000000-0000-4000-8000-000000000001",
    ttl_seconds: 300,
    expires_at: "2026-01-01T00:00:00Z",
  };

  test("accepts safe package ids only", () => {
    expect(SAFE_ID.test("notes.app")).toBe(true);
    expect(SAFE_ID.test("../secrets")).toBe(false);
    expect(SAFE_ID.test("Bad ID")).toBe(false);
  });

  test("requires explicit operation scopes", () => {
    expect(requiredCapability("list_objects")).toBe("ark.read");
    expect(requiredCapability("noop")).toBeUndefined();
    expect(
      hasManifestGrant(
        [{ capability: "ark.read", scopes: ["list_objects"] }],
        "ark.read",
        "list_objects",
      ),
    ).toBe(true);
    expect(
      hasManifestGrant([{ capability: "ark.read", scopes: [] }], "ark.read", "list_objects"),
    ).toBe(false);
    expect(
      hasManifestGrant([{ capability: "ark.read", scopes: ["*"] }], "ark.read", "list_objects"),
    ).toBe(false);
    expect(
      hasManifestGrant(
        [{ capability: "ark", scopes: ["list_objects"] }],
        "ark.read",
        "list_objects",
      ),
    ).toBe(false);
  });

  test("launcher grants are exact and cannot wildcard", () => {
    expect(
      hasLauncherGrant(
        [{ capability: "launcher.search", scopes: ["commands.invoke"] }],
        "commands.invoke",
      ),
    ).toBe(true);
    expect(
      hasLauncherGrant([{ capability: "launcher.search", scopes: ["*"] }], "commands.invoke"),
    ).toBe(false);
    expect(
      hasLauncherGrant(
        [{ capability: "ark.read", scopes: ["commands.invoke"] }],
        "commands.invoke",
      ),
    ).toBe(false);
  });

  test("withholds the ARK bridge from Apps without an explicit ARK grant", () => {
    expect(hasArkGrant([{ capability: "launcher.search", scopes: ["commands.list"] }])).toBe(false);
    expect(hasArkGrant([{ capability: "ark.read", scopes: ["list_objects"] }])).toBe(true);
  });

  test("selects launch-scoped v2 authority only from Engine markers", () => {
    expect(isV2Launch({ manifest_schema_version: 2 })).toBe(true);
    expect(isV2Launch({ data_api: "http://127.0.0.1/v1/apps/launch/id/ark" })).toBe(true);
    expect(isV2Launch({ manifest_schema_version: 1 })).toBe(false);
    expect(
      hasLaunchReadPermission(
        {
          ...launch,
          manifest_schema_version: 2,
          permissions: [],
          effective_read_types: ["com.kosmos.note"],
        },
        { type_id: "com.kosmos.note" },
      ),
    ).toBe(true);
    expect(
      hasLaunchReadPermission(
        {
          ...launch,
          manifest_schema_version: 2,
          permissions: [],
          effective_read_types: ["com.kosmos.note"],
        },
        { type_id: "com.kosmos.task" },
      ),
    ).toBe(false);
    expect(
      hasLaunchReadPermission(
        {
          ...launch,
          manifest_schema_version: 2,
          permissions: [],
          effective_read_types: ["com.kosmos.note"],
        },
        { entity: { data: { type_id: "com.kosmos.note" } } },
      ),
    ).toBe(true);
    expect(
      hasLaunchReadPermission(
        {
          ...launch,
          manifest_schema_version: 2,
          permissions: [],
          effective_read_types: ["com.kosmos.note"],
        },
        { entity: { data: { typeId: "com.kosmos.task" } } },
      ),
    ).toBe(false);
    expect(
      hasLaunchReadPermission(
        {
          ...launch,
          manifest_schema_version: 2,
          permissions: [],
          effective_read_types: ["com.kosmos.note"],
        },
        { entity: { data: { type_id: 42 } } },
      ),
    ).toBe(false);
    expect(
      hasLaunchReadPermission(
        {
          ...launch,
          manifest_schema_version: 2,
          permissions: [{ capability: "ark.read", scopes: ["objects.subscribe"] }],
        },
        { type_id: "com.kosmos.note" },
      ),
    ).toBe(false);
    expect(hasLaunchReadPermission({ ...launch, manifest_schema_version: 2 }, {})).toBe(false);
    const agentsLaunch = {
      ...launch,
      manifest_schema_version: 2,
      permissions: [],
      effective_events: ["agents_event"],
    };
    expect(hasLaunchReadPermission(agentsLaunch)).toBe(true);
    expect(hasLaunchReadPermission(agentsLaunch, { event: "agents_event", data: {} })).toBe(true);
    expect(
      hasLaunchReadPermission(agentsLaunch, { event: "entity_changed", type_id: "note" }),
    ).toBe(false);
    expect(
      hasLaunchReadPermission(
        { ...agentsLaunch, effective_events: [], effective_read_types: ["com.kosmos.note"] },
        { event: "agents_event", data: { type_id: "com.kosmos.note" } },
      ),
    ).toBe(false);
    expect(hasLaunchReadPermission(launch)).toBe(true);
  });

  test("renewal scheduling stops at expiry and backs off failed attempts", () => {
    const now = Date.parse("2026-01-01T00:00:00Z");
    expect(launchRenewalDelayMs("2026-01-01T00:01:00Z", now)).toBe(30_000);
    expect(launchRenewalDelayMs("2026-01-01T00:00:10Z", now, true)).toBe(9_000);
    expect(launchRenewalDelayMs("2025-12-31T23:59:59Z", now, true)).toBeNull();
    expect(launchRenewalDelayMs("not-a-date", now)).toBeNull();
  });

  test("filters shortcut input and redacts crash metadata", () => {
    expect(isInstalledEnabledApp({ id: "notes.app", enabled: true, revoked: false })).toBe(true);
    expect(
      isInstalledEnabledApp({
        id: "notes.app",
        enabled: false,
        revoked: false,
      }),
    ).toBe(false);
    expect(isInstalledEnabledApp({ id: "notes.app", enabled: true, revoked: true })).toBe(false);
    expect(
      redactedCrashMetadata("notes.app", "1.2.3", {
        reason: "crashed",
        exitCode: 9,
      }),
    ).toEqual({
      component: "renderer",
      app_id: "notes.app",
      version: "1.2.3",
      reason: "crashed",
      exit_code: 9,
    });
    expect(
      redactedCrashMetadata("notes.app", "1.2.3", {
        reason: "C:\\secret",
        exitCode: "stderr",
      }),
    ).toEqual({
      component: "renderer",
      app_id: "notes.app",
      version: "1.2.3",
      reason: "unknown",
      exit_code: null,
    });
  });
});

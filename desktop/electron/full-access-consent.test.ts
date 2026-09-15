import { describe, expect, test } from "../test-support/node-test.mjs";
import {
  assertMainRendererArkRequestAllowed,
  buildFullAccessConsentDetails,
  fullAccessConsentIssue,
  fullAccessSessionParams,
  isFullAccessConsentOperation,
  withoutFullAccessConfirmation,
} from "./full-access-consent";

describe("full-access consent details", () => {
  test("recognizes every host-only consent operation", () => {
    expect(isFullAccessConsentOperation("agents.full_access_consent.issue")).toBe(true);
    expect(isFullAccessConsentOperation("agents.sessions.issue_full_access_consent")).toBe(true);
    expect(isFullAccessConsentOperation("agents.sessions.approve_full_access_consent")).toBe(true);
    expect(isFullAccessConsentOperation("agents.sessions.respond_full_access_consent")).toBe(true);
    expect(isFullAccessConsentOperation("agents.sessions.create")).toBe(false);
    expect(() => assertMainRendererArkRequestAllowed("agents.sessions.create", true)).toThrow(
      "host-only",
    );
    expect(() =>
      assertMainRendererArkRequestAllowed("agents.sessions.create", false),
    ).not.toThrow();
  });
  test("uses a stable hash and bounded prompt summary", () => {
    const prompt = "  inspect\n files and fix the bug  ";
    const details = buildFullAccessConsentDetails({
      packageId: "cortex",
      packageVersion: "1.2.3",
      projectId: "project-1",
      projectPath: "C:\\work\\project-1",
      prompt,
      model: "gpt-5",
    });
    expect(details.promptSummary).toBe("inspect files and fix the bug");
    expect(details.model).toBe("gpt-5");
    expect(details.promptHash).toBe(
      "3c1d04c6fbef55859dc7517373e6495c970eb017e4745c1874fcb7ee45ec08fe",
    );
  });

  test("requires the exact full-access inputs and strips renderer confirmation", () => {
    expect(
      fullAccessSessionParams({
        project_id: "project-1",
        prompt: "do work",
        mode: "full-access",
        model: "gpt-5",
        full_access_confirmed: true,
        consent_token: "spoofed",
      }),
    ).toEqual({ projectId: "project-1", prompt: "do work", model: "gpt-5" });
    expect(
      withoutFullAccessConfirmation({ full_access_confirmed: true, consent_token: "x" }),
    ).toEqual({});
    expect(
      buildFullAccessConsentDetails({
        packageId: "cortex",
        packageVersion: "1.2.3",
        projectId: "project-1",
        projectPath: "C:\\work\\project-1",
        prompt: "do work",
      }).model,
    ).toBe("Default model");
    expect(
      fullAccessSessionParams({ project_id: "project-1", prompt: "do work", mode: "full-access" }),
    ).toEqual({
      projectId: "project-1",
      prompt: "do work",
    });
    expect(fullAccessSessionParams({ mode: "auto-review", prompt: "do work" })).toBeNull();
  });

  test("accepts only operation-bound Runtime metadata", () => {
    const expected = {
      packageId: "daedalus",
      packageVersion: "1.2.3",
      projectId: "project-1",
      prompt: "do work",
      model: "gpt-5",
    };
    const response = {
      request_id: "request-1",
      package_id: "daedalus",
      package_version: "1.2.3",
      project_id: "project-1",
      project_path: "C:\\work\\project-1",
      mode: "full-access",
      model: "gpt-5",
      prompt_sha256: "64d6f071c16a0984c4d1331002dd6f6a2ec7a503d23b38648fa52069af7330e7",
      expires_at: "2026-08-27T12:00:00Z",
    };
    expect(fullAccessConsentIssue(response, expected)?.request_id).toBe("request-1");
    expect(fullAccessConsentIssue({ ...response, project_id: "other" }, expected)).toBeNull();
    expect(fullAccessConsentIssue({ ...response, prompt_sha256: "forged" }, expected)).toBeNull();
  });
});

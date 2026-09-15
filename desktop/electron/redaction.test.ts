import { expect, test } from "../test-support/node-test.mjs";
import type { JsonRecord } from "./extension-permissions";

import {
  createCrashMetadata,
  isSupportTextFile,
  redactText,
  redactTextFile,
  redactUnknown,
} from "./redaction";

test("redactText masks secrets, email and user paths", () => {
  const token = "a".repeat(64);
  const output = redactText(
    `Bearer abc.def auth_token=${token} alice@example.com ` +
      "C:\\Users\\alice\\vault\\note.md /home/alice/vault/note.md",
  );
  expect(output).not.toContain("abc.def");
  expect(output).not.toContain(token);
  expect(output).not.toContain("alice@example.com");
  expect(output).not.toContain("note.md");
});

test("redactUnknown recursively removes sensitive values", () => {
  // SAFETY: The fixture is a JSON object and redaction preserves its object shape.
  const output = redactUnknown({
    safe: "ok",
    nested: {
      token: "secret-value",
      payload: { title: "private note" },
    },
    // SAFETY: The surrounding boundary establishes this documented contract.
  }) as JsonRecord;
  expect(output.safe).toBe("ok");
  expect(JSON.stringify(output)).not.toContain("secret-value");
  expect(JSON.stringify(output)).not.toContain("private note");
});

test("support bundle accepts text only and re-redacts each line", () => {
  expect(isSupportTextFile("engine.log")).toBe(true);
  expect(isSupportTextFile("panic.txt")).toBe(true);
  expect(isSupportTextFile("dump.dmp")).toBe(false);
  expect(redactTextFile("alice@example.com\nsafe")).toBe("[REDACTED]\nsafe");
  expect(redactTextFile('{"payload":{"title":"private note"}}')).not.toContain("private note");
});

test("crash metadata has IDs and no raw sensitive detail", () => {
  const correlationId = "00000000-0000-4000-8000-000000000001";
  const crash = createCrashMetadata("electron-renderer", correlationId, {
    reason: "alice@example.com",
    exitCode: 9,
  });
  expect(crash.component).toBe("electron-renderer");
  expect(crash.correlationId).toBe(correlationId);
  expect(String(crash.crashId)).toMatch(/^[0-9a-f-]{36}$/);
  expect(crash.reason).toBe("[REDACTED]");
  expect(crash.exitCode).toBe(9);
});

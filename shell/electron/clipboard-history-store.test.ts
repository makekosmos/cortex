import { test, expect } from "bun:test";
import {
  createClipboardHistoryStore,
  defaultClipboardHistorySettings,
  fingerprintImageBytes,
} from "./clipboard-history-store";

test("fingerprintImageBytes стабилен для одинакового входа", () => {
  const a = Buffer.alloc(4096, 7);
  const b = Buffer.alloc(4096, 7);
  expect(fingerprintImageBytes(64, 64, a)).toBe(fingerprintImageBytes(64, 64, b));
});

test("fingerprintImageBytes чувствителен к изменению пикселей", () => {
  const a = Buffer.alloc(4096, 7);
  const c = Buffer.alloc(4096, 7);
  c[2048] = 99;
  expect(fingerprintImageBytes(64, 64, a)).not.toBe(fingerprintImageBytes(64, 64, c));
});

test("fingerprintImageBytes различает размер при том же буфере", () => {
  const a = Buffer.alloc(4096, 7);
  expect(fingerprintImageBytes(64, 64, a)).not.toBe(fingerprintImageBytes(32, 64, a));
});

test("default maxBytes снижен до 64 MB", () => {
  expect(defaultClipboardHistorySettings().maxBytes).toBe(64 * 1024 * 1024);
});

test("запись больше per-item лимита не сохраняется", () => {
  const store = createClipboardHistoryStore({ maxItemBytes: 1024 });
  expect(store.record("x".repeat(5000))).toBeNull();
  expect(store.list()).toHaveLength(0);
});

test("запись в пределах per-item лимита сохраняется", () => {
  const store = createClipboardHistoryStore({ maxItemBytes: 1024 });
  expect(store.record("привет")).not.toBeNull();
  expect(store.list()).toHaveLength(1);
});

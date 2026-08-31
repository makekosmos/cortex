import fs from "node:fs";
import path from "node:path";
import { expect, type Page } from "@playwright/test";
import type { JsonValue } from "./host-runtime";

export function responseNumber(value: JsonValue, key: string) {
  expect(value).toMatchObject({ ok: true, data: { [key]: expect.any(Number) } });
  // SAFETY: the assertion above established a successful response with a numeric keyed field.
  return (value as { data: Record<string, number> }).data[key];
}

export function responseMessage(value: JsonValue) {
  expect(value).toMatchObject({ ok: false, message: expect.any(String) });
  // SAFETY: the assertion above established a denied response with a string message.
  return (value as { message: string }).message;
}

export function writeOverdueThirdWorkState(dataDir: string) {
  fs.writeFileSync(
    path.join(dataDir, "pomodoro-state.json"),
    JSON.stringify({
      version: 1,
      phase: "work",
      remainingMs: 0,
      totalMs: 25 * 60_000,
      completedPomodoros: 2,
      isRunning: true,
      isPaused: false,
      phaseEndsAtMs: 0,
      lastConfig: {
        workMin: 25,
        shortBreakMin: 5,
        longBreakMin: 15,
        pomodorosUntilLongBreak: 4,
        autoStartWork: false,
        autoStartBreak: false,
        title: "Ordo Host E2E",
        tasks: [],
      },
    }),
  );
}

export function expectOverdueThirdWork(value: JsonValue) {
  expect(value, JSON.stringify(value)).toMatchObject({
    ok: true,
    data: {
      phase: "shortBreak",
      completedPomodoros: 3,
      isRunning: false,
      isPaused: false,
      totalMs: 5 * 60_000,
      remainingMs: 5 * 60_000,
    },
  });
}

export async function initializeOrdo(page: Page) {
  const custom = await page.evaluate(() =>
    window.kosmosApp.ark.request("focus.upsert_blocklist", {
      name: "Host E2E custom",
      domains: ["Example.COM", "e2e.invalid"],
    }),
  );
  expect(custom).toMatchObject({ ok: true, data: { id: expect.any(String) } });
  // SAFETY: the assertion above established a successful response with a string data.id.
  const id = (custom as { data: { id: string } }).data.id;
  const rest = await page.evaluate(async (blocklistId) => {
    const listed = await window.kosmosApp.ark.request("focus.list_blocklists", {});
    const active = await window.kosmosApp.ark.request("focus.set_active_state", {
      active: true,
      blocklist_id: blocklistId,
    });
    const state = await window.kosmosApp.ark.request("focus.get_active_state", {});
    const domains = await window.kosmosApp.ark.request("focus.resolve_blocklist_domains", {
      id: blocklistId,
    });
    const start = await window.kosmosApp.ark.request("pomodoro.start", {
      config: {
        workMin: 1,
        shortBreakMin: 1,
        longBreakMin: 1,
        pomodorosUntilLongBreak: 4,
        title: "Ordo Host E2E",
        tasks: [],
      },
    });
    const paused = await window.kosmosApp.ark.request("pomodoro.pause", {});
    const resumed = await window.kosmosApp.ark.request("pomodoro.resume", {});
    const denied = await Promise.all([
      window.kosmosApp.ark.request("dictation.get_state", {}),
      window.kosmosApp.ark.request("pomodoro.skip", {}),
    ]);
    return { listed, active, state, domains, start, paused, resumed, denied };
  }, id);
  return { custom, id, ...rest };
}

#!/usr/bin/env node
// dictation-file-e2e.mjs — feed an audio file through the real dictation
// pipeline (capture.start → drain → streaming chunks → transcribe) without
// a microphone or speakers. `sourceFile`/`sourceSpeed` are per-capture
// params: the audio-capturer helper is spawned per capture and gets them
// as env vars — the Engine env is never hijacked, and file-source captures
// emit no dictation_audio_level broadcasts so the pill stays hidden.
//
//   node scripts/dictation-file-e2e.mjs <audio-file> [--speed N]
//
// Delivery is forced to text_only on the transcribe call, so nothing is
// pasted into the focused app and the clipboard is never written.

import { readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import os from "node:os";
import path from "node:path";

const args = process.argv.slice(2);
const file = args.find((a) => !a.startsWith("--"));
const speedFlag = args.indexOf("--speed");
const speed = speedFlag >= 0 ? Number(args[speedFlag + 1]) : 1;
if (!file || !Number.isFinite(speed) || speed <= 0) {
  console.error("usage: node scripts/dictation-file-e2e.mjs <audio-file> [--speed N]");
  process.exit(2);
}

// Engine RPC over the dev lock file (same auth headers the GPUI app uses).
const dataDir = process.env.MUNDUS_DATA_DIR ?? path.join(os.homedir(), ".config", "Mundus");
const lock = JSON.parse(await readFile(path.join(dataDir, "engine.lock.json"), "utf8"));
const rpc = async (operation, params = {}) => {
  const res = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      "x-kosmos-client-pid": String(process.pid),
      "x-kosmos-api-version": "1.0.0",
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ operation, ...params, _req_id: crypto.randomUUID() }),
  });
  const body = await res.json();
  if (!body.ok) throw new Error(`${operation}: ${body.error?.message ?? JSON.stringify(body)}`);
  return body.data;
};

// afinfo prints "estimated duration: N sec" for wav/mp3/m4a alike.
const afinfo = execFileSync("afinfo", [file], { encoding: "utf8" });
const durationSec = Number(/estimated duration: ([\d.]+) sec/.exec(afinfo)?.[1]);
if (!Number.isFinite(durationSec) || durationSec <= 0) {
  throw new Error(`cannot determine duration of ${file}`);
}

console.log(`${file} (${durationSec.toFixed(1)}s @speed ${speed})`);
const stopWatch = { start: 0 };
try {
  const started = await rpc("dictation.capture.start", {
    sourceFile: path.resolve(file),
    sourceSpeed: speed,
  });
  const captureId = started.captureId;
  console.log(`recording (captureId ${captureId.slice(0, 8)}…)`);
  await new Promise((r) => setTimeout(r, (durationSec / speed) * 1000 + 500));

  stopWatch.start = Date.now();
  const stopped = await rpc("dictation.capture.stop", { captureId });
  const stopMs = Date.now() - stopWatch.start;
  const wavBytes = Buffer.from(stopped.audioB64, "base64").length;
  console.log(`capture.stop: ${stopMs}ms, ${wavBytes} wav bytes (${stopped.durationMs}ms audio)`);

  const tr = await rpc("dictation.speech.transcribe", {
    audioB64: stopped.audioB64,
    durationSec,
    delivery: "text_only",
  });
  const totalMs = Date.now() - stopWatch.start;
  console.log(`stop→transcript: ${totalMs}ms (engine durationMs=${tr.durationMs ?? "?"})`);
  console.log(`delivery: ${tr.delivery} (clipboard untouched)`);
  console.log(`transcript: ${tr.text}`);
} finally {
  // Never leave the capture session busy if we failed mid-run.
  await rpc("dictation.cancel").catch(() => {});
}

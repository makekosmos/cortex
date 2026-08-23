import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const cortexRoot = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const desktopRoot = path.join(cortexRoot, "desktop");
const require = createRequire(import.meta.url);

function makeSmokeWavBase64() {
  const sampleRate = 16000;
  const durationSec = 1.5;
  const samples = Math.floor(sampleRate * durationSec);
  const dataSize = samples * 2;
  const wav = Buffer.alloc(44 + dataSize);

  wav.write("RIFF", 0, "ascii");
  wav.writeUInt32LE(36 + dataSize, 4);
  wav.write("WAVEfmt ", 8, "ascii");
  wav.writeUInt32LE(16, 16);
  wav.writeUInt16LE(1, 20);
  wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(sampleRate, 24);
  wav.writeUInt32LE(sampleRate * 2, 28);
  wav.writeUInt16LE(2, 32);
  wav.writeUInt16LE(16, 34);
  wav.write("data", 36, "ascii");
  wav.writeUInt32LE(dataSize, 40);

  for (let i = 0; i < samples; i += 1) {
    const t = i / sampleRate;
    const envelope =
      (t < 0.12 ? 0 : 1) *
      (t < 0.42 ? (t - 0.12) / 0.3 : 1) *
      (t > 1.28 ? Math.max(0, (1.5 - t) / 0.22) : 1);
    const syllablePulse =
      (t >= 0.14 && t < 0.48 ? Math.sin(Math.PI * ((t - 0.14) / 0.34)) ** 2 : 0) +
      (t >= 0.62 && t < 1.0 ? Math.sin(Math.PI * ((t - 0.62) / 0.38)) ** 2 : 0) +
      (t >= 1.08 ? Math.sin(Math.PI * Math.min(1, (t - 1.08) / 0.28)) ** 2 : 0);
    const voiced = Math.sin(2 * Math.PI * 165 * t) * 0.52;
    const formant = Math.sin(2 * Math.PI * 245 * t + 0.3) * 0.28;
    const airy = Math.sin(2 * Math.PI * 420 * t + 1.2) * 0.12;
    const sample = Math.max(
      -1,
      Math.min(1, (voiced + formant + airy) * Math.max(0.35, syllablePulse) * envelope),
    );
    wav.writeInt16LE(Math.round(sample * 0x7fff), 44 + i * 2);
  }

  return wav.toString("base64");
}

function resolvePlaywrightCliPath() {
  try {
    return require.resolve("playwright/cli");
  } catch {
    const packageJsonPath = require.resolve("playwright/package.json");
    return path.join(path.dirname(packageJsonPath), "cli.js");
  }
}

function quoteForDisplay(value) {
  if (!/[\s"'`]/.test(value)) {
    return value;
  }
  return `"${value.replace(/"/g, '\\"')}"`;
}

const apiKey = process.env.KOSMOS_TEST_GROQ_API_KEY?.trim();

if (!apiKey) {
  console.error("Missing required env var: KOSMOS_TEST_GROQ_API_KEY");
  console.error("Set it and rerun:");
  console.error('  $env:KOSMOS_TEST_GROQ_API_KEY = "..."');
  process.exit(1);
}

const audioB64 = process.env.KOSMOS_TEST_DICTATION_AUDIO_B64?.trim() || makeSmokeWavBase64();
const usingSyntheticAudio = !process.env.KOSMOS_TEST_DICTATION_AUDIO_B64?.trim();
const expectedSubstring =
  process.env.KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING?.trim() || null;
const outputDir = mkdtempSync(path.join(os.tmpdir(), "kosmos-dictation-groq-smoke-"));
const playwrightCliPath = resolvePlaywrightCliPath();
const playwrightConfig = path.join(desktopRoot, "playwright.config.ts");
const dictationSpec = path.join(desktopRoot, "e2e/dictation.spec.ts");

if (!existsSync(playwrightConfig) || !existsSync(dictationSpec)) {
  console.error(`Cortex desktop test files are unavailable under ${desktopRoot}`);
  process.exit(1);
}

if (usingSyntheticAudio) {
  console.log("No KOSMOS_TEST_DICTATION_AUDIO_B64 provided; using a synthetic WAV fixture.");
  if (expectedSubstring) {
    console.log(
      "Transcript assertion remains enabled via KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING.",
    );
  } else {
    console.log(
      "Transcript assertion is skipped unless KOSMOS_TEST_DICTATION_EXPECTED_TRANSCRIPT_SUBSTRING is set.",
    );
  }
}

const playwrightArgs = [
  "test",
  "--config",
  playwrightConfig,
  "--reporter=line",
  "--output",
  outputDir,
  dictationSpec,
  "-g",
  "opt-in real provider path works headless without microphone",
];

console.log(`$ node ${quoteForDisplay(playwrightCliPath)} ${playwrightArgs.join(" ")}`);
console.log(`Playwright output dir: ${outputDir}`);

const result = spawnSync(process.execPath, [playwrightCliPath, ...playwrightArgs], {
  cwd: desktopRoot,
  env: {
    ...process.env,
    KOSMOS_TEST_GROQ_API_KEY: apiKey,
    KOSMOS_TEST_DICTATION_AUDIO_B64: audioB64,
  },
  stdio: "inherit",
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);

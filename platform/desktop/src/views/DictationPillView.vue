<script setup lang="ts">
// Dictation pill renderer — overlay (~320x64) с waveform и таймером во время
// записи. Аудио-захват через Web Audio API; PCM → WAV → base64 → backend
// `dictation.submit_audio`. См. spec
// `.agent/tasks/2026-05-24-dictation/spec.md`.
//
// Команды от main приходят через `window.kepler.dictation.onCommand`:
//   { kind: "start" }  → запуск getUserMedia + accumulation
//   { kind: "stop" }   → encode WAV + submit → pillFinished
//   { kind: "cancel" } → drop buffer + pillFinished

import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

type PillStatus = "idle" | "recording" | "transcribing" | "waiting" | "error";

const status = ref<PillStatus>("idle");
const errorText = ref<string>("");
/// Подпись под индикатором (для waiting / error). На recording / transcribing
/// pill самодостаточен (waveform / dots).
const subText = ref<string>("");
const elapsedSec = ref<number>(0);
const WAVE_BAR_COUNT = 24;
const WAVE_DOT_PX = 4;
const WAVE_HEIGHT_PX = 30;
const WAVE_PEAK = 0.82;
const RECORDING_PEAK = 1;
const levelBars = ref<number[]>(Array.from({ length: WAVE_BAR_COUNT }, () => 0));

// Audio capture lifecycle:
//   • `mediaStream` + `audioCtx` — warm-cache. Создаются при первой записи и
//     остаются открытыми после `stopAndSubmit` / `cancelCapture`. Это даёт
//     ~0ms latency на серии записей подряд (нет повторного getUserMedia
//     init'а на ~80-500ms).
//   • Через `STREAM_KEEP_ALIVE_MS` после последней сессии stream закрывается
//     (track.stop) → Windows mic indicator в трее гаснет, ресурсы
//     освобождаются. Следующий toggleDictation создаёт stream заново.
//   • `processor` / `analyser` / `source` / `pcmChunks` пересоздаются при
//     каждой сессии — они per-recording.
let mediaStream: MediaStream | null = null;
let audioCtx: AudioContext | null = null;
let analyser: AnalyserNode | null = null;
let processor: ScriptProcessorNode | null = null;
let source: MediaStreamAudioSourceNode | null = null;
let pcmChunks: Int16Array[] = [];
let timerHandle: ReturnType<typeof setInterval> | null = null;
let levelHandle: ReturnType<typeof setInterval> | null = null;
let unsubscribeCommand: (() => void) | null = null;
let recordStartMs = 0;
let streamShutdownTimer: ReturnType<typeof setTimeout> | null = null;

const TARGET_SAMPLE_RATE = 16000;
/** После этого окна тишины stream закрывается полностью (track.stop),
 *  Windows mic indicator гаснет. На следующий hotkey — ~80-500ms cold start. */
const STREAM_KEEP_ALIVE_MS = 30_000;

const hasDictationBridge = () => Boolean(window.kepler?.dictation);
const isPreview =
  new URLSearchParams(window.location.hash.split("?")[1] ?? "").get("preview") === "1" ||
  !hasDictationBridge();
const idleBars = Array.from({ length: WAVE_BAR_COUNT }, () => 0);
const previewBars = [
  0, 0, 0.08, 0.24, 0.44, 0.68, 0.38, 0.16, 0.52, 0.82, 0.46, 0.2, 0.34, 0.58, 0.28, 0.1, 0.5, 0.72,
  0.36, 0.18, 0.3, 0.48, 0.14, 0,
];
const transcribingWaveIndex = ref(0);
const transcribingBars = computed(() =>
  idleBars.map((base, i) => {
    const distance = Math.abs(i - transcribingWaveIndex.value);
    if (distance === 0) return WAVE_PEAK;
    if (distance === 1) return 0.48;
    if (distance === 2) return 0.22;
    return base;
  }),
);
const waitingWaveIndex = ref(0);
const waitingBars = computed(() =>
  idleBars.map((base, i) => {
    const distance = Math.abs(i - waitingWaveIndex.value);
    if (distance === 0) return WAVE_PEAK;
    if (distance === 1) return 0.48;
    if (distance === 2) return 0.22;
    return base;
  }),
);
const errorWaveIndex = ref(0);
const errorBars = computed(() =>
  idleBars.map((base, i) => {
    const left = Math.floor((WAVE_BAR_COUNT - 1) / 2) - errorWaveIndex.value;
    const right = Math.ceil((WAVE_BAR_COUNT - 1) / 2) + errorWaveIndex.value;
    const distance = Math.min(Math.abs(i - left), Math.abs(i - right));
    if (distance === 0) return WAVE_PEAK;
    if (distance === 1) return 0.36;
    return base;
  }),
);
let transcribingWaveDirection = 1;
let transcribingWaveHandle: ReturnType<typeof setInterval> | null = null;
let waitingWaveDirection = 1;
let waitingWaveHandle: ReturnType<typeof setInterval> | null = null;
let errorWaveDirection = 1;
let errorWaveHandle: ReturnType<typeof setInterval> | null = null;
const previewStates: {
  status: PillStatus;
  label: string;
  subText?: string;
  errorText?: string;
  bars?: number[];
}[] = [
  { status: "idle", label: "idle" },
  { status: "recording", label: "recording", bars: previewBars },
  { status: "transcribing", label: "transcribing" },
  { status: "waiting", label: "waiting", subText: "Жду сеть… (попытка 2)" },
  { status: "error", label: "error", errorText: "Сеть не вернулась" },
];

function startTranscribingWave(): void {
  if (transcribingWaveHandle) return;
  transcribingWaveHandle = setInterval(() => {
    const next = transcribingWaveIndex.value + transcribingWaveDirection;
    if (next >= idleBars.length - 1) {
      transcribingWaveIndex.value = idleBars.length - 1;
      transcribingWaveDirection = -1;
    } else if (next <= 0) {
      transcribingWaveIndex.value = 0;
      transcribingWaveDirection = 1;
    } else {
      transcribingWaveIndex.value = next;
    }
  }, 55);
}

function stopTranscribingWave(): void {
  if (transcribingWaveHandle) {
    clearInterval(transcribingWaveHandle);
    transcribingWaveHandle = null;
  }
  transcribingWaveIndex.value = 0;
  transcribingWaveDirection = 1;
}

function startWaitingWave(): void {
  if (waitingWaveHandle) return;
  waitingWaveHandle = setInterval(() => {
    const next = waitingWaveIndex.value + waitingWaveDirection;
    if (next >= idleBars.length - 1) {
      waitingWaveIndex.value = idleBars.length - 1;
      waitingWaveDirection = -1;
    } else if (next <= 0) {
      waitingWaveIndex.value = 0;
      waitingWaveDirection = 1;
    } else {
      waitingWaveIndex.value = next;
    }
  }, 55);
}

function stopWaitingWave(): void {
  if (waitingWaveHandle) {
    clearInterval(waitingWaveHandle);
    waitingWaveHandle = null;
  }
  waitingWaveIndex.value = 0;
  waitingWaveDirection = 1;
}

function startErrorWave(): void {
  if (errorWaveHandle) return;
  errorWaveHandle = setInterval(() => {
    const max = Math.floor((WAVE_BAR_COUNT - 1) / 2);
    const next = errorWaveIndex.value + errorWaveDirection;
    if (next >= max) {
      errorWaveIndex.value = max;
      errorWaveDirection = -1;
    } else if (next <= 0) {
      errorWaveIndex.value = 0;
      errorWaveDirection = 1;
    } else {
      errorWaveIndex.value = next;
    }
  }, 65);
}

function stopErrorWave(): void {
  if (errorWaveHandle) {
    clearInterval(errorWaveHandle);
    errorWaveHandle = null;
  }
  errorWaveIndex.value = 0;
  errorWaveDirection = 1;
}

function formatElapsed(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}

const elapsedLabel = computed(() => formatElapsed(elapsedSec.value));

function barHeight(value: number): string {
  const normalized = Math.min(WAVE_HEIGHT_PX, Math.max(WAVE_DOT_PX, value * WAVE_HEIGHT_PX));
  return `${normalized}px`;
}

function recordingBarHeight(value: number): string {
  return barHeight(Math.min(value, RECORDING_PEAK));
}

function barOpacity(value: number): string {
  return `${Math.min(1, 0.4 + Math.max(0, value) * 0.6)}`;
}

function handleCancelClick(event: Event): void {
  event.stopPropagation();
  if (isPreview) return;
  void cancelCapture();
}

function handleSubmitClick(event: Event): void {
  event.stopPropagation();
  if (isPreview) return;
  void stopAndSubmit();
}

function statusText(): string {
  switch (status.value) {
    case "recording":
      return "Слушаю…";
    case "transcribing":
      return "Распознаю…";
    case "waiting":
      return subText.value || "Жду сеть…";
    case "error":
      return errorText.value || "Ошибка";
    case "idle":
    default:
      return "";
  }
}

/** Создаёт MediaStream + AudioContext если их ещё нет (cold start), либо
 *  возвращает уже warm cache. На warm-пути — мгновенно (нет getUserMedia).
 *  Reset'ит scheduleStreamShutdown — пока юзер активно диктует, idle timer
 *  не должен закрывать stream под ногами. */
async function ensureStream(): Promise<MediaStream> {
  cancelStreamShutdown();
  if (mediaStream && mediaStream.active) return mediaStream;

  // Cold start: запрашиваем нужное устройство из config'а.
  let preferredDeviceId: string | null = null;
  try {
    const cfg = (await window.kepler.ark.request("dictation.get_config", {})) as {
      config?: { microphoneDeviceId?: string | null };
    };
    preferredDeviceId = cfg.config?.microphoneDeviceId?.trim() || null;
  } catch {
    /* ignore — поедем на default */
  }

  const baseConstraints: MediaTrackConstraints = {
    channelCount: 1,
    echoCancellation: true,
    noiseSuppression: true,
    autoGainControl: true,
  };

  if (preferredDeviceId) {
    try {
      mediaStream = await navigator.mediaDevices.getUserMedia({
        audio: { ...baseConstraints, deviceId: { exact: preferredDeviceId } },
      });
    } catch (e) {
      console.warn("[dictation-pill] preferred mic not available, falling back to default:", e);
      mediaStream = await navigator.mediaDevices.getUserMedia({ audio: baseConstraints });
    }
  } else {
    mediaStream = await navigator.mediaDevices.getUserMedia({ audio: baseConstraints });
  }
  return mediaStream;
}

function cancelStreamShutdown(): void {
  if (streamShutdownTimer) {
    clearTimeout(streamShutdownTimer);
    streamShutdownTimer = null;
  }
}

/** Запускает таймер закрытия stream'а. Вызывается после finalize сессии
 *  (submit / cancel / pillFinished). Если до истечения timer'а юзер
 *  запустит новую запись — `ensureStream` сбросит таймер и переиспользует
 *  warm-stream (0ms latency). Иначе через STREAM_KEEP_ALIVE_MS закрываем
 *  track'и и AudioContext — Windows mic indicator в трее гаснет. */
function scheduleStreamShutdown(): void {
  cancelStreamShutdown();
  streamShutdownTimer = setTimeout(() => {
    streamShutdownTimer = null;
    closeStream();
  }, STREAM_KEEP_ALIVE_MS);
}

/** Полностью закрывает warm-stream + AudioContext. После этого следующая
 *  сессия пойдёт по cold path через `ensureStream`. */
function closeStream(): void {
  if (audioCtx) {
    void audioCtx.close().catch(() => {
      /* ignore */
    });
    audioCtx = null;
  }
  if (mediaStream) {
    for (const t of mediaStream.getTracks()) {
      try {
        t.stop();
      } catch {
        /* ignore */
      }
    }
    mediaStream = null;
  }
}

async function startCapture(): Promise<void> {
  if (status.value === "recording") return;
  pcmChunks = [];
  errorText.value = "";
  elapsedSec.value = 0;

  let stream: MediaStream;
  try {
    stream = await ensureStream();
  } catch (e) {
    status.value = "error";
    errorText.value = "Нет доступа к микрофону";
    console.error("[dictation-pill] getUserMedia failed:", e);
    try {
      await window.kepler.ark.request("dictation.cancel", {});
    } catch {
      /* ignore */
    }
    void window.kepler.dictation.pillFinished();
    return;
  }

  // AudioContext тоже warm'ится — нет смысла close/open на каждую сессию.
  if (!audioCtx) {
    audioCtx = new AudioContext({ sampleRate: TARGET_SAMPLE_RATE });
  }

  source = audioCtx.createMediaStreamSource(stream);
  analyser = audioCtx.createAnalyser();
  analyser.fftSize = 256;
  analyser.smoothingTimeConstant = 0.85;
  source.connect(analyser);

  // ScriptProcessorNode deprecated, но работает без AudioWorklet boilerplate.
  // Для Phase 1 — приемлемо; в Phase 2 (если стрим в local whisper) перейдём
  // на AudioWorklet.
  processor = audioCtx.createScriptProcessor(2048, 1, 1);
  processor.onaudioprocess = (e: AudioProcessingEvent) => {
    const input = e.inputBuffer.getChannelData(0);
    const i16 = new Int16Array(input.length);
    for (let i = 0; i < input.length; i++) {
      const s = Math.max(-1, Math.min(1, input[i]));
      i16[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
    }
    pcmChunks.push(i16);
  };
  source.connect(processor);
  processor.connect(audioCtx.destination);

  status.value = "recording";
  recordStartMs = Date.now();
  timerHandle = setInterval(() => {
    elapsedSec.value = (Date.now() - recordStartMs) / 1000;
  }, 200);
  levelHandle = setInterval(() => {
    if (!analyser) return;
    const spectrum = new Uint8Array(analyser.frequencyBinCount);
    analyser.getByteFrequencyData(spectrum);
    const bars = Array.from({ length: WAVE_BAR_COUNT }, () => 0);
    const startFreq = Math.floor(spectrum.length * 0.05);
    const endFreq = Math.floor(spectrum.length * 0.4);
    const relevantData = spectrum.slice(startFreq, endFreq);
    const halfCount = Math.floor(WAVE_BAR_COUNT / 2);
    const sensitivity = 0.8;
    for (let i = halfCount - 1; i >= 0; i--) {
      const dataIndex = Math.floor((i / halfCount) * relevantData.length);
      const value = Math.min(1, ((relevantData[dataIndex] ?? 0) / 255) * sensitivity);
      bars[halfCount - 1 - i] = Math.max(0, value);
    }
    for (let i = 0; i < halfCount; i++) {
      const dataIndex = Math.floor((i / halfCount) * relevantData.length);
      const value = Math.min(1, ((relevantData[dataIndex] ?? 0) / 255) * sensitivity);
      bars[halfCount + i] = Math.max(0, value);
    }
    levelBars.value = bars;
  }, 80);
}

/** Останавливает per-session graph'а (processor / analyser / source / timers),
 *  но НЕ трогает mediaStream и audioCtx — они кэшируются для warm restart.
 *  Закрытием stream'а занимается `scheduleStreamShutdown`. */
function teardownCapture(): void {
  if (timerHandle) {
    clearInterval(timerHandle);
    timerHandle = null;
  }
  if (levelHandle) {
    clearInterval(levelHandle);
    levelHandle = null;
  }
  if (processor) {
    try {
      processor.disconnect();
    } catch {
      /* ignore */
    }
    processor.onaudioprocess = null;
    processor = null;
  }
  if (analyser) {
    try {
      analyser.disconnect();
    } catch {
      /* ignore */
    }
    analyser = null;
  }
  if (source) {
    try {
      source.disconnect();
    } catch {
      /* ignore */
    }
    source = null;
  }
  levelBars.value = Array.from({ length: WAVE_BAR_COUNT }, () => 0);
}

function concatPcm(chunks: Int16Array[]): Int16Array {
  let total = 0;
  for (const c of chunks) total += c.length;
  const out = new Int16Array(total);
  let off = 0;
  for (const c of chunks) {
    out.set(c, off);
    off += c.length;
  }
  return out;
}

function encodeWav(samples: Int16Array, sampleRate: number): Uint8Array {
  const numSamples = samples.length;
  const buf = new ArrayBuffer(44 + numSamples * 2);
  const view = new DataView(buf);
  const writeStr = (off: number, s: string) => {
    for (let i = 0; i < s.length; i++) view.setUint8(off + i, s.charCodeAt(i));
  };
  writeStr(0, "RIFF");
  view.setUint32(4, 36 + numSamples * 2, true);
  writeStr(8, "WAVE");
  writeStr(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeStr(36, "data");
  view.setUint32(40, numSamples * 2, true);
  let off = 44;
  for (let i = 0; i < numSamples; i++, off += 2) {
    view.setInt16(off, samples[i] ?? 0, true);
  }
  return new Uint8Array(buf);
}

function bytesToBase64(bytes: Uint8Array): string {
  // Chunk'аем чтобы не превышать stack call размер для String.fromCharCode.
  let bin = "";
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    const slice = bytes.subarray(i, Math.min(i + CHUNK, bytes.length));
    bin += String.fromCharCode(...slice);
  }
  return btoa(bin);
}

async function stopAndSubmit(): Promise<void> {
  if (status.value !== "recording") return;
  const sampleRate = audioCtx?.sampleRate ?? TARGET_SAMPLE_RATE;
  teardownCapture();
  if (pcmChunks.length === 0) {
    status.value = "idle";
    scheduleStreamShutdown();
    void window.kepler.dictation.pillFinished();
    return;
  }
  status.value = "transcribing";
  const pcm = concatPcm(pcmChunks);
  pcmChunks = [];
  const durationSec = pcm.length / sampleRate;
  const wav = encodeWav(pcm, sampleRate);
  const b64 = bytesToBase64(wav);
  let queuedUuid: string | null = null;
  try {
    const resp = (await window.kepler.ark.request("dictation.submit_audio", {
      audioB64: b64,
      durationSec,
    })) as {
      uuid?: string;
      state?: PillStatus | "pending";
      queued?: boolean;
      error?: string;
    };
    if (resp.state === "error") {
      // Fatal от backend (401/400/403/etc) — показываем user_msg, закроемся
      // с error mark. Pending всё ещё на диске — юзер увидит в Settings.
      status.value = "error";
      errorText.value = resp.error ?? "Не удалось распознать";
    } else if (resp.queued && resp.uuid) {
      // Первая попытка fail → backend запустил auto-retry в фоне.
      // Поллим очередь со спиннером "Жду сеть…".
      queuedUuid = resp.uuid;
      console.info("[dictation-pill] queued for background retry:", resp.uuid);
    } else {
      // Success path — text уже инжектнут, pill закрывается.
      status.value = "idle";
    }
  } catch (e) {
    status.value = "error";
    errorText.value = (e as Error)?.message ?? "Ошибка распознавания";
    console.error("[dictation-pill] submit_audio failed:", e);
  }
  scheduleStreamShutdown();

  if (queuedUuid) {
    // Висим со спиннером пока background retry работает. Backend расписание:
    // 1+5+10+20+40 sec = 76s sleeps + ~5×8s attempt window ≈ ~120s максимум.
    // Даём 130s timeout — чуть больше чем полный цикл backend.
    await waitForQueueResolve(queuedUuid, 130_000);
    setTimeout(
      () => void window.kepler.dictation.pillFinished(),
      status.value === "error" ? 4500 : 80,
    );
  } else {
    setTimeout(
      () => void window.kepler.dictation.pillFinished(),
      status.value === "error" ? 4500 : 80,
    );
  }
}

/// Поллит `dictation.list_pending` пока наш uuid в очереди, или истекает
/// timeoutMs. Меняет статус pill на 'waiting' (спиннер + subText). На исходе
/// либо переходим в idle (success — item исчез), либо в error (timeout).
async function waitForQueueResolve(uuid: string, timeoutMs: number): Promise<void> {
  const startMs = Date.now();
  status.value = "waiting";
  subText.value = "Жду сеть…";
  while (Date.now() - startMs < timeoutMs) {
    try {
      const resp = (await window.kepler.ark.request("dictation.list_pending", {})) as {
        items?: { uuid: string; attempts: number }[];
      };
      const item = (resp.items ?? []).find((i) => i.uuid === uuid);
      if (!item) {
        // Item исчез → backend сделал success+drop. Закрываемся тихо.
        status.value = "idle";
        return;
      }
      // Обновляем подпись с числом попыток для feedback'а.
      subText.value = item.attempts > 0 ? `Жду сеть… (попытка ${item.attempts})` : "Жду сеть…";
    } catch (e) {
      console.warn("[dictation-pill] poll list_pending failed:", e);
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
  // Timeout — auto-retry скорее всего исчерпан. Pill закрываем с error mark;
  // pending остаётся в Settings → Очередь для ручного retry.
  status.value = "error";
  errorText.value = "Сеть не вернулась — открой Settings → Диктация → Очередь";
}

async function cancelCapture(): Promise<void> {
  teardownCapture();
  pcmChunks = [];
  status.value = "idle";
  try {
    await window.kepler.ark.request("dictation.cancel", {});
  } catch {
    /* ignore */
  }
  scheduleStreamShutdown();
  void window.kepler.dictation.pillFinished();
}

function handleKeydown(e: KeyboardEvent): void {
  // Esc — cancel. Note: pill focusable: false на main-стороне, так что
  // keydown сюда не дойдёт. Слушаем все равно для случая когда юзер
  // явно кликнет в pill (focus всё же придёт).
  if (e.key === "Escape") {
    void cancelCapture();
  }
}

onMounted(() => {
  if (isPreview) {
    startTranscribingWave();
    startWaitingWave();
    startErrorWave();
    return;
  }
  unsubscribeCommand = window.kepler.dictation.onCommand((cmd) => {
    if (cmd.kind === "start") {
      void startCapture();
    } else if (cmd.kind === "stop") {
      void stopAndSubmit();
    } else if (cmd.kind === "cancel") {
      void cancelCapture();
    }
  });
  window.addEventListener("keydown", handleKeydown);
});

const stopStatusWatch = watch(
  status,
  (value) => {
    if (isPreview) return;
    if (value === "transcribing") {
      startTranscribingWave();
    } else {
      stopTranscribingWave();
    }
    if (value === "waiting") {
      startWaitingWave();
    } else {
      stopWaitingWave();
    }
    if (value === "error") {
      startErrorWave();
    } else {
      stopErrorWave();
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  unsubscribeCommand?.();
  stopStatusWatch();
  stopTranscribingWave();
  stopWaitingWave();
  stopErrorWave();
  window.removeEventListener("keydown", handleKeydown);
  teardownCapture();
  // Окно демонтируется (Kepler закрывают) — hard-close без grace-периода.
  cancelStreamShutdown();
  closeStream();
});

// expose to template
const exposeStatusText = computed(() => statusText());
</script>

<template>
  <div v-if="isPreview" class="preview-page">
    <div class="preview-list">
      <div v-for="item in previewStates" :key="item.status" class="preview-item">
        <div class="preview-label">{{ item.label }}</div>
        <div class="preview-frame">
          <div class="pill" :class="`status-${item.status}`">
            <div v-if="item.status === 'recording'" class="waveform">
              <span
                v-for="(b, i) in item.bars ?? []"
                :key="i"
                class="wave-bar"
                :style="{
                  height: recordingBarHeight(b),
                  opacity: barOpacity(b),
                }"
              />
            </div>
            <div v-else-if="item.status === 'transcribing'" class="waveform transcribing-wave">
              <span
                v-for="(b, i) in transcribingBars"
                :key="i"
                class="wave-bar"
                :style="{
                  height: barHeight(b),
                  opacity: barOpacity(b),
                }"
              />
            </div>
            <div
              v-else-if="item.status === 'waiting'"
              class="waveform waiting-wave"
              :title="item.subText"
            >
              <span
                v-for="(b, i) in waitingBars"
                :key="i"
                class="wave-bar"
                :style="{ height: barHeight(b), opacity: barOpacity(b) }"
              />
            </div>
            <div
              v-else-if="item.status === 'error'"
              class="waveform error-wave"
              :title="item.errorText"
            >
              <span
                v-for="(b, i) in errorBars"
                :key="i"
                class="wave-bar"
                :style="{ height: barHeight(b), opacity: barOpacity(b) }"
              />
            </div>
            <div v-else class="waveform idle-wave">
              <span
                v-for="(b, i) in idleBars"
                :key="i"
                class="wave-bar"
                :style="{ height: barHeight(b), opacity: barOpacity(b) }"
              />
            </div>
            <div v-if="item.status === 'recording'" class="pill-hover-controls">
              <button
                class="pill-action"
                type="button"
                title="Отменить"
                @pointerdown.stop.prevent="handleCancelClick"
              >
                ×
              </button>
              <span class="pill-time">{{ formatElapsed(3) }}</span>
              <button
                class="pill-action"
                type="button"
                title="Отправить"
                @pointerdown.stop.prevent="handleSubmitClick"
              >
                ✓
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
  <div v-else class="stage">
    <div class="pill" :class="`status-${status}`" :title="exposeStatusText">
      <!-- Waveform внутри пилюли: вертикальные бары разной высоты, по центру. -->
      <div v-if="status === 'recording'" class="waveform">
        <span
          v-for="(b, i) in levelBars"
          :key="i"
          class="wave-bar"
          :style="{
            height: recordingBarHeight(b),
            opacity: barOpacity(b),
          }"
        />
      </div>
      <div v-else-if="status === 'transcribing'" class="waveform transcribing-wave">
        <span
          v-for="(b, i) in transcribingBars"
          :key="i"
          class="wave-bar"
          :style="{
            height: barHeight(b),
            opacity: barOpacity(b),
          }"
        />
      </div>
      <div v-else-if="status === 'waiting'" class="waveform waiting-wave" :title="subText">
        <span
          v-for="(b, i) in waitingBars"
          :key="i"
          class="wave-bar"
          :style="{ height: barHeight(b), opacity: barOpacity(b) }"
        />
      </div>
      <div v-else-if="status === 'error'" class="waveform error-wave" :title="errorText">
        <span
          v-for="(b, i) in errorBars"
          :key="i"
          class="wave-bar"
          :style="{ height: barHeight(b), opacity: barOpacity(b) }"
        />
      </div>
      <div v-else class="waveform idle-wave">
        <span
          v-for="(b, i) in idleBars"
          :key="i"
          class="wave-bar"
          :style="{ height: barHeight(b), opacity: barOpacity(b) }"
        />
      </div>
      <div v-if="status === 'recording'" class="pill-hover-controls">
        <button
          class="pill-action"
          type="button"
          title="Отменить"
          @pointerdown.stop.prevent="handleCancelClick"
        >
          ×
        </button>
        <span class="pill-time">{{ elapsedLabel }}</span>
        <button
          class="pill-action"
          type="button"
          title="Отправить"
          @pointerdown.stop.prevent="handleSubmitClick"
        >
          ✓
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
:global(html),
:global(body),
:global(#app) {
  background: transparent !important;
}

.preview-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px;
  background: var(--background);
  color: var(--foreground);
}

.preview-list {
  width: min(760px, 100%);
  display: grid;
  gap: 14px;
}

.preview-item {
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr);
  align-items: center;
  gap: 16px;
}

.preview-label {
  font-family: var(--font-sans, -apple-system, sans-serif);
  font-size: 12px;
  color: color-mix(in srgb, var(--foreground) 60%, transparent);
  text-transform: uppercase;
}

.preview-frame {
  min-height: 92px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  border-radius: 12px;
  background:
    linear-gradient(
      90deg,
      color-mix(in srgb, var(--foreground) 4%, transparent) 1px,
      transparent 1px
    ),
    linear-gradient(
      180deg,
      color-mix(in srgb, var(--foreground) 4%, transparent) 1px,
      transparent 1px
    ),
    color-mix(in srgb, var(--background) 88%, var(--surface) 12%);
  background-size: 16px 16px;
}

.preview-frame .pill {
  width: 248px;
  height: 48px;
}

.stage {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  /* Прозрачное окно — сам stage не должен ловить клики мимо пилюли. */
  pointer-events: none;
  background: transparent;
}

.pill {
  /* Заполняем родителя целиком (как `.widget` у focus-widget). В overlay
   * родитель — `.stage` (100vw/100vh = окно), окно само размером с пилюлю
   * (см. dictation-pill.ts). Fill гарантирует, что прозрачной области вокруг
   * пилюли нет в принципе — значит Win32 нечего композитить белым. На preview
   * `.preview-frame .pill` переопределяет размер на 120×36. */
  position: relative;
  pointer-events: auto;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  /* Глянцевый чёрный — тонкий светлый highlight сверху, тёмный низ. */
  background: color-mix(in srgb, var(--surface) 58%, var(--background) 42%);
  border-radius: 8px;
  border: 2px solid color-mix(in srgb, var(--foreground) 18%, transparent);
  /* No outer shadow: the overlay window is pill-sized, so shadows create a visible composited square. */
  box-shadow:
    inset 0 1px 0 color-mix(in srgb, var(--foreground) 8%, transparent),
    inset 0 -1px 0 color-mix(in srgb, var(--background) 70%, transparent);
  /* `-webkit-app-region: drag` УБРАН: BrowserWindow создаётся с
   * `movable: false`, так что drag всё равно ничего не делает. Но
   * `app-region: drag` на parent блокирует click events для всех
   * детей (Electron на Windows глючит с nested `no-drag`),
   * из-за чего retry-кнопка на error state не нажималась. */
  user-select: none;
  overflow: hidden;
  transition: transform 220ms cubic-bezier(0.2, 0.7, 0.2, 1.4);
  animation: pill-in 260ms cubic-bezier(0.2, 0.7, 0.2, 1.4);
}

@keyframes pill-in {
  /* No translateY: window is now exactly pill-sized, so a vertical slide would
   * be clipped at the window edge. Pure fade + inward scale stays in bounds. */
  from {
    transform: scale(0.96);
    opacity: 0;
  }
  to {
    transform: scale(1);
    opacity: 1;
  }
}

.waveform {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
  width: 100%;
  height: 36px;
  padding: 0 8px;
  transition: opacity 120ms linear;
  overflow: hidden;
  mask-image: linear-gradient(
    90deg,
    transparent 0,
    #000 32px,
    #000 calc(100% - 32px),
    transparent 100%
  );
}

.wave-bar {
  display: block;
  width: 5px;
  min-width: 5px;
  background: #71717a;
  border-radius: 8px;
  opacity: 0.4;
  transition:
    height 80ms linear,
    background 150ms linear,
    opacity 150ms linear;
}

.idle-wave .wave-bar,
.transcribing-wave .wave-bar {
  background: color-mix(in srgb, var(--foreground) 92%, transparent);
  opacity: 0.95;
}

.waiting-wave .wave-bar {
  background: #f5a524;
  opacity: 0.95;
}

.error-wave .wave-bar {
  background: #ff453a;
  opacity: 0.95;
}

.pill-hover-controls {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: grid;
  grid-template-columns: 34px 1fr 34px;
  align-items: center;
  gap: 4px;
  padding: 4px;
  opacity: 0;
  pointer-events: none;
  transition: opacity 120ms linear;
}

.pill.status-recording:hover .waveform {
  opacity: 0;
}

.pill.status-recording:hover .pill-hover-controls {
  opacity: 1;
  pointer-events: auto;
}

.pill-action {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
  color: color-mix(in srgb, var(--foreground) 96%, transparent);
  font-family: var(--font-sans, -apple-system, sans-serif);
  font-size: 0;
  line-height: 1;
  cursor: default;
  transition:
    background 120ms linear,
    transform 120ms cubic-bezier(0.2, 0.7, 0.2, 1.2);
}

.pill-action::before {
  font-size: 18px;
  font-weight: 600;
}

.pill-action:first-child::before {
  content: "×";
}

.pill-action:last-child::before {
  content: "✓";
}

.pill-action:hover {
  background: color-mix(in srgb, var(--foreground) 18%, transparent);
}

.pill-action:active {
  transform: scale(0.98);
}

.pill-action:first-child {
  justify-self: start;
}

.pill-action:last-child {
  justify-self: end;
}

.pill-time {
  min-width: 0;
  text-align: center;
  font-family: var(--font-sans, -apple-system, sans-serif);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  color: color-mix(in srgb, var(--foreground) 96%, transparent);
  line-height: 1;
}
</style>

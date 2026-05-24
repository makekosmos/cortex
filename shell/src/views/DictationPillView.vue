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

import { computed, onBeforeUnmount, onMounted, ref } from "vue";

type PillStatus = "idle" | "recording" | "transcribing" | "error";

const status = ref<PillStatus>("idle");
const errorText = ref<string>("");
const elapsedSec = ref<number>(0);
const levelBars = ref<number[]>(Array.from({ length: 12 }, () => 0));

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

function statusText(): string {
  switch (status.value) {
    case "recording":
      return "Слушаю…";
    case "transcribing":
      return "Распознаю…";
    case "error":
      return errorText.value || "Ошибка";
    case "idle":
    default:
      return "";
  }
}

const timeText = computed(() => {
  const total = Math.max(0, Math.floor(elapsedSec.value));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
});

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
  analyser.fftSize = 64;
  analyser.smoothingTimeConstant = 0.5;
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
    const buf = new Uint8Array(analyser.frequencyBinCount);
    analyser.getByteFrequencyData(buf);
    const bars: number[] = [];
    const step = Math.max(1, Math.floor(buf.length / levelBars.value.length));
    for (let i = 0; i < levelBars.value.length; i++) {
      const v = buf[i * step] ?? 0;
      bars.push(v / 255);
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
  levelBars.value = Array.from({ length: 12 }, () => 0);
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
  try {
    await window.kepler.ark.request("dictation.submit_audio", {
      audioB64: b64,
      durationSec,
    });
    status.value = "idle";
  } catch (e) {
    status.value = "error";
    errorText.value = (e as Error)?.message ?? "Ошибка распознавания";
    console.error("[dictation-pill] submit_audio failed:", e);
  }
  // В обоих случаях (success/error) закрываем pill — статус ошибки
  // показывается через 800ms задержку для UX.
  scheduleStreamShutdown();
  setTimeout(
    () => {
      void window.kepler.dictation.pillFinished();
    },
    status.value === "error" ? 1200 : 80,
  );
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

onBeforeUnmount(() => {
  unsubscribeCommand?.();
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
  <div class="stage">
    <div class="pill" :class="`status-${status}`" :title="exposeStatusText">
      <!-- Waveform внутри пилюли: вертикальные бары разной высоты, по центру. -->
      <div v-if="status === 'recording'" class="waveform">
        <span
          v-for="(b, i) in levelBars"
          :key="i"
          class="wave-bar"
          :style="{
            height: `${Math.max(3, b * 32)}px`,
            opacity: 0.55 + b * 0.45,
          }"
        />
      </div>
      <div v-else-if="status === 'transcribing'" class="dots">
        <span class="dot" />
        <span class="dot" />
        <span class="dot" />
      </div>
      <div v-else-if="status === 'error'" class="idle-mark error-mark">!</div>
      <div v-else class="idle-mark"></div>
    </div>
  </div>
</template>

<style scoped>
.stage {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  /* Прозрачное окно — сам stage не должен ловить клики мимо пилюли. */
  pointer-events: none;
  background: transparent;
}

.pill {
  pointer-events: auto;
  width: 200px;
  height: 56px;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 20px;
  /* Глянцевый чёрный — тонкий светлый highlight сверху, тёмный низ. */
  background: linear-gradient(180deg, #2a2a2c 0%, #141416 55%, #0a0a0b 100%);
  border-radius: 999px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  /* Без drop-shadow по запросу — оставляем только тонкий inset bevel внутри. */
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.08),
    inset 0 -1px 0 rgba(0, 0, 0, 0.55);
  -webkit-app-region: drag;
  user-select: none;
  transition: transform 220ms cubic-bezier(0.2, 0.7, 0.2, 1.4);
  animation: pill-in 260ms cubic-bezier(0.2, 0.7, 0.2, 1.4);
}

@keyframes pill-in {
  from {
    transform: translateY(12px) scale(0.96);
    opacity: 0;
  }
  to {
    transform: translateY(0) scale(1);
    opacity: 1;
  }
}

.waveform {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  width: 100%;
  height: 32px;
}

.wave-bar {
  display: block;
  width: 2px;
  min-height: 3px;
  background: #f5f5f7;
  border-radius: 2px;
  transition:
    height 70ms linear,
    opacity 120ms linear;
}

/* Idle (зашли в pill но запись ещё не пошла) — тонкая горизонтальная линия. */
.idle-mark {
  width: 36px;
  height: 2px;
  background: rgba(245, 245, 247, 0.35);
  border-radius: 2px;
}

.error-mark {
  width: auto;
  height: auto;
  background: transparent;
  font-family: var(--font-sans, -apple-system, sans-serif);
  font-size: 18px;
  font-weight: 700;
  color: #f5a524;
}

.dots {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 32px;
}

.dots .dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #f5f5f7;
  animation: dot-bounce 1s infinite;
}

.dots .dot:nth-child(2) {
  animation-delay: 0.15s;
}

.dots .dot:nth-child(3) {
  animation-delay: 0.3s;
}

@keyframes dot-bounce {
  0%,
  100% {
    transform: translateY(0);
    opacity: 0.4;
  }
  50% {
    transform: translateY(-4px);
    opacity: 1;
  }
}
</style>

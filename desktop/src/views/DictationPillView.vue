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

import { KbdKey } from "@kosmos/visuals";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  buildDictationVoiceModelValue,
  normalizeDictationConfig,
  resolveAvailableDictationVoiceModelValue,
  type DictationLocalModelsSnapshot,
  type DictationConfigData,
} from "./dictation-model-selection";
import { isString, type JsonRecord } from "../shared/runtimeGuards";
import {
  drawWaveformCanvas,
  idleBars,
  WAVEFORM_SENSITIVITY,
  WAVE_BAR_COUNT,
  type PillStatus,
} from "./dictation-pill-waveform";

const status = ref<PillStatus>("idle");
const errorText = ref<string>("");
/// Подпись под индикатором (для waiting / error). На recording / transcribing
/// pill самодостаточен (waveform / dots).
const subText = ref<string>("");
const elapsedSec = ref<number>(0);
const DEFAULT_DICTATION_HOTKEY = "Ctrl+Shift+;";
const dictationHotkey = ref<string>(DEFAULT_DICTATION_HOTKEY);
const injectMode = ref<"auto_paste" | "clipboard_only">("auto_paste");
type TranscriptDelivery = "pasted" | "clipboard_only" | "clipboard_fallback" | "failed";
const delivery = ref<TranscriptDelivery | null>(null);
const deliveryReason = ref<string>("");
const backgroundDelivery = ref(false);
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
let startInFlight = false;
let stopAfterStart = false;
let captureGeneration = 0;
/** Reentrancy guard для stopAndSubmit — пока идёт grace-wait/submit, повторный
 *  stop не должен запустить второй submit того же аудио (double-submit). */
let stopInFlight = false;
let pcmSampleCount = 0;
let queuedUuid: string | null = null;
let expectedTranscriptUuid: string | null = null;
let unsubscribeArkEvent: (() => void) | null = null;

const TARGET_SAMPLE_RATE = 16000;
/** После этого окна тишины stream закрывается полностью (track.stop),
 *  Windows mic indicator гаснет. На следующий hotkey — ~80-500ms cold start. */
const STREAM_KEEP_ALIVE_MS = 30_000;
/** Короткая фраза: ScriptProcessor буфер (2048 @16kHz ≈ 128ms) может ещё не
 *  успеть отдать первый onaudioprocess к моменту stop'а. Ждём первые PCM-кадры
 *  не дольше этого окна, прежде чем счесть запись пустой — иначе быстрая
 *  диктовка молча теряется. */
const PCM_FIRST_FRAME_GRACE_MS = 350;

const hasDictationBridge = () => Boolean(window.kepler?.dictation);
const isPreview =
  new URLSearchParams(window.location.hash.split("?")[1] ?? "").get("preview") === "1" ||
  !hasDictationBridge();
const previewBars = Array.from({ length: WAVE_BAR_COUNT }, (_, i) => {
  const centered = (i - WAVE_BAR_COUNT / 2) / (WAVE_BAR_COUNT / 2);
  const voiceAmplitude = 0.34 + Math.sin(i * 0.39) * 0.2 + Math.cos(i * 0.17) * 0.12;
  const centerWeight = 1 - Math.abs(centered) * 0.34;
  return Math.max(0.02, Math.min(0.82, voiceAmplitude * centerWeight));
});
const processingBars = ref<number[]>(idleBars);
let processingAnimationFrame: number | null = null;
let processingTime = 0;
let processingTransitionProgress = 0;
let lastActiveBars = idleBars;
const previewStates: {
  status: PillStatus;
  label: string;
  subText?: string;
  errorText?: string;
}[] = [
  { status: "idle", label: "idle" },
  { status: "recording", label: "recording" },
  { status: "transcribing", label: "transcribing" },
  { status: "waiting", label: "waiting", subText: "Жду сеть… (попытка 2)" },
  { status: "error", label: "error", errorText: "Сеть не вернулась" },
];

function nextProcessingBars(): number[] {
  const halfCount = Math.floor(WAVE_BAR_COUNT / 2);
  return Array.from({ length: WAVE_BAR_COUNT }, (_, i) => {
    const normalizedPosition = (i - halfCount) / halfCount;
    const centerWeight = 1 - Math.abs(normalizedPosition) * 0.4;
    const wave1 = Math.sin(processingTime * 1.5 + normalizedPosition * 3) * 0.25;
    const wave2 = Math.sin(processingTime * 0.8 - normalizedPosition * 2) * 0.2;
    const wave3 = Math.cos(processingTime * 2 + normalizedPosition) * 0.15;
    const processingValue = (0.2 + wave1 + wave2 + wave3) * centerWeight;
    const lastValue = lastActiveBars[i] ?? 0;
    const finalValue =
      lastValue * (1 - processingTransitionProgress) +
      processingValue * processingTransitionProgress;
    return Math.max(0.05, Math.min(1, finalValue));
  });
}

function startProcessingWave(): void {
  if (processingAnimationFrame !== null) return;
  processingTransitionProgress = 0;
  const animate = () => {
    processingTime += 0.03;
    processingTransitionProgress = Math.min(1, processingTransitionProgress + 0.02);
    processingBars.value = nextProcessingBars();
    requestWaveformDraw();
    processingAnimationFrame = requestAnimationFrame(animate);
  };
  animate();
}

function stopProcessingWave(): void {
  if (processingAnimationFrame !== null) {
    cancelAnimationFrame(processingAnimationFrame);
    processingAnimationFrame = null;
  }
  processingBars.value = idleBars;
  processingTransitionProgress = 0;
  requestWaveformDraw();
}
function formatElapsed(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}
const dictationHotkeyParts = computed(() => splitHotkey(dictationHotkey.value));

function hotkeyPartLabel(value: string): string {
  return value.toLowerCase() === "shift" ? "⇧" : value;
}

function splitHotkey(value: string): string[] {
  return value
    .split("+")
    .map((part) => part.trim())
    .filter(Boolean);
}

type DeliveryPayload = {
  delivery?: unknown;
  deliveryReason?: unknown;
  injected?: unknown;
  uuid?: unknown;
};

const bufferedDeliveryEvents = new Map<string, DeliveryPayload>();

function deliveryFromPayload(
  payload: DeliveryPayload,
  isBackground = false,
): TranscriptDelivery | null {
  if (
    payload.delivery === "pasted" ||
    payload.delivery === "clipboard_only" ||
    payload.delivery === "clipboard_fallback" ||
    payload.delivery === "failed"
  ) {
    return payload.delivery;
  }
  if (payload.injected === true) return "pasted";
  if (payload.injected === false) {
    return isBackground || injectMode.value === "clipboard_only"
      ? "clipboard_only"
      : "clipboard_fallback";
  }
  return null;
}

function deliveryLabel(
  nextDelivery: TranscriptDelivery,
  isBackground = backgroundDelivery.value,
): string {
  switch (nextDelivery) {
    case "pasted":
      return "Вставлено";
    case "clipboard_only":
      return isBackground ? "Буфер обмена — фоновый повтор" : "Буфер обмена — вставьте вручную";
    case "clipboard_fallback":
      return "Вставка не удалась — текст в буфере";
    case "failed":
      return "Не доставлено";
  }
}

function applyDelivery(payload: DeliveryPayload, isBackground = false): boolean {
  const nextDelivery = deliveryFromPayload(payload, isBackground);
  if (!nextDelivery) return false;

  delivery.value = nextDelivery;
  backgroundDelivery.value = isBackground;
  deliveryReason.value = isString(payload.deliveryReason) ? payload.deliveryReason.trim() : "";
  if (nextDelivery === "pasted" || nextDelivery === "clipboard_only") {
    status.value = "idle";
    errorText.value = "";
    subText.value =
      nextDelivery === "clipboard_only"
        ? deliveryReason.value || "Текст в буфере обмена — вставьте вручную"
        : "";
  } else if (nextDelivery === "clipboard_fallback") {
    status.value = "error";
    errorText.value = deliveryReason.value
      ? `Вставка не удалась: ${deliveryReason.value}. Текст в буфере обмена.`
      : "Вставка не удалась — текст в буфере обмена.";
    subText.value = "";
  } else {
    status.value = "error";
    errorText.value = deliveryReason.value || "Не удалось доставить текст";
    subText.value = "";
  }
  return true;
}

function handleTranscriptEvent(event: DeliveryPayload & { event?: unknown }): void {
  if (event.event !== "dictation_transcript") return;
  const uuid = isString(event.uuid) ? event.uuid : null;
  if (!uuid) return;
  if (!expectedTranscriptUuid && !queuedUuid) {
    bufferedDeliveryEvents.set(uuid, event);
    if (bufferedDeliveryEvents.size > 16) {
      const oldest = bufferedDeliveryEvents.keys().next().value;
      if (isString(oldest)) bufferedDeliveryEvents.delete(oldest);
    }
    return;
  }
  if (expectedTranscriptUuid && uuid !== expectedTranscriptUuid) return;
  if (queuedUuid && uuid !== queuedUuid) return;
  const isBackground = Boolean(queuedUuid && uuid === queuedUuid);
  if (applyDelivery(event, isBackground)) expectedTranscriptUuid = uuid;
}

function deliveryFinishDelay(): number {
  switch (delivery.value) {
    case "pasted":
      return 80;
    case "clipboard_only":
      return 2500;
    case "clipboard_fallback":
    case "failed":
      return 4500;
    default:
      return status.value === "error" ? 4500 : 80;
  }
}

async function loadDictationConfig(): Promise<void> {
  try {
    // SAFETY: the surrounding domain validation preserves the asserted contract.
    const cfg = (await window.kepler.ark.request("dictation.get_config", {})) as {
      config?: { hotkey?: string | null; injectMode?: string | null };
    };
    const hotkey = cfg.config?.hotkey?.trim();
    if (hotkey) dictationHotkey.value = hotkey;
    if (cfg.config?.injectMode === "clipboard_only") injectMode.value = "clipboard_only";
    else if (cfg.config?.injectMode === "auto_paste") injectMode.value = "auto_paste";
  } catch {
    /* keep last known backend value */
  }
}
async function ensureReadyDictationModel(): Promise<boolean> {
  try {
    // SAFETY: the surrounding domain validation preserves the asserted contract.
    const cfgResp = (await window.kepler.ark.request("dictation.get_config", {})) as {
      config?: JsonRecord;
      hasApiKey?: boolean;
    };
    // SAFETY: the surrounding domain validation preserves the asserted contract.
    const localModels = (await window.kepler.ark.request(
      "dictation.list_local_models",
      {},
    )) as DictationLocalModelsSnapshot;
    // SAFETY: the surrounding domain validation preserves the asserted contract.
    const config = normalizeDictationConfig(
      cfgResp.config as Partial<DictationConfigData> | undefined,
    );
    const current = buildDictationVoiceModelValue(config);
    const resolved = resolveAvailableDictationVoiceModelValue(config, {
      hasApiKey: cfgResp.hasApiKey ?? false,
      localModels,
    });
    if (!resolved) {
      status.value = "error";
      errorText.value = "Нет доступной модели. Откройте Settings → AI.";
      setTimeout(() => void window.kepler.dictation.pillFinished(), 4500);
      return false;
    }
    if (resolved === current) return true;
    const [source, modelId] = resolved.split(":", 2);
    if (source === "local") {
      await window.kepler.ark.request("dictation.use_local_model", { modelId });
    } else {
      await window.kepler.ark.request("dictation.update_config", {
        provider: "groq",
        providerEnabled: true,
        model: modelId,
      });
    }
    return true;
  } catch (e) {
    status.value = "error";
    errorText.value = "Не удалось подготовить модель";
    console.error("[dictation-pill] ensureReadyDictationModel failed:", e);
    setTimeout(() => void window.kepler.dictation.pillFinished(), 4500);
    return false;
  }
}

const waveformCanvases = new Map<string, HTMLCanvasElement>();
let waveformFrame: number | null = null;
let waveformResizeObserver: ResizeObserver | null = null;

function waveformBarsForStatus(nextStatus: PillStatus, preview = false): number[] {
  if (nextStatus === "recording") return preview ? previewBars : levelBars.value;
  if (nextStatus === "transcribing" || nextStatus === "waiting" || nextStatus === "error") {
    return processingBars.value;
  }
  return idleBars;
}

function drawWaveforms(): void {
  waveformFrame = null;
  for (const [key, canvas] of waveformCanvases) {
    const isPreviewCanvas = key.startsWith("preview-");
    // SAFETY: preview keys are generated exclusively from the PillStatus values.
    const nextStatus = isPreviewCanvas
      ? (key.slice("preview-".length) as PillStatus)
      : status.value;
    drawWaveformCanvas(canvas, waveformBarsForStatus(nextStatus, isPreviewCanvas), nextStatus);
  }
}

function requestWaveformDraw(): void {
  if (waveformFrame !== null) return;
  waveformFrame = requestAnimationFrame(drawWaveforms);
}

function setWaveformCanvas(key: string, el: Element | null): void {
  if (!waveformResizeObserver) {
    waveformResizeObserver = new ResizeObserver(() => requestWaveformDraw());
  }
  if (el instanceof HTMLCanvasElement) {
    waveformCanvases.set(key, el);
    waveformResizeObserver.observe(el);
  } else {
    waveformCanvases.delete(key);
  }
  requestWaveformDraw();
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
  if (delivery.value) {
    const label = deliveryLabel(delivery.value);
    return deliveryReason.value ? `${label}: ${deliveryReason.value}` : label;
  }
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

function footerLabel(
  nextStatus: PillStatus,
  nextDelivery: TranscriptDelivery | null = null,
): string {
  if (nextDelivery) return deliveryLabel(nextDelivery);
  switch (nextStatus) {
    case "recording":
      return "Идёт запись";
    case "transcribing":
      return "Распознаю";
    case "waiting":
      return "Жду сеть";
    case "error":
      return "Ошибка";
    case "idle":
    default:
      return "Модель готова";
  }
}

function canCancel(nextStatus: PillStatus): boolean {
  return nextStatus !== "error";
}

function canSubmit(nextStatus: PillStatus): boolean {
  return nextStatus === "recording";
}

/** Создаёт MediaStream + AudioContext если их ещё нет (cold start), либо
 *  возвращает уже warm cache. На warm-пути — мгновенно (нет getUserMedia).
 *  Reset'ит scheduleStreamShutdown — пока юзер активно диктует, idle timer
 *  не должен закрывать stream под ногами. */
async function ensureStream(): Promise<MediaStream> {
  cancelStreamShutdown();
  if (mediaStream && mediaStream.active) {
    console.debug("[dictation-pill] microphone stream reused", {
      tracks: mediaStream.getAudioTracks().length,
    });
    return mediaStream;
  }

  // Cold start: запрашиваем нужное устройство из config'а.
  let preferredDeviceId: string | null = null;
  try {
    // SAFETY: the surrounding domain validation preserves the asserted contract.
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
      mediaStream = await navigator.mediaDevices.getUserMedia({
        audio: baseConstraints,
      });
    }
  } else {
    mediaStream = await navigator.mediaDevices.getUserMedia({
      audio: baseConstraints,
    });
  }
  const track = mediaStream.getAudioTracks()[0];
  console.info("[dictation-pill] microphone stream ready", {
    tracks: mediaStream.getAudioTracks().length,
    deviceIdAvailable: Boolean(track?.getSettings().deviceId),
    sampleRate: track?.getSettings().sampleRate ?? null,
    channelCount: track?.getSettings().channelCount ?? null,
  });
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
  if (status.value === "recording" || startInFlight) return;
  console.info("[dictation-pill] capture start", { status: status.value });
  const generation = ++captureGeneration;
  startInFlight = true;
  pcmChunks = [];
  pcmSampleCount = 0;
  errorText.value = "";
  subText.value = "";
  delivery.value = null;
  deliveryReason.value = "";
  backgroundDelivery.value = false;
  queuedUuid = null;
  expectedTranscriptUuid = null;
  bufferedDeliveryEvents.clear();
  elapsedSec.value = 0;
  await loadDictationConfig();
  if (!(await ensureReadyDictationModel())) {
    // Модель не готова — снимаем in-flight флаги, иначе startInFlight
    // навсегда залипнет true и любой следующий toggle молча проигнорируется.
    startInFlight = false;
    stopAfterStart = false;
    return;
  }

  let stream: MediaStream;
  try {
    stream = await ensureStream();
  } catch (e) {
    startInFlight = false;
    status.value = "error";
    errorText.value = "Нет доступа к микрофону";
    console.error("[dictation-pill] getUserMedia failed", { error: String(e) });
    try {
      await window.kepler.ark.request("dictation.cancel", {});
    } catch {
      /* ignore */
    }
    void window.kepler.dictation.pillFinished();
    return;
  }
  startInFlight = false;
  if (generation !== captureGeneration) {
    scheduleStreamShutdown();
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
    pcmSampleCount += i16.length;
    if (pcmChunks.length === 1 || pcmChunks.length % 20 === 0) {
      console.debug("[dictation-pill] pcm chunk", {
        chunks: pcmChunks.length,
        chunkSamples: i16.length,
        totalSamples: pcmSampleCount,
      });
    }
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
    for (let i = halfCount - 1; i >= 0; i--) {
      const dataIndex = Math.floor((i / halfCount) * relevantData.length);
      const value = Math.min(1, ((relevantData[dataIndex] ?? 0) / 255) * WAVEFORM_SENSITIVITY);
      bars[halfCount - 1 - i] = Math.max(0, value);
    }
    for (let i = 0; i < halfCount; i++) {
      const dataIndex = Math.floor((i / halfCount) * relevantData.length);
      const value = Math.min(1, ((relevantData[dataIndex] ?? 0) / 255) * WAVEFORM_SENSITIVITY);
      bars[halfCount + i] = Math.max(0, value);
    }
    levelBars.value = bars;
  }, 80);
  if (stopAfterStart) {
    stopAfterStart = false;
    void stopAndSubmit();
  }
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
  lastActiveBars = levelBars.value.length > 0 ? [...levelBars.value] : idleBars;
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

/** Ждёт появления первых PCM-кадров (или истечения timeoutMs). Процессор
 *  должен оставаться подключённым (teardownCapture ещё НЕ вызван), иначе
 *  onaudioprocess не дольёт буфер. Резолвится сразу, как только pcmChunks
 *  непустой. */
function waitForFirstPcm(timeoutMs: number): Promise<void> {
  return new Promise((resolve) => {
    if (pcmChunks.length > 0) {
      resolve();
      return;
    }
    const startMs = Date.now();
    const tick = () => {
      if (pcmChunks.length > 0 || Date.now() - startMs >= timeoutMs) {
        resolve();
        return;
      }
      setTimeout(tick, 20);
    };
    setTimeout(tick, 20);
  });
}

async function stopAndSubmit(): Promise<void> {
  if (status.value !== "recording") {
    // Stop пришёл пока start ещё в полёте — отложим submit до конца startCapture
    // (он сам дёрнет stopAndSubmit), чтобы быстрая запись не потерялась.
    if (startInFlight) stopAfterStart = true;
    return;
  }
  if (stopInFlight) return;
  stopInFlight = true;
  const generation = captureGeneration;
  try {
    console.info("[dictation-pill] capture stop", {
      chunks: pcmChunks.length,
      samples: pcmSampleCount,
    });
    const sampleRate = audioCtx?.sampleRate ?? TARGET_SAMPLE_RATE;
    // Короткая фраза: processor мог ещё не отдать первый буфер. Ждём (bounded)
    // первые кадры ДО teardownCapture — пока граф ещё подключён.
    if (pcmChunks.length === 0) {
      await waitForFirstPcm(PCM_FIRST_FRAME_GRACE_MS);
    }
    // Cancel во время grace-wait — generation сдвинулся; ничего не отправляем.
    if (generation !== captureGeneration) {
      return;
    }
    teardownCapture();
    if (pcmChunks.length === 0) {
      // Действительно пусто (ни одного кадра за grace-период) — тихо закрываемся.
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
    console.info("[dictation-pill] wav encoded", {
      samples: pcm.length,
      sampleRate,
      durationSec,
      bytes: wav.byteLength,
    });
    const b64 = bytesToBase64(wav);
    try {
      // SAFETY: the surrounding domain validation preserves the asserted contract.
      const resp = (await window.kepler.ark.request("dictation.submit_audio", {
        audioB64: b64,
        durationSec,
      })) as {
        uuid?: string;
        state?: PillStatus | "pending";
        queued?: boolean;
        injected?: boolean;
        delivery?: TranscriptDelivery;
        deliveryReason?: string | null;
        error?: string;
      };
      if (resp.state === "error") {
        // Fatal от backend (401/400/403/etc) — показываем user_msg, закроемся
        // с error mark. Pending всё ещё на диске — юзер увидит в Settings.
        expectedTranscriptUuid = resp.uuid ?? expectedTranscriptUuid;
        if (!resp.delivery || !applyDelivery(resp)) {
          status.value = "error";
          errorText.value = resp.error ?? "Не удалось распознать";
        } else if (resp.error) {
          errorText.value = resp.error;
        }
        console.warn("[dictation-pill] submit_audio response", {
          state: resp.state,
          uuid: resp.uuid,
          error: errorText.value,
        });
      } else if (resp.queued && resp.uuid) {
        // Первая попытка fail → backend запустил auto-retry в фоне.
        // Поллим очередь со спиннером "Жду сеть…".
        queuedUuid = resp.uuid;
        expectedTranscriptUuid = resp.uuid;
        const buffered = bufferedDeliveryEvents.get(resp.uuid);
        bufferedDeliveryEvents.delete(resp.uuid);
        if (buffered) applyDelivery(buffered, true);
        console.info("[dictation-pill] submit_audio response", {
          state: "pending",
          queued: true,
        });
      } else {
        console.info("[dictation-pill] submit_audio response", {
          state: resp.state ?? "unknown",
          injected: resp.injected ?? null,
          delivery: resp.delivery ?? null,
        });
        // Clipboard-only is a valid success path: backend intentionally does
        // not report an OS paste in that mode.
        expectedTranscriptUuid = resp.uuid ?? expectedTranscriptUuid;
        if (resp.uuid) bufferedDeliveryEvents.delete(resp.uuid);
        if (!applyDelivery(resp)) status.value = "idle";
      }
    } catch (e) {
      status.value = "error";
      // SAFETY: the surrounding domain validation preserves the asserted contract.
      errorText.value = (e as Error)?.message ?? "Ошибка распознавания";
      console.error("[dictation-pill] submit_audio failed", {
        error: String(e),
      });
    }
    scheduleStreamShutdown();

    if (queuedUuid) {
      // Висим со спиннером пока background retry работает. Backend расписание:
      // 1+5+10+20+40 sec = 76s sleeps + ~5×8s attempt window ≈ ~120s максимум.
      // Даём 130s timeout — чуть больше чем полный цикл backend.
      await waitForQueueResolve(queuedUuid, 130_000);
    }
    setTimeout(() => void window.kepler.dictation.pillFinished(), deliveryFinishDelay());
  } finally {
    stopInFlight = false;
  }
}

/// Поллит `dictation.list_pending` пока наш uuid в очереди, или истекает
/// timeoutMs. Меняет статус pill на 'waiting' (спиннер + subText). На исходе
/// либо переходим в idle (success — item исчез), либо в error (timeout).
async function waitForQueueResolve(uuid: string, timeoutMs: number): Promise<void> {
  if (delivery.value) return;
  const startMs = Date.now();
  status.value = "waiting";
  subText.value = "Жду сеть…";
  while (Date.now() - startMs < timeoutMs) {
    if (delivery.value) return;
    try {
      // SAFETY: the surrounding domain validation preserves the asserted contract.
      const resp = (await window.kepler.ark.request("dictation.list_pending", {})) as {
        items?: { uuid: string; attempts: number }[];
      };
      const item = (resp.items ?? []).find((i) => i.uuid === uuid);
      if (!item) {
        // Item исчез → backend сделал success+drop. Закрываемся тихо.
        if (!delivery.value) status.value = "idle";
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
  console.info("[dictation-pill] capture cancel", {
    chunks: pcmChunks.length,
    samples: pcmSampleCount,
  });
  captureGeneration++;
  startInFlight = false;
  stopAfterStart = false;
  stopInFlight = false;
  teardownCapture();
  pcmChunks = [];
  pcmSampleCount = 0;
  status.value = "idle";
  try {
    await window.kepler.ark.request("dictation.cancel", {});
  } catch {
    /* ignore */
  }
  scheduleStreamShutdown();
  void window.kepler.dictation.pillFinished();
}

onMounted(() => {
  if (!isPreview) void loadDictationConfig();
  if (isPreview) {
    startProcessingWave();
    return;
  }
  unsubscribeCommand = window.kepler.dictation.onCommand((cmd) => {
    console.info("[dictation-pill] command received", { kind: cmd.kind });
    if (cmd.kind === "start") {
      void startCapture();
    } else if (cmd.kind === "stop") {
      void stopAndSubmit();
    } else if (cmd.kind === "cancel") {
      void cancelCapture();
    }
  });
  unsubscribeArkEvent = window.kepler.ark.onEvent((event) => handleTranscriptEvent(event));
});

const stopStatusWatch = watch(
  status,
  (value) => {
    if (isPreview) return;
    if (value === "transcribing" || value === "waiting" || value === "error") {
      startProcessingWave();
    } else {
      stopProcessingWave();
    }
  },
  { immediate: true },
);

const stopWaveformWatch = watch([levelBars, processingBars, status], () => requestWaveformDraw(), {
  immediate: true,
});

onBeforeUnmount(() => {
  unsubscribeCommand?.();
  unsubscribeArkEvent?.();
  stopStatusWatch();
  stopWaveformWatch();
  stopProcessingWave();
  teardownCapture();
  // Окно демонтируется (Kepler закрывают) — hard-close без grace-периода.
  cancelStreamShutdown();
  closeStream();
  if (waveformFrame !== null) {
    cancelAnimationFrame(waveformFrame);
    waveformFrame = null;
  }
  waveformResizeObserver?.disconnect();
  waveformResizeObserver = null;
  waveformCanvases.clear();
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
            <div class="pill-wave">
              <canvas
                class="waveform"
                :class="`${item.status}-wave`"
                :title="item.subText ?? item.errorText"
                :ref="(el) => setWaveformCanvas(`preview-${item.status}`, el)"
                aria-hidden="true"
              />
            </div>
            <div class="pill-footer" :class="`pill-footer--${item.status}`">
              <span class="pill-footer__status">
                <span
                  class="pill-footer__dot"
                  :class="`pill-footer__dot--${item.status}`"
                  aria-hidden="true"
                />
                {{ footerLabel(item.status) }}
              </span>
              <div
                v-if="canCancel(item.status) || canSubmit(item.status)"
                class="pill-footer__actions"
              >
                <button
                  v-if="canCancel(item.status)"
                  class="pill-footer__hint"
                  type="button"
                  title="Отменить"
                  @pointerdown.stop.prevent="handleCancelClick"
                >
                  Отмена
                </button>
                <span
                  v-if="canCancel(item.status) && canSubmit(item.status)"
                  class="pill-footer__sep"
                  aria-hidden="true"
                />
                <button
                  v-if="canSubmit(item.status)"
                  class="pill-footer__hint pill-footer__hint--primary"
                  type="button"
                  title="Отправить"
                  @pointerdown.stop.prevent="handleSubmitClick"
                >
                  Отправить
                  <span class="pill-footer__hotkey" aria-hidden="true">
                    <KbdKey v-for="part in dictationHotkeyParts" :key="part">{{
                      hotkeyPartLabel(part)
                    }}</KbdKey>
                  </span>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
  <div v-else class="stage">
    <div class="pill" :class="`status-${status}`" :title="exposeStatusText">
      <!-- Waveform внутри пилюли: вертикальные бары разной высоты, по центру. -->
      <div class="pill-wave">
        <canvas
          class="waveform"
          :class="`${status}-wave`"
          :title="subText || errorText"
          :ref="(el) => setWaveformCanvas('runtime', el)"
          aria-hidden="true"
        />
      </div>
      <div
        class="pill-footer"
        :class="delivery ? `pill-footer--delivery-${delivery}` : `pill-footer--${status}`"
      >
        <span class="pill-footer__status">
          <span
            class="pill-footer__dot"
            :class="
              delivery ? `pill-footer__dot--delivery-${delivery}` : `pill-footer__dot--${status}`
            "
            aria-hidden="true"
          />
          {{ footerLabel(status, delivery) }}
        </span>
        <div v-if="canCancel(status) || canSubmit(status)" class="pill-footer__actions">
          <button
            v-if="canCancel(status)"
            class="pill-footer__hint"
            type="button"
            title="Отменить"
            @pointerdown.stop.prevent="handleCancelClick"
          >
            Отмена
          </button>
          <span
            v-if="canCancel(status) && canSubmit(status)"
            class="pill-footer__sep"
            aria-hidden="true"
          />
          <button
            v-if="canSubmit(status)"
            class="pill-footer__hint pill-footer__hint--primary"
            type="button"
            title="Отправить"
            @pointerdown.stop.prevent="handleSubmitClick"
          >
            Отправить
            <span class="pill-footer__hotkey" aria-hidden="true">
              <KbdKey v-for="part in dictationHotkeyParts" :key="part">{{
                hotkeyPartLabel(part)
              }}</KbdKey>
            </span>
          </button>
        </div>
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
  min-height: 148px;
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
  width: 380px;
  height: 126px;
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
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  padding: 0;
  /* Глянцевый чёрный — тонкий светлый highlight сверху, тёмный низ. */
  background: color-mix(in srgb, var(--surface) 58%, var(--background) 42%);
  border-radius: 8px;
  border: 1px solid color-mix(in srgb, var(--foreground) 18%, transparent);
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

.pill-wave {
  position: relative;
  z-index: 1;
  width: 100%;
  flex: 1 1 auto;
  min-height: 0;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 8px 10px 6px;
  overflow: hidden;
}

.waveform {
  display: block;
  width: 100%;
  height: 76px;
  transition: opacity 120ms linear;
}

.pill-footer {
  position: relative;
  z-index: 2;
  width: 100%;
  height: 34px;
  flex: 0 0 34px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 10px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  background: color-mix(in srgb, var(--background) 18%, transparent);
  color: color-mix(in srgb, var(--foreground) 78%, transparent);
  font-family: var(--font-sans, -apple-system, sans-serif);
  font-size: 11px;
  line-height: 1;
}

.pill-footer__status,
.pill-footer__actions,
.pill-footer__hint {
  display: inline-flex;
  align-items: center;
}

.pill-footer__status {
  min-width: 0;
  gap: 6px;
  white-space: nowrap;
}

.pill-footer__dot {
  width: 7px;
  height: 7px;
  flex: 0 0 auto;
  border-radius: var(--radius-pill, 999px);
  background: color-mix(in srgb, var(--foreground) 44%, transparent);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--foreground) 8%, transparent);
}

.pill-footer__dot--recording,
.pill-footer__dot--error {
  background: #ff453a;
  box-shadow: 0 0 0 2px color-mix(in srgb, #ff453a 14%, transparent);
}

.pill-footer__dot--waiting {
  background: #f5a524;
  box-shadow: 0 0 0 2px color-mix(in srgb, #f5a524 14%, transparent);
}

.pill-footer--delivery-pasted {
  color: #5eead4;
}

.pill-footer--delivery-clipboard_only {
  color: #f5a524;
}

.pill-footer--delivery-clipboard_fallback,
.pill-footer--delivery-failed {
  color: #ff453a;
}

.pill-footer__dot--delivery-pasted {
  background: #2dd4bf;
  box-shadow: 0 0 0 2px color-mix(in srgb, #2dd4bf 14%, transparent);
}

.pill-footer__dot--delivery-clipboard_only {
  background: #f5a524;
  box-shadow: 0 0 0 2px color-mix(in srgb, #f5a524 14%, transparent);
}

.pill-footer__dot--delivery-clipboard_fallback,
.pill-footer__dot--delivery-failed {
  background: #ff453a;
  box-shadow: 0 0 0 2px color-mix(in srgb, #ff453a 14%, transparent);
}

.pill-footer__actions {
  flex: 0 0 auto;
  gap: 7px;
}

.pill-footer__hint {
  gap: 4px;
  min-width: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 80%, transparent);
  font: inherit;
  cursor: default;
  white-space: nowrap;
}

.pill-footer__hint:hover {
  color: color-mix(in srgb, var(--foreground) 94%, transparent);
}

.pill-footer__hint--primary {
  color: color-mix(in srgb, var(--foreground) 92%, transparent);
}

.pill-footer__sep {
  width: 1px;
  height: 12px;
  background: color-mix(in srgb, var(--foreground) 16%, transparent);
}

.pill-footer__hotkey {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
</style>

/* eslint-disable no-console */

import {
  EndSensitivity,
  type FunctionCall,
  GoogleGenAI,
  type LiveServerMessage,
  Modality,
  type Session,
  StartSensitivity,
  TurnCoverage,
} from "@google/genai";

const GEMINI_MODEL = "gemini-2.5-flash-native-audio-preview-12-2025";

const GEMINI_AUDIO_MIME_TYPE = "audio/pcm;rate=16000";

const TARGET_SAMPLE_RATE = 16_000;

const CREATE_TASK_FUNCTION = "create_task";

const UPDATE_LAST_TASK_FUNCTION = "update_last_task";

const DELETE_LAST_TASK_FUNCTION = "delete_last_task";

const LIVE_DEBUG = import.meta.env.VITE_GEMINI_LIVE_DEBUG === "1";

const VAD_PREFIX_PADDING_MS = 120;

const VAD_SILENCE_DURATION_MS = 260;

const SYSTEM_INSTRUCTION = `
You are a realtime todo capture engine, similar to Todoist Ramble.
The user is dictating tasks by voice, mostly in Russian.

Rules:
1) Create tasks immediately while user is speaking. Do not wait for end of dictation.
2) Keep task titles in the user's language. If user speaks Russian, titles must be Russian.
3) Use create_task for each actionable item.
4) If user corrects the previous item, call update_last_task.
5) If user cancels previous item, call delete_last_task.
6) Do not output normal text. Use only tool calls.
7) Assume each completed user utterance is a task by default.
`.trim();

type LiveVoiceTaskCallbacks = {
  onCreateTask: (title: string) => Promise<boolean>;

  onUpdateLastTask: (title: string) => Promise<boolean>;

  onDeleteLastTask: () => Promise<boolean>;

  onTranscript?: (transcript: string) => void;

  onError?: (message: string) => void;
};

type LiveVoiceTaskHandle = {
  stop: () => Promise<void>;
};

function getGeminiApiKey() {
  const key = import.meta.env.VITE_GEMINI_API_KEY?.trim();

  if (!key) {
    throw new Error(
      "VITE_GEMINI_API_KEY is missing. Add Gemini API key before using voice tasks.",
    );
  }

  return key;
}

function toErrorMessage(error: unknown) {
  if (error instanceof Error) return error.message;

  return "Gemini Live API request failed.";
}

function normalizeTaskTitle(value: unknown) {
  if (typeof value !== "string") return "";

  return value.replace(/\s+/g, " ").trim();
}

function mergeTranscriptChunk(current: string, chunk: string) {
  const base = current.trim();

  const part = chunk.trim();

  if (!base) return part;

  if (!part) return base;

  if (base === part) return base;

  if (part.startsWith(base)) return part;

  if (base.startsWith(part)) return base;

  if (base.endsWith(part)) return base;

  if (part.endsWith(base)) return part;

  return `${base} ${part}`.replace(/\s+/g, " ").trim();
}

function isLikelyEditCommand(text: string) {
  return /^(исправь|замени|удали|не надо|пропусти|remove|delete|scratch|actually)\b/i.test(
    text.trim(),
  );
}

function toBase64(bytes: Uint8Array) {
  let binary = "";

  const chunkSize = 0x8000;

  for (let index = 0; index < bytes.length; index += chunkSize) {
    const slice = bytes.subarray(index, index + chunkSize);

    binary += String.fromCharCode(...slice);
  }

  return btoa(binary);
}

function downsampleTo16k(samples: Float32Array, sourceRate: number) {
  if (sourceRate === TARGET_SAMPLE_RATE) {
    return samples;
  }

  const ratio = sourceRate / TARGET_SAMPLE_RATE;

  const outputLength = Math.max(1, Math.floor(samples.length / ratio));

  const output = new Float32Array(outputLength);

  for (let index = 0; index < outputLength; index += 1) {
    const position = index * ratio;

    const left = Math.floor(position);

    const right = Math.min(left + 1, samples.length - 1);

    const weight = position - left;

    output[index] = samples[left] * (1 - weight) + samples[right] * weight;
  }

  return output;
}

function floatToPcm16(samples: Float32Array) {
  const pcm = new Int16Array(samples.length);

  for (let index = 0; index < samples.length; index += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[index]));

    pcm[index] = clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff;
  }

  return new Uint8Array(pcm.buffer);
}

function stopStream(stream: MediaStream | null) {
  if (!stream) return;

  for (const track of stream.getTracks()) {
    track.stop();
  }
}

type ToolResponsePayload = {
  id?: string;

  name?: string;

  response: Record<string, unknown>;
};

class LiveVoiceTaskSession {
  private readonly ai = new GoogleGenAI({ apiKey: getGeminiApiKey() });

  private readonly processedToolCallIds = new Set<string>();

  private readonly emittedCreateTitles = new Set<string>();

  private stream: MediaStream | null = null;

  private audioContext: AudioContext | null = null;

  private sourceNode: MediaStreamAudioSourceNode | null = null;

  private processorNode: ScriptProcessorNode | null = null;

  private silentGainNode: GainNode | null = null;

  private session: Session | null = null;

  private isClosed = false;

  private wsOpen = false;

  private setupComplete = false;

  private transcript = "";

  private utteranceBuffer = "";

  private sentAudioChunks = 0;

  private errorsReported = 0;

  constructor(private readonly callbacks: LiveVoiceTaskCallbacks) {}

  private debug(message: string, details?: unknown) {
    if (!LIVE_DEBUG) return;

    if (details === undefined) {
      console.debug(`[gemini-live] ${message}`);

      return;
    }

    console.debug(`[gemini-live] ${message}`, details);
  }

  private reportError(message: string) {
    this.errorsReported += 1;

    if (this.errorsReported > 4) return;

    this.callbacks.onError?.(message);
  }

  private async cleanupAudioPipeline() {
    try {
      this.processorNode?.disconnect();

      this.sourceNode?.disconnect();

      this.silentGainNode?.disconnect();
    } catch {
      // Ignore disconnect errors.
    }

    this.processorNode = null;

    this.sourceNode = null;

    this.silentGainNode = null;

    stopStream(this.stream);

    this.stream = null;

    if (this.audioContext && this.audioContext.state !== "closed") {
      await this.audioContext.close();
    }

    this.audioContext = null;
  }

  private async forceStopWithoutSignal() {
    if (this.isClosed) return;

    this.isClosed = true;

    this.wsOpen = false;

    await this.cleanupAudioPipeline();

    this.session = null;
  }

  async start() {
    if (!navigator.mediaDevices?.getUserMedia) {
      throw new Error("Audio recording is not supported in this environment.");
    }

    if (typeof AudioContext === "undefined") {
      throw new Error("AudioContext API is not available in this environment.");
    }

    if (typeof window === "undefined" || !("ScriptProcessorNode" in window)) {
      throw new Error(
        "ScriptProcessorNode is not available in this environment.",
      );
    }

    this.stream = await navigator.mediaDevices.getUserMedia({
      audio: {
        channelCount: 1,

        sampleRate: TARGET_SAMPLE_RATE,

        echoCancellation: true,

        noiseSuppression: true,

        autoGainControl: true,
      },
    });

    this.session = await this.ai.live.connect({
      model: GEMINI_MODEL,

      config: {
        systemInstruction: SYSTEM_INSTRUCTION,

        responseModalities: [Modality.AUDIO],

        inputAudioTranscription: {},

        temperature: 0.1,

        realtimeInputConfig: {
          automaticActivityDetection: {
            startOfSpeechSensitivity: StartSensitivity.START_SENSITIVITY_HIGH,

            endOfSpeechSensitivity: EndSensitivity.END_SENSITIVITY_HIGH,

            prefixPaddingMs: VAD_PREFIX_PADDING_MS,

            silenceDurationMs: VAD_SILENCE_DURATION_MS,
          },

          turnCoverage: TurnCoverage.TURN_INCLUDES_ONLY_ACTIVITY,
        },

        tools: [
          {
            functionDeclarations: [
              {
                name: CREATE_TASK_FUNCTION,

                description:
                  "Create one task immediately from current speech fragment.",

                parametersJsonSchema: {
                  type: "object",

                  properties: {
                    title: {
                      type: "string",

                      description: "Short task title in user language.",
                    },
                  },

                  required: ["title"],
                },
              },

              {
                name: UPDATE_LAST_TASK_FUNCTION,

                description:
                  "Update the most recently created task when user corrects wording.",

                parametersJsonSchema: {
                  type: "object",

                  properties: {
                    title: {
                      type: "string",

                      description: "Corrected title in user language.",
                    },
                  },

                  required: ["title"],
                },
              },

              {
                name: DELETE_LAST_TASK_FUNCTION,

                description:
                  "Delete the most recently created task when user cancels it.",

                parametersJsonSchema: {
                  type: "object",

                  properties: {},
                },
              },
            ],
          },
        ],
      },

      callbacks: {
        onopen: () => {
          this.wsOpen = true;

          this.debug("WebSocket opened.");
        },

        onmessage: (message) => {
          void this.handleMessage(message);
        },

        onerror: (error) => {
          this.debug("WebSocket error event.", error);
        },

        onclose: (event) => {
          this.wsOpen = false;

          if (this.isClosed) return;

          const code = event?.code ?? "unknown";

          const reason = event?.reason?.trim() || "no reason from server";

          const diagnostic = `Gemini Live closed: code=${code}, reason="${reason}", setupComplete=${this.setupComplete}, sentAudioChunks=${this.sentAudioChunks}.`;

          this.debug("WebSocket closed.", diagnostic);

          this.reportError(diagnostic);

          void this.forceStopWithoutSignal();
        },
      },
    });

    this.audioContext = new AudioContext();

    this.sourceNode = this.audioContext.createMediaStreamSource(this.stream);

    this.processorNode = this.audioContext.createScriptProcessor(1024, 1, 1);

    this.silentGainNode = this.audioContext.createGain();

    this.silentGainNode.gain.value = 0;

    this.processorNode.onaudioprocess = (event) => {
      if (!this.session || this.isClosed || !this.wsOpen) return;

      const input = event.inputBuffer.getChannelData(0);

      const chunk = new Float32Array(input);

      const downsampled = downsampleTo16k(
        chunk,

        this.audioContext?.sampleRate ?? 48000,
      );

      const pcm = floatToPcm16(downsampled);

      const data = toBase64(pcm);

      if (!data) return;

      try {
        this.session.sendRealtimeInput({
          audio: {
            mimeType: GEMINI_AUDIO_MIME_TYPE,

            data,
          },
        });

        this.sentAudioChunks += 1;
      } catch (error) {
        this.wsOpen = false;

        const message = toErrorMessage(error);

        this.debug("Failed to send realtime audio chunk.", message);

        this.reportError(`Audio send failed: ${message}`);

        void this.forceStopWithoutSignal();
      }
    };

    this.sourceNode.connect(this.processorNode);

    this.processorNode.connect(this.silentGainNode);

    this.silentGainNode.connect(this.audioContext.destination);

    await this.audioContext.resume();
  }

  async stop() {
    if (this.isClosed) return;

    this.isClosed = true;

    try {
      if (this.session && this.wsOpen) {
        this.session.sendRealtimeInput({ audioStreamEnd: true });
      }
    } catch {
      // Ignore signal send errors during shutdown.
    }

    await new Promise((resolve) => setTimeout(resolve, 350));

    await this.cleanupAudioPipeline();

    this.session?.close();

    this.session = null;

    this.wsOpen = false;
  }

  private async executeToolCall(
    call: FunctionCall,
  ): Promise<ToolResponsePayload> {
    const responseBase = {
      id: call.id,

      name: call.name,
    };

    if (call.id && this.processedToolCallIds.has(call.id)) {
      return {
        ...responseBase,

        response: { output: { ok: true, ignored: "duplicate_call" } },
      };
    }

    if (call.id) {
      this.processedToolCallIds.add(call.id);
    }

    try {
      if (call.name === CREATE_TASK_FUNCTION) {
        const title = normalizeTaskTitle(call.args?.title);

        if (!title) {
          return {
            ...responseBase,

            response: { error: "title is required for create_task" },
          };
        }

        const dedupeKey = title.toLowerCase();

        if (this.emittedCreateTitles.has(dedupeKey)) {
          return {
            ...responseBase,

            response: {
              output: { ok: true, ignored: "duplicate_title", title },
            },
          };
        }

        const ok = await this.callbacks.onCreateTask(title);

        if (ok) this.emittedCreateTitles.add(dedupeKey);

        return {
          ...responseBase,

          response: { output: { ok, action: "create", title } },
        };
      }

      if (call.name === UPDATE_LAST_TASK_FUNCTION) {
        const title = normalizeTaskTitle(call.args?.title);

        if (!title) {
          return {
            ...responseBase,

            response: { error: "title is required for update_last_task" },
          };
        }

        const ok = await this.callbacks.onUpdateLastTask(title);

        return {
          ...responseBase,

          response: { output: { ok, action: "update_last", title } },
        };
      }

      if (call.name === DELETE_LAST_TASK_FUNCTION) {
        const ok = await this.callbacks.onDeleteLastTask();

        return {
          ...responseBase,

          response: { output: { ok, action: "delete_last" } },
        };
      }

      return {
        ...responseBase,

        response: { error: `unsupported function: ${call.name ?? "unknown"}` },
      };
    } catch (error) {
      return {
        ...responseBase,

        response: { error: toErrorMessage(error) },
      };
    }
  }

  private async createTaskFromTranscriptFallback(text: string) {
    const title = normalizeTaskTitle(text);

    if (!title) return;

    if (isLikelyEditCommand(title)) return;

    const dedupeKey = title.toLowerCase();

    if (this.emittedCreateTitles.has(dedupeKey)) return;

    const ok = await this.callbacks.onCreateTask(title);

    if (ok) {
      this.emittedCreateTitles.add(dedupeKey);

      this.debug("Fallback create_task from transcription.", title);
    }
  }

  private async handleMessage(message: LiveServerMessage) {
    if (message.setupComplete) {
      this.setupComplete = true;

      this.debug("Received setupComplete.");
    }

    const inputTranscription = message.serverContent?.inputTranscription;

    const transcriptChunk = inputTranscription?.text?.trim();

    if (transcriptChunk) {
      this.transcript = mergeTranscriptChunk(this.transcript, transcriptChunk);

      this.utteranceBuffer = mergeTranscriptChunk(
        this.utteranceBuffer,

        transcriptChunk,
      );

      this.callbacks.onTranscript?.(this.transcript);
    }

    const functionCalls = message.toolCall?.functionCalls ?? [];

    const hasAnyToolCall = functionCalls.length > 0;

    if (functionCalls.length === 0 || !this.session) {
      const boundaryReached =
        Boolean(inputTranscription?.finished) ||
        Boolean(message.serverContent?.turnComplete);

      if (boundaryReached && this.utteranceBuffer) {
        try {
          await this.createTaskFromTranscriptFallback(this.utteranceBuffer);
        } catch (error) {
          this.reportError(
            `Fallback task create failed: ${toErrorMessage(error)}`,
          );
        } finally {
          this.utteranceBuffer = "";
        }
      }

      return;
    }

    const responses: ToolResponsePayload[] = [];

    for (const call of functionCalls) {
      // eslint-disable-next-line no-await-in-loop -- sequential tool execution required
      responses.push(await this.executeToolCall(call));
    }

    if (responses.length > 0) {
      try {
        this.session.sendToolResponse({ functionResponses: responses });
      } catch (error) {
        this.reportError(`Tool response send failed: ${toErrorMessage(error)}`);
      }
    }

    if (hasAnyToolCall) {
      this.utteranceBuffer = "";
    }
  }
}

export async function startLiveVoiceTasks(
  callbacks: LiveVoiceTaskCallbacks,
): Promise<LiveVoiceTaskHandle> {
  const session = new LiveVoiceTaskSession(callbacks);

  await session.start();

  return {
    stop: () => session.stop(),
  };
}

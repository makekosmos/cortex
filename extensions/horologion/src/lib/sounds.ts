// Простые звуки через Web Audio API — никаких внешних файлов.
// Каждое имя = функция, которая играет короткий тон / последовательность.

export type SoundName = "none" | "bell" | "chime" | "tap" | "alarm";

export interface SoundOption {
  value: SoundName;
  label: string;
}

export const SOUND_OPTIONS: readonly SoundOption[] = [
  { value: "none", label: "Без звука" },
  { value: "bell", label: "Колокольчик" },
  { value: "chime", label: "Перелив" },
  { value: "tap", label: "Стук" },
  { value: "alarm", label: "Сигнал" },
] as const;

let audioCtx: AudioContext | null = null;
function ctx(): AudioContext {
  if (!audioCtx) {
    const Ctor = (window as unknown as { AudioContext?: typeof AudioContext }).AudioContext;
    if (!Ctor) throw new Error("AudioContext недоступен");
    audioCtx = new Ctor();
  }
  if (audioCtx.state === "suspended") void audioCtx.resume();
  return audioCtx;
}

interface Tone {
  freq: number;
  duration: number; // ms
  delay?: number; // ms
  type?: OscillatorType;
  gain?: number; // 0..1
}

let volumeMultiplier = 1;

export function setVolumeMultiplier(v: number): void {
  volumeMultiplier = Math.max(0, Math.min(1, v));
}

function playTones(tones: Tone[]): void {
  let c: AudioContext;
  try {
    c = ctx();
  } catch {
    return;
  }
  const now = c.currentTime;
  for (const t of tones) {
    const start = now + (t.delay ?? 0) / 1000;
    const stop = start + t.duration / 1000;
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = t.type ?? "sine";
    osc.frequency.value = t.freq;
    const peak = Math.max(0.0001, (t.gain ?? 0.4) * volumeMultiplier);
    gain.gain.setValueAtTime(0.0001, start);
    gain.gain.exponentialRampToValueAtTime(peak, start + 0.012);
    gain.gain.exponentialRampToValueAtTime(0.0001, stop);
    osc.connect(gain);
    gain.connect(c.destination);
    osc.start(start);
    osc.stop(stop + 0.02);
  }
}

const SOUNDS: Record<Exclude<SoundName, "none">, () => void> = {
  bell: () =>
    playTones([
      { freq: 880, duration: 350, type: "sine", gain: 0.55 },
      { freq: 1320, duration: 280, type: "sine", gain: 0.35, delay: 30 },
    ]),
  chime: () =>
    playTones([
      { freq: 587.33, duration: 220, type: "sine", gain: 0.45 },
      { freq: 783.99, duration: 240, type: "sine", gain: 0.45, delay: 180 },
      { freq: 1174.66, duration: 380, type: "sine", gain: 0.35, delay: 360 },
    ]),
  tap: () =>
    playTones([
      { freq: 420, duration: 60, type: "triangle", gain: 0.6 },
      { freq: 320, duration: 90, type: "triangle", gain: 0.4, delay: 50 },
    ]),
  alarm: () =>
    playTones([
      { freq: 880, duration: 160, type: "square", gain: 0.4 },
      { freq: 660, duration: 160, type: "square", gain: 0.4, delay: 180 },
      { freq: 880, duration: 160, type: "square", gain: 0.4, delay: 360 },
      { freq: 660, duration: 160, type: "square", gain: 0.4, delay: 540 },
    ]),
};

export function playSound(name: SoundName): void {
  if (name === "none") return;
  try {
    SOUNDS[name]();
  } catch {
    /* ignore — audio context может быть заблокирован до user gesture'а */
  }
}

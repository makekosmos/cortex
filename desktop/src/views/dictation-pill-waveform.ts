export type PillStatus = "idle" | "recording" | "transcribing" | "waiting" | "error";

export const WAVEFORM_HISTORY_SIZE = 120;
export const WAVEFORM_BAR_WIDTH_PX = 3;
export const WAVEFORM_BAR_GAP_PX = 2;
const WAVEFORM_BAR_RADIUS_PX = 8;
export const WAVEFORM_BASE_BAR_HEIGHT_PX = 4;
export const WAVEFORM_FADE_WIDTH_PX = 48;
export const WAVEFORM_SENSITIVITY = 0.8;
const WAVEFORM_RECORDING_COLOR = "#71717a";
export const WAVE_BAR_COUNT = WAVEFORM_HISTORY_SIZE;

export const idleBars = Array.from({ length: WAVE_BAR_COUNT }, () => 0);

export function waveformColorForStatus(status: PillStatus): string {
  if (status === "waiting") return "#f5a524";
  if (status === "error") return "#ff453a";
  return WAVEFORM_RECORDING_COLOR;
}

export function sampleWaveformValue(values: number[], index: number, count: number): number {
  if (values.length === 0) return 0;
  const sourceIndex = Math.round((index / Math.max(1, count - 1)) * (values.length - 1));
  return Math.max(0, Math.min(1, values[sourceIndex] ?? 0));
}

function drawRoundedBar(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
): void {
  const radius = Math.min(WAVEFORM_BAR_RADIUS_PX, width / 2, height / 2);
// SAFETY: the surrounding domain validation preserves the asserted contract.
  const roundRect = (ctx as CanvasRenderingContext2D & {
    roundRect?: (x: number, y: number, width: number, height: number, radii?: number) => void;
  }).roundRect;
  if (roundRect) {
    ctx.beginPath();
    roundRect.call(ctx, x, y, width, height, radius);
    ctx.fill();
    return;
  }
  ctx.beginPath();
  ctx.moveTo(x + radius, y);
  ctx.lineTo(x + width - radius, y);
  ctx.quadraticCurveTo(x + width, y, x + width, y + radius);
  ctx.lineTo(x + width, y + height - radius);
  ctx.quadraticCurveTo(x + width, y + height, x + width - radius, y + height);
  ctx.lineTo(x + radius, y + height);
  ctx.quadraticCurveTo(x, y + height, x, y + height - radius);
  ctx.lineTo(x, y + radius);
  ctx.quadraticCurveTo(x, y, x + radius, y);
  ctx.fill();
}

export function drawWaveformCanvas(
  canvas: HTMLCanvasElement,
  values: number[],
  status: PillStatus,
): void {
  const rect = canvas.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0) return;

  const dpr = window.devicePixelRatio || 1;
  const targetWidth = Math.max(1, Math.floor(rect.width * dpr));
  const targetHeight = Math.max(1, Math.floor(rect.height * dpr));
  if (canvas.width !== targetWidth || canvas.height !== targetHeight) {
    canvas.width = targetWidth;
    canvas.height = targetHeight;
  }

  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, rect.width, rect.height);

  const step = WAVEFORM_BAR_WIDTH_PX + WAVEFORM_BAR_GAP_PX;
  const barCount = Math.max(1, Math.floor(rect.width / step));
  const totalWidth = barCount * WAVEFORM_BAR_WIDTH_PX + (barCount - 1) * WAVEFORM_BAR_GAP_PX;
  const startX = (rect.width - totalWidth) / 2;
  const centerY = rect.height / 2;

  if (status === "idle") {
    ctx.strokeStyle = waveformColorForStatus(status);
    ctx.globalAlpha = 0.22;
    ctx.lineWidth = 2;
    ctx.setLineDash([2, 4]);
    ctx.beginPath();
    ctx.moveTo(0, centerY);
    ctx.lineTo(rect.width, centerY);
    ctx.stroke();
    ctx.setLineDash([]);
  } else {
    ctx.fillStyle = waveformColorForStatus(status);
    for (let i = 0; i < barCount; i++) {
      const value = sampleWaveformValue(values, i, barCount);
      const barHeight = Math.max(
        WAVEFORM_BASE_BAR_HEIGHT_PX,
        value * rect.height * WAVEFORM_SENSITIVITY,
      );
      const x = startX + i * step;
      const y = centerY - barHeight / 2;
      ctx.globalAlpha = 0.4 + value * 0.6;
      drawRoundedBar(ctx, x, y, WAVEFORM_BAR_WIDTH_PX, barHeight);
    }
  }
  ctx.globalAlpha = 1;

  const fade = Math.min(0.3, WAVEFORM_FADE_WIDTH_PX / Math.max(1, rect.width));
  ctx.globalCompositeOperation = "destination-out";
  const gradient = ctx.createLinearGradient(0, 0, rect.width, 0);
  gradient.addColorStop(0, "rgba(255,255,255,1)");
  gradient.addColorStop(fade, "rgba(255,255,255,0)");
  gradient.addColorStop(1 - fade, "rgba(255,255,255,0)");
  gradient.addColorStop(1, "rgba(255,255,255,1)");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, rect.width, rect.height);
  ctx.globalCompositeOperation = "source-over";
}

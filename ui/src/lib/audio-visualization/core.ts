import type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";

export type VisualParticle = {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  size: number;
  phase: number;
};

export type VisualizationState = {
  mode: AudioVisualizationMode;
  frequencyHistory: number[][];
  waveformHistory: number[][];
  peakHistory: number[];
  heldPeaks: number[];
  particles: VisualParticle[];
  artworkImage: HTMLImageElement | null;
  artworkColors: string[];
  lastHistoryAt: number;
  lastFrameAt: number;
  phase: number;
};

export type VisualizationFrame = {
  context: CanvasRenderingContext2D;
  width: number;
  height: number;
  frequency: Uint8Array<ArrayBuffer>;
  waveform: Uint8Array<ArrayBuffer>;
  leftFrequency: Uint8Array<ArrayBuffer>;
  rightFrequency: Uint8Array<ArrayBuffer>;
  leftWaveform: Uint8Array<ArrayBuffer>;
  rightWaveform: Uint8Array<ArrayBuffer>;
  stereo: boolean;
  live: boolean;
  colors: string[];
  intensity: number;
  time: number;
  delta: number;
  sampleRate: number;
  artwork: HTMLImageElement | null;
};

export function createVisualizationState(): VisualizationState {
  return {
    mode: "off",
    frequencyHistory: [],
    waveformHistory: [],
    peakHistory: [],
    heldPeaks: [],
    particles: [],
    artworkImage: null,
    artworkColors: [],
    lastHistoryAt: 0,
    lastFrameAt: 0,
    phase: 0,
  };
}

export function resetVisualizationState(
  state: VisualizationState,
  mode: AudioVisualizationMode,
) {
  state.mode = mode;
  state.frequencyHistory = [];
  state.waveformHistory = [];
  state.peakHistory = [];
  state.heldPeaks = [];
  state.particles = [];
  state.artworkImage = null;
  state.artworkColors = [];
  state.lastHistoryAt = 0;
  state.lastFrameAt = 0;
  state.phase = 0;
}

export function updateVisualizationState(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  state.phase += frame.delta * (0.22 + energy(frame, 0, 0.2) * 0.8);
  const peak = Math.max(
    energy(frame, 0, 0.16),
    energy(frame, 0.16, 0.48),
    energy(frame, 0.48, 1),
  );
  if (!frame.live || frame.time - state.lastHistoryAt >= 68) {
    state.lastHistoryAt = frame.time;
    state.frequencyHistory.unshift(snapshotFrequency(frame, 72));
    state.waveformHistory.unshift(snapshotWaveform(frame, 96));
    state.peakHistory.unshift(frame.live ? peak : 0.18);
    state.frequencyHistory.length = Math.min(state.frequencyHistory.length, 32);
    state.waveformHistory.length = Math.min(state.waveformHistory.length, 18);
    state.peakHistory.length = Math.min(state.peakHistory.length, 96);
  }
}

export function mainBounds(frame: VisualizationFrame) {
  const sidebar = frame.width > 920 ? Math.min(252, frame.width * 0.18) : 0;
  return {
    left: sidebar,
    width: frame.width - sidebar,
    centerX: sidebar + (frame.width - sidebar) / 2,
    centerY: frame.height * 0.52,
  };
}

export function clamp01(value: number): number {
  return Math.min(1, Math.max(0, value));
}

export function frequencyValue(
  frame: VisualizationFrame,
  position: number,
  logarithmic = false,
  channel: "main" | "left" | "right" = "main",
): number {
  const source =
    channel === "left"
      ? frame.leftFrequency
      : channel === "right"
        ? frame.rightFrequency
        : frame.frequency;
  const normalized = clamp01(position);
  const mapped = logarithmic ? (Math.pow(10, normalized) - 1) / 9 : normalized;
  const index = Math.min(
    source.length - 1,
    Math.floor(mapped * source.length * 0.88),
  );
  if (frame.live) return source[index] / 255;
  return 0.09 + ((index * 13 + 7) % 11) / 110;
}

export function waveformValue(
  frame: VisualizationFrame,
  position: number,
  channel: "main" | "left" | "right" = "main",
): number {
  const source =
    channel === "left"
      ? frame.leftWaveform
      : channel === "right"
        ? frame.rightWaveform
        : frame.waveform;
  const index = Math.min(
    source.length - 1,
    Math.floor(clamp01(position) * (source.length - 1)),
  );
  if (frame.live) return (source[index] - 128) / 128;
  return (
    Math.sin(position * Math.PI * 9) * 0.055 +
    Math.sin(position * Math.PI * 3) * 0.025
  );
}

export function energy(
  frame: VisualizationFrame,
  start: number,
  end: number,
  channel: "main" | "left" | "right" = "main",
): number {
  const steps = 18;
  let total = 0;
  for (let index = 0; index < steps; index += 1) {
    total += frequencyValue(
      frame,
      start + ((end - start) * index) / Math.max(steps - 1, 1),
      true,
      channel,
    );
  }
  return total / steps;
}

export function waveformRms(
  frame: VisualizationFrame,
  channel: "main" | "left" | "right" = "main",
): number {
  let total = 0;
  const steps = 64;
  for (let index = 0; index < steps; index += 1) {
    const value = waveformValue(frame, index / (steps - 1), channel);
    total += value * value;
  }
  return clamp01(Math.sqrt(total / steps) * 2.8);
}

export function stereoCorrelation(frame: VisualizationFrame): number {
  if (!frame.stereo) return 1;
  let cross = 0;
  let leftPower = 0;
  let rightPower = 0;
  const steps = 96;
  for (let index = 0; index < steps; index += 1) {
    const position = index / (steps - 1);
    const left = waveformValue(frame, position, "left");
    const right = waveformValue(frame, position, "right");
    cross += left * right;
    leftPower += left * left;
    rightPower += right * right;
  }
  const denominator = Math.sqrt(leftPower * rightPower);
  return denominator > 0.0001
    ? Math.min(1, Math.max(-1, cross / denominator))
    : 1;
}

export function paletteColor(
  colors: string[],
  index: number,
  total: number,
): string {
  if (colors.length <= 1) return colors[0] ?? "currentColor";
  const normalized = Math.max(0, Math.min(0.999, index / Math.max(total, 1)));
  return (
    colors[Math.floor(normalized * colors.length)] ??
    colors[0] ??
    "currentColor"
  );
}

export function paletteGradient(
  context: CanvasRenderingContext2D,
  colors: string[],
  x0: number,
  y0: number,
  x1: number,
  y1: number,
): string | CanvasGradient {
  if (colors.length <= 1) return colors[0] ?? "currentColor";
  const gradient = context.createLinearGradient(x0, y0, x1, y1);
  colors.forEach((color, index) => {
    gradient.addColorStop(index / Math.max(colors.length - 1, 1), color);
  });
  return gradient;
}

export function radialGradient(
  context: CanvasRenderingContext2D,
  colors: string[],
  x: number,
  y: number,
  radius: number,
): CanvasGradient {
  const gradient = context.createRadialGradient(x, y, 0, x, y, radius);
  const source = colors.length > 0 ? colors : ["currentColor"];
  source.forEach((color, index) => {
    gradient.addColorStop((index / source.length) * 0.75, color);
  });
  gradient.addColorStop(1, "transparent");
  return gradient;
}

export function snapshotFrequency(
  frame: VisualizationFrame,
  count: number,
): number[] {
  return Array.from({ length: count }, (_, index) =>
    frequencyValue(frame, index / Math.max(count - 1, 1), true),
  );
}

export function snapshotWaveform(
  frame: VisualizationFrame,
  count: number,
): number[] {
  return Array.from({ length: count }, (_, index) =>
    waveformValue(frame, index / Math.max(count - 1, 1)),
  );
}

export function deterministic(index: number, salt = 0): number {
  const value = Math.sin(index * 12.9898 + salt * 78.233) * 43758.5453;
  return value - Math.floor(value);
}

export function drawArtworkFallback(
  frame: VisualizationFrame,
  x: number,
  y: number,
  size: number,
) {
  const { context } = frame;
  context.fillStyle = paletteGradient(
    context,
    frame.colors,
    x,
    y,
    x + size,
    y + size,
  );
  context.fillRect(x, y, size, size);
  context.globalAlpha *= 0.38;
  context.fillStyle = "black";
  for (let index = 0; index < 7; index += 1) {
    const inset = (size * index) / 16;
    context.fillRect(x + inset, y + inset, size - inset * 2, 1);
    context.fillRect(x + inset, y + size - inset, size - inset * 2, 1);
  }
}

export function drawArtworkCover(
  frame: VisualizationFrame,
  x: number,
  y: number,
  width: number,
  height = width,
) {
  const { context, artwork } = frame;
  if (artwork?.complete && artwork.naturalWidth > 0) {
    const imageRatio = artwork.naturalWidth / artwork.naturalHeight;
    const targetRatio = width / height;
    let sourceX = 0;
    let sourceY = 0;
    let sourceWidth = artwork.naturalWidth;
    let sourceHeight = artwork.naturalHeight;
    if (imageRatio > targetRatio) {
      sourceWidth = artwork.naturalHeight * targetRatio;
      sourceX = (artwork.naturalWidth - sourceWidth) / 2;
    } else {
      sourceHeight = artwork.naturalWidth / targetRatio;
      sourceY = (artwork.naturalHeight - sourceHeight) / 2;
    }
    context.drawImage(
      artwork,
      sourceX,
      sourceY,
      sourceWidth,
      sourceHeight,
      x,
      y,
      width,
      height,
    );
    return;
  }
  drawArtworkFallback(frame, x, y, Math.min(width, height));
}

export function extractedArtworkColors(
  frame: VisualizationFrame,
  state: VisualizationState,
): string[] {
  const image = frame.artwork;
  if (!image?.complete || image.naturalWidth <= 0) return frame.colors;
  if (state.artworkImage === image && state.artworkColors.length > 0) {
    return state.artworkColors;
  }
  state.artworkImage = image;
  try {
    const canvas = document.createElement("canvas");
    canvas.width = 16;
    canvas.height = 16;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return frame.colors;
    context.drawImage(image, 0, 0, 16, 16);
    const pixels = context.getImageData(0, 0, 16, 16).data;
    const candidates: Array<{ color: string; hue: number; score: number }> = [];
    for (let index = 0; index < pixels.length; index += 16) {
      const red = pixels[index] ?? 0;
      const green = pixels[index + 1] ?? 0;
      const blue = pixels[index + 2] ?? 0;
      const alpha = pixels[index + 3] ?? 0;
      const maximum = Math.max(red, green, blue);
      const minimum = Math.min(red, green, blue);
      const lightness = (maximum + minimum) / 2;
      if (alpha < 180 || lightness < 24 || lightness > 235) continue;
      candidates.push({
        color: `rgb(${red} ${green} ${blue})`,
        hue: rgbHue(red, green, blue),
        score: maximum - minimum,
      });
    }
    candidates.sort(
      (left, right) => left.hue - right.hue || right.score - left.score,
    );
    const selected = Array.from(
      { length: Math.min(5, candidates.length) },
      (_, index) => {
        const position = Math.floor(
          (index / Math.max(Math.min(5, candidates.length) - 1, 1)) *
            Math.max(candidates.length - 1, 0),
        );
        return candidates[position]?.color;
      },
    ).filter((color): color is string => Boolean(color));
    state.artworkColors = selected.length > 0 ? selected : frame.colors;
  } catch {
    state.artworkColors = frame.colors;
  }
  return state.artworkColors;
}

function rgbHue(red: number, green: number, blue: number): number {
  const normalizedRed = red / 255;
  const normalizedGreen = green / 255;
  const normalizedBlue = blue / 255;
  const maximum = Math.max(normalizedRed, normalizedGreen, normalizedBlue);
  const minimum = Math.min(normalizedRed, normalizedGreen, normalizedBlue);
  const delta = maximum - minimum;
  if (delta === 0) return 0;
  let hue: number;
  if (maximum === normalizedRed) {
    hue = ((normalizedGreen - normalizedBlue) / delta) % 6;
  } else if (maximum === normalizedGreen) {
    hue = (normalizedBlue - normalizedRed) / delta + 2;
  } else {
    hue = (normalizedRed - normalizedGreen) / delta + 4;
  }
  return (hue * 60 + 360) % 360;
}

export function roundedRect(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
) {
  const safeRadius = Math.min(radius, width / 2, height / 2);
  context.beginPath();
  context.roundRect(x, y, width, height, safeRadius);
}

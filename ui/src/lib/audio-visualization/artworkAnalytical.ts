import type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";
import {
  drawArtworkCover,
  energy,
  extractedArtworkColors,
  frequencyValue,
  mainBounds,
  paletteColor,
  paletteGradient,
  roundedRect,
  stereoCorrelation,
  waveformRms,
  waveformValue,
  type VisualizationFrame,
  type VisualizationState,
} from "$lib/audio-visualization/core";

const ARTWORK_MODES = new Set<AudioVisualizationMode>([
  "artwork-displacement",
  "artwork-particles",
  "artwork-mosaic",
  "vinyl-groove",
  "cassette-scope",
  "cover-halo",
  "color-extraction",
  "pixel-sort",
]);

const ANALYTICAL_MODES = new Set<AudioVisualizationMode>([
  "vu-needles",
  "digital-level-meter",
  "phase-correlation-meter",
  "goniometer",
  "chromagram-ring",
  "bass-mid-treble-triptych",
  "frequency-labels",
  "peak-history",
  "telemetry-matrix",
  "spectral-scanner",
]);

export function renderArtworkOrAnalytical(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
): boolean {
  if (ARTWORK_MODES.has(mode)) {
    renderArtwork(mode, frame, state);
    return true;
  }
  if (ANALYTICAL_MODES.has(mode)) {
    renderAnalytical(mode, frame, state);
    return true;
  }
  return false;
}

function renderArtwork(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "artwork-displacement") drawArtworkDisplacement(frame, state);
  else if (mode === "artwork-particles") drawArtworkParticles(frame, state);
  else if (mode === "artwork-mosaic") drawArtworkMosaic(frame);
  else if (mode === "vinyl-groove") drawVinyl(frame, state);
  else if (mode === "cassette-scope") drawCassette(frame);
  else if (mode === "cover-halo") drawCoverHalo(frame);
  else if (mode === "color-extraction") drawColorExtraction(frame, state);
  else drawPixelSort(frame);
}

function artworkSquare(frame: VisualizationFrame) {
  const bounds = mainBounds(frame);
  const size = Math.min(
    330,
    Math.max(170, Math.min(bounds.width, frame.height) * 0.34),
  );
  return {
    x: bounds.centerX - size / 2,
    y: bounds.centerY - size / 2,
    size,
  };
}

function drawArtworkDisplacement(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, intensity } = frame;
  const cover = artworkSquare(frame);
  const slices = 28;
  context.save();
  for (let slice = 0; slice < slices; slice += 1) {
    const y = cover.y + (slice / slices) * cover.size;
    const height = cover.size / slices + 1;
    const value = frequencyValue(frame, slice / slices, true);
    const offset = Math.sin(slice * 0.8 + state.phase) * value * 30 * intensity;
    context.save();
    context.beginPath();
    context.rect(cover.x + offset, y, cover.size, height);
    context.clip();
    context.globalAlpha = 0.12 + value * 0.32;
    drawArtworkCover(frame, cover.x + offset, cover.y, cover.size);
    context.restore();
  }
  context.restore();
}

function drawArtworkParticles(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, intensity } = frame;
  const bounds = mainBounds(frame);
  const colors = extractedArtworkColors(frame, state);
  const count = 170;
  for (let index = 0; index < count; index += 1) {
    const angle = (index / count) * Math.PI * 8 + state.phase * 0.15;
    const position = (index % 43) / 43;
    const value = frequencyValue(frame, position, true);
    const radius =
      Math.min(bounds.width, frame.height) *
      (0.06 + (index / count) * 0.32) *
      (1 + value * 0.15 * intensity);
    const x = bounds.centerX + Math.cos(angle) * radius;
    const y = bounds.centerY + Math.sin(angle) * radius * 0.66;
    context.globalAlpha = 0.06 + value * 0.32;
    context.fillStyle = paletteColor(colors, index, count);
    context.fillRect(x, y, 1 + value * 3, 1 + value * 3);
  }
  const cover = artworkSquare(frame);
  context.globalAlpha = 0.22;
  drawArtworkCover(
    frame,
    cover.x + cover.size * 0.27,
    cover.y + cover.size * 0.27,
    cover.size * 0.46,
  );
}

function drawArtworkMosaic(frame: VisualizationFrame) {
  const { context, intensity } = frame;
  const cover = artworkSquare(frame);
  const cells = 7;
  const cell = cover.size / cells;
  for (let row = 0; row < cells; row += 1) {
    for (let column = 0; column < cells; column += 1) {
      const index = row * cells + column;
      const value = frequencyValue(frame, index / (cells * cells), true);
      const scale = 0.72 + value * 0.3 * intensity;
      const inset = (cell * (1 - scale)) / 2;
      context.save();
      context.beginPath();
      context.rect(
        cover.x + column * cell + inset,
        cover.y + row * cell + inset,
        cell * scale - 2,
        cell * scale - 2,
      );
      context.clip();
      context.globalAlpha = 0.09 + value * 0.28;
      drawArtworkCover(frame, cover.x, cover.y, cover.size);
      context.restore();
    }
  }
}

function drawVinyl(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.2);
  const radius =
    Math.min(bounds.width, frame.height) * 0.28 * (1 + bass * 0.06 * intensity);
  context.save();
  context.translate(bounds.centerX, bounds.centerY);
  context.rotate(state.phase * 0.18);
  context.strokeStyle = colors[0] ?? "currentColor";
  for (let groove = 0; groove < 34; groove += 1) {
    context.globalAlpha = 0.025 + (groove % 5 === 0 ? 0.09 : 0);
    context.beginPath();
    context.arc(0, 0, radius * (0.24 + groove * 0.022), 0, Math.PI * 2);
    context.stroke();
  }
  context.restore();
  const coverSize = radius * 0.72;
  context.save();
  context.beginPath();
  context.arc(bounds.centerX, bounds.centerY, coverSize / 2, 0, Math.PI * 2);
  context.clip();
  context.globalAlpha = 0.32;
  drawArtworkCover(
    frame,
    bounds.centerX - coverSize / 2,
    bounds.centerY - coverSize / 2,
    coverSize,
  );
  context.restore();
}

function drawCassette(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const width = Math.min(520, bounds.width * 0.62);
  const height = width * 0.52;
  const x = bounds.centerX - width / 2;
  const y = bounds.centerY - height / 2;
  context.globalAlpha = 0.28;
  context.strokeStyle = colors[0] ?? "currentColor";
  context.lineWidth = 1;
  roundedRect(context, x, y, width, height, 12);
  context.stroke();
  const reelY = y + height * 0.42;
  [x + width * 0.3, x + width * 0.7].forEach((reelX, index) => {
    const value = energy(frame, index * 0.45, index * 0.45 + 0.38);
    context.globalAlpha = 0.14 + value * 0.32;
    context.beginPath();
    context.arc(
      reelX,
      reelY,
      height * (0.12 + value * 0.03 * intensity),
      0,
      Math.PI * 2,
    );
    context.stroke();
  });
  const scopeY = y + height * 0.72;
  context.globalAlpha = 0.52;
  context.strokeStyle = paletteGradient(context, colors, x, 0, x + width, 0);
  context.beginPath();
  for (let index = 0; index < 128; index += 1) {
    const position = index / 127;
    const px = x + width * 0.12 + position * width * 0.76;
    const py =
      scopeY + waveformValue(frame, position) * height * 0.1 * intensity;
    if (index === 0) context.moveTo(px, py);
    else context.lineTo(px, py);
  }
  context.stroke();
}

function drawCoverHalo(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const cover = artworkSquare(frame);
  const bounds = mainBounds(frame);
  const radius = cover.size * 0.57;
  const spokes = 88;
  for (let index = 0; index < spokes; index += 1) {
    const position = index / spokes;
    const value = frequencyValue(frame, position, true);
    const angle = position * Math.PI * 2 - Math.PI / 2;
    context.globalAlpha = 0.12 + value * 0.38;
    context.strokeStyle = paletteColor(colors, index, spokes);
    context.beginPath();
    context.moveTo(
      bounds.centerX + Math.cos(angle) * radius,
      bounds.centerY + Math.sin(angle) * radius,
    );
    context.lineTo(
      bounds.centerX + Math.cos(angle) * (radius + value * 70 * intensity),
      bounds.centerY + Math.sin(angle) * (radius + value * 70 * intensity),
    );
    context.stroke();
  }
  context.globalAlpha = 0.28;
  drawArtworkCover(frame, cover.x, cover.y, cover.size);
}

function drawColorExtraction(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const colors = extractedArtworkColors(frame, state);
  const bands = Math.max(3, colors.length * 2);
  for (let band = 0; band < bands; band += 1) {
    const value = energy(frame, band / bands, (band + 1) / bands);
    const width = bounds.width / bands;
    const sway = Math.sin(state.phase + band) * value * 24 * intensity;
    context.globalAlpha = 0.025 + value * 0.1;
    context.fillStyle = paletteColor(colors, band, bands);
    context.fillRect(bounds.left + band * width + sway, 0, width + 2, height);
  }
  const cover = artworkSquare(frame);
  context.globalAlpha = 0.12;
  drawArtworkCover(frame, cover.x, cover.y, cover.size);
}

function drawPixelSort(frame: VisualizationFrame) {
  const { context, intensity } = frame;
  const cover = artworkSquare(frame);
  const columns = 42;
  const columnWidth = cover.size / columns;
  for (let column = 0; column < columns; column += 1) {
    const value = frequencyValue(frame, column / columns, true);
    const stretch = 1 + value * 1.4 * intensity;
    const height = cover.size * stretch;
    const y = cover.y + cover.size / 2 - height / 2;
    context.save();
    context.beginPath();
    context.rect(cover.x + column * columnWidth, y, columnWidth + 1, height);
    context.clip();
    context.globalAlpha = 0.08 + value * 0.25;
    context.translate(0, y - cover.y);
    context.scale(1, stretch);
    drawArtworkCover(frame, cover.x, cover.y, cover.size);
    context.restore();
  }
}

function renderAnalytical(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "vu-needles") drawVuNeedles(frame);
  else if (mode === "digital-level-meter") drawDigitalMeters(frame);
  else if (mode === "phase-correlation-meter") drawCorrelation(frame);
  else if (mode === "goniometer") drawGoniometer(frame);
  else if (mode === "chromagram-ring") drawChromagram(frame);
  else if (mode === "bass-mid-treble-triptych") drawTriptych(frame);
  else if (mode === "frequency-labels") drawLabeledSpectrum(frame);
  else if (mode === "peak-history") drawPeakHistory(frame, state);
  else if (mode === "telemetry-matrix") drawTelemetry(frame, state);
  else drawScanner(frame, state);
}

function drawVuNeedles(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const width = Math.min(310, bounds.width * 0.34);
  const height = Math.min(180, frame.height * 0.28);
  (["left", "right"] as const).forEach((channel, index) => {
    const centerX = bounds.centerX + (index ? 0.54 : -0.54) * width;
    const centerY = bounds.centerY + height * 0.32;
    const level = waveformRms(frame, channel);
    context.globalAlpha = 0.28;
    context.strokeStyle = paletteColor(colors, index, 2);
    context.strokeRect(
      centerX - width / 2,
      centerY - height / 2,
      width,
      height,
    );
    for (let tick = 0; tick <= 10; tick += 1) {
      const angle = Math.PI * 0.8 + (tick / 10) * Math.PI * 0.4;
      context.globalAlpha = 0.12 + tick * 0.012;
      context.beginPath();
      context.moveTo(
        centerX + Math.cos(angle) * height * 0.56,
        centerY + Math.sin(angle) * height * 0.56,
      );
      context.lineTo(
        centerX + Math.cos(angle) * height * 0.48,
        centerY + Math.sin(angle) * height * 0.48,
      );
      context.stroke();
    }
    const needleAngle =
      Math.PI * 0.8 + Math.min(1, level * intensity) * Math.PI * 0.4;
    context.globalAlpha = 0.72;
    context.beginPath();
    context.moveTo(centerX, centerY);
    context.lineTo(
      centerX + Math.cos(needleAngle) * height * 0.54,
      centerY + Math.sin(needleAngle) * height * 0.54,
    );
    context.stroke();
  });
}

function drawDigitalMeters(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const segments = 32;
  const segmentWidth = Math.min(16, bounds.width / 48);
  const gap = 4;
  (["left", "right"] as const).forEach((channel, channelIndex) => {
    const level = waveformRms(frame, channel) * intensity;
    const x =
      bounds.centerX +
      (channelIndex ? 18 : -18) -
      (channelIndex ? 0 : segmentWidth);
    for (let segment = 0; segment < segments; segment += 1) {
      const active = segment / segments <= level;
      context.globalAlpha = active ? 0.24 + (segment / segments) * 0.52 : 0.055;
      context.fillStyle = paletteColor(colors, segment, segments);
      context.fillRect(
        x,
        bounds.centerY + (segments / 2 - segment) * (segmentWidth + gap),
        segmentWidth,
        segmentWidth,
      );
    }
  });
}

function drawCorrelation(frame: VisualizationFrame) {
  const { context, colors } = frame;
  const bounds = mainBounds(frame);
  const width = Math.min(640, bounds.width * 0.68);
  const correlation = stereoCorrelation(frame);
  const y = bounds.centerY;
  context.strokeStyle = colors[0] ?? "currentColor";
  context.globalAlpha = 0.24;
  context.beginPath();
  context.moveTo(bounds.centerX - width / 2, y);
  context.lineTo(bounds.centerX + width / 2, y);
  context.stroke();
  for (let tick = -10; tick <= 10; tick += 1) {
    const x = bounds.centerX + (tick / 20) * width;
    context.globalAlpha = tick === 0 ? 0.34 : 0.14;
    context.fillRect(
      x,
      y - (tick % 5 === 0 ? 12 : 7),
      1,
      tick % 5 === 0 ? 24 : 14,
    );
  }
  const markerX = bounds.centerX + (correlation / 2) * width;
  context.globalAlpha = 0.74;
  context.fillStyle = paletteColor(
    colors,
    Math.round((correlation + 1) * 5),
    10,
  );
  context.fillRect(markerX - 3, y - 22, 6, 44);
  context.font = "11px ui-monospace, monospace";
  context.textAlign = "center";
  context.fillText(correlation.toFixed(2), markerX, y - 34);
}

function drawGoniometer(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.min(bounds.width, frame.height) * 0.27 * intensity;
  context.globalAlpha = 0.11;
  context.strokeStyle = colors[0] ?? "currentColor";
  context.beginPath();
  context.moveTo(bounds.centerX - radius, bounds.centerY);
  context.lineTo(bounds.centerX + radius, bounds.centerY);
  context.moveTo(bounds.centerX, bounds.centerY - radius);
  context.lineTo(bounds.centerX, bounds.centerY + radius);
  context.stroke();
  context.globalAlpha = 0.58;
  context.strokeStyle = paletteGradient(
    context,
    colors,
    bounds.centerX - radius,
    0,
    bounds.centerX + radius,
    0,
  );
  context.beginPath();
  for (let index = 0; index < 220; index += 1) {
    const position = index / 219;
    const left = waveformValue(frame, position, "left");
    const right = waveformValue(frame, position, "right");
    const x = bounds.centerX + (left - right) * radius * 0.72;
    const y = bounds.centerY - (left + right) * radius * 0.72;
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.stroke();
}

function drawChromagram(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const values = chromaValues(frame);
  const radius = Math.min(bounds.width, frame.height) * 0.18;
  context.font = "10px ui-monospace, monospace";
  context.textAlign = "center";
  const labels = [
    "C",
    "C#",
    "D",
    "D#",
    "E",
    "F",
    "F#",
    "G",
    "G#",
    "A",
    "A#",
    "B",
  ];
  values.forEach((value, index) => {
    const angle = (index / 12) * Math.PI * 2 - Math.PI / 2;
    const outer = radius + value * radius * 0.78 * intensity;
    context.globalAlpha = 0.2 + value * 0.46;
    context.strokeStyle = paletteColor(colors, index, 12);
    context.beginPath();
    context.moveTo(
      bounds.centerX + Math.cos(angle) * radius,
      bounds.centerY + Math.sin(angle) * radius,
    );
    context.lineTo(
      bounds.centerX + Math.cos(angle) * outer,
      bounds.centerY + Math.sin(angle) * outer,
    );
    context.stroke();
    context.fillStyle = context.strokeStyle;
    context.fillText(
      labels[index] ?? "",
      bounds.centerX + Math.cos(angle) * (radius * 0.77),
      bounds.centerY + Math.sin(angle) * (radius * 0.77) + 3,
    );
  });
}

function chromaValues(frame: VisualizationFrame): number[] {
  const values = Array.from({ length: 12 }, () => 0);
  const counts = Array.from({ length: 12 }, () => 0);
  for (let bin = 2; bin < frame.frequency.length * 0.78; bin += 1) {
    const frequency = (bin * frame.sampleRate) / (frame.frequency.length * 2);
    if (frequency < 45 || frequency > 5000) continue;
    const midi = Math.round(69 + 12 * Math.log2(frequency / 440));
    const pitch = ((midi % 12) + 12) % 12;
    values[pitch] =
      (values[pitch] ?? 0) + (frame.live ? frame.frequency[bin]! / 255 : 0.1);
    counts[pitch] = (counts[pitch] ?? 0) + 1;
  }
  return values.map((value, index) => value / Math.max(counts[index] ?? 1, 1));
}

function drawTriptych(frame: VisualizationFrame) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const bands = [
    { label: "BASS", value: energy(frame, 0, 0.16) },
    { label: "MID", value: energy(frame, 0.16, 0.55) },
    { label: "TREBLE", value: energy(frame, 0.55, 1) },
  ];
  const gap = 18;
  const width = Math.min(260, (bounds.width - gap * 4) / 3);
  const boxHeight = Math.min(310, height * 0.46);
  context.font = "10px ui-monospace, monospace";
  context.textAlign = "left";
  bands.forEach((band, index) => {
    const x = bounds.centerX + (index - 1) * (width + gap) - width / 2;
    const y = bounds.centerY - boxHeight / 2;
    context.globalAlpha = 0.22;
    context.strokeStyle = paletteColor(colors, index, 3);
    context.strokeRect(x, y, width, boxHeight);
    context.globalAlpha = 0.08 + band.value * 0.24;
    context.fillStyle = context.strokeStyle;
    context.fillRect(
      x,
      y + boxHeight * (1 - Math.min(1, band.value * intensity)),
      width,
      boxHeight * Math.min(1, band.value * intensity),
    );
    context.globalAlpha = 0.64;
    context.fillText(band.label, x + 10, y + 18);
  });
}

function drawLabeledSpectrum(frame: VisualizationFrame) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const base = height * 0.72;
  const maxHeight = Math.min(height * 0.34, 300) * intensity;
  context.globalAlpha = 0.58;
  context.strokeStyle = paletteGradient(
    context,
    colors,
    bounds.left,
    0,
    bounds.left + bounds.width,
    0,
  );
  context.beginPath();
  for (let index = 0; index <= 120; index += 1) {
    const position = index / 120;
    const x = bounds.left + position * bounds.width;
    const y = base - frequencyValue(frame, position, true) * maxHeight;
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.stroke();
  const labels = [60, 120, 250, 500, 1000, 2000, 4000, 8000, 16000];
  context.font = "10px ui-monospace, monospace";
  context.textAlign = "center";
  labels.forEach((label) => {
    const normalized = Math.log10(label / 20) / Math.log10(20000 / 20);
    const x = bounds.left + normalized * bounds.width;
    context.globalAlpha = 0.14;
    context.fillStyle = colors[0] ?? "currentColor";
    context.fillRect(x, base - maxHeight, 1, maxHeight);
    context.globalAlpha = 0.48;
    context.fillText(
      label >= 1000 ? `${label / 1000}k` : String(label),
      x,
      base + 18,
    );
  });
}

function drawPeakHistory(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const base = height * 0.72;
  const values = state.peakHistory;
  context.fillStyle = paletteGradient(
    context,
    colors,
    bounds.left,
    0,
    bounds.left + bounds.width,
    0,
  );
  values.forEach((value, index) => {
    const width = bounds.width / Math.max(values.length, 1);
    context.globalAlpha =
      0.12 + (1 - index / Math.max(values.length, 1)) * 0.42;
    context.fillRect(
      bounds.left + bounds.width - (index + 1) * width,
      base - value * height * 0.34 * intensity,
      Math.max(1, width - 2),
      value * height * 0.34 * intensity,
    );
  });
}

function drawTelemetry(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const history = state.frequencyHistory.slice(0, 24);
  const columns = 24;
  const cellWidth = bounds.width / columns;
  const cellHeight = Math.min(
    15,
    (height * 0.52) / Math.max(history.length, 1),
  );
  const y0 = height * 0.22;
  history.forEach((values, row) => {
    for (let column = 0; column < columns; column += 1) {
      const source = Math.floor((column / columns) * values.length);
      const value = values[source] ?? 0.08;
      context.globalAlpha = 0.025 + value * 0.42 * intensity;
      context.fillStyle = paletteColor(
        colors,
        column + row,
        columns + history.length,
      );
      context.fillRect(
        bounds.left + column * cellWidth + 1,
        y0 + row * cellHeight + 1,
        Math.max(1, cellWidth - 3),
        Math.max(1, cellHeight - 3),
      );
    }
  });
  context.globalAlpha = 0.35;
  context.fillStyle = colors[0] ?? "currentColor";
  context.font = "10px ui-monospace, monospace";
  context.textAlign = "left";
  context.fillText("FREQUENCY / TIME", bounds.left, y0 - 10);
}

function drawScanner(frame: VisualizationFrame, state: VisualizationState) {
  drawLabeledSpectrum(frame);
  const { context, colors, height } = frame;
  const bounds = mainBounds(frame);
  const position = (state.phase * 0.18) % 1;
  const x = bounds.left + position * bounds.width;
  const value = frequencyValue(frame, position, true);
  context.globalAlpha = 0.68;
  context.strokeStyle = paletteColor(colors, Math.floor(position * 10), 10);
  context.beginPath();
  context.moveTo(x, height * 0.28);
  context.lineTo(x, height * 0.76);
  context.stroke();
  context.fillStyle = context.strokeStyle;
  context.font = "10px ui-monospace, monospace";
  context.textAlign = "center";
  context.fillText(`${Math.round(value * 100)}%`, x, height * 0.25);
}

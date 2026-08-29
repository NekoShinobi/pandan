import type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";
import {
  frequencyValue,
  mainBounds,
  paletteColor,
  paletteGradient,
  waveformValue,
  type VisualizationFrame,
  type VisualizationState,
} from "$lib/audio-visualization/core";

const FREQUENCY_MODES = new Set<AudioVisualizationMode>([
  "spectrum",
  "mirrored-spectrum",
  "stereo-split-spectrum",
  "led-equalizer",
  "octave-bands",
  "logarithmic-spectrum",
  "radial-spectrum",
  "spiral-spectrum",
  "spectrum-skyline",
  "waterfall-spectrogram",
  "frequency-heatmap",
  "spectrum-tunnel",
]);

const WAVE_MODES = new Set<AudioVisualizationMode>([
  "wave",
  "filled-waveform",
  "mirrored-waveform",
  "circular-waveform",
  "wave-ribbon",
  "stacked-echoes",
  "persistence-scope",
  "lissajous-scope",
  "waveform-tunnel",
  "seismograph",
  "braided-waves",
]);

export function renderFrequencyOrWave(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
): boolean {
  if (FREQUENCY_MODES.has(mode)) {
    renderFrequency(mode, frame, state);
    return true;
  }
  if (WAVE_MODES.has(mode)) {
    renderWave(mode, frame, state);
    return true;
  }
  return false;
}

function renderFrequency(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "radial-spectrum" || mode === "spiral-spectrum") {
    drawRadialSpectrum(frame, mode === "spiral-spectrum");
    return;
  }
  if (mode === "waterfall-spectrogram" || mode === "frequency-heatmap") {
    drawFrequencyHistory(frame, state, mode === "frequency-heatmap");
    return;
  }
  if (mode === "spectrum-tunnel") {
    drawSpectrumTunnel(frame, state);
    return;
  }
  if (mode === "spectrum-skyline") {
    drawSkyline(frame);
    return;
  }
  drawBars(frame, state, mode);
}

function drawBars(
  frame: VisualizationFrame,
  state: VisualizationState,
  mode: AudioVisualizationMode,
) {
  const { context, height, intensity, colors } = frame;
  const bounds = mainBounds(frame);
  const isOctave = mode === "octave-bands";
  const isLed = mode === "led-equalizer";
  const isLog = mode === "logarithmic-spectrum" || isOctave;
  const mirrored =
    mode === "mirrored-spectrum" || mode === "stereo-split-spectrum";
  const columns = isOctave
    ? Math.min(24, Math.max(12, Math.floor(bounds.width / 64)))
    : Math.min(76, Math.max(28, Math.floor(bounds.width / 18)));
  const gap = Math.max(3, bounds.width / columns / (isLed ? 4 : 6));
  const barWidth = bounds.width / columns - gap;
  const centerY = height * 0.56;
  const baseline = mirrored ? centerY : height * 0.73;
  const maxHeight =
    Math.min(height * (mirrored ? 0.22 : 0.34), 310) * intensity;
  context.globalAlpha = frame.live ? 0.7 : 0.28;

  for (let index = 0; index < columns; index += 1) {
    const position = index / Math.max(columns - 1, 1);
    const x = bounds.left + index * (barWidth + gap) + gap / 2;
    const left = frequencyValue(frame, position, isLog, "left");
    const right = frequencyValue(frame, position, isLog, "right");
    const value =
      mode === "stereo-split-spectrum" && frame.stereo
        ? Math.max(left, right)
        : frequencyValue(frame, position, isLog);
    const barHeight = 3 + value * maxHeight;
    context.fillStyle = paletteColor(colors, index, columns);

    if (isLed) {
      const segmentHeight = 5;
      const segments = Math.max(1, Math.floor(barHeight / (segmentHeight + 3)));
      for (let segment = 0; segment < segments; segment += 1) {
        context.globalAlpha =
          0.28 + (segment / Math.max(segments - 1, 1)) * 0.48;
        context.fillRect(
          x,
          baseline - segment * (segmentHeight + 3) - segmentHeight,
          barWidth,
          segmentHeight,
        );
      }
      const held = Math.max(state.heldPeaks[index] ?? 0, value);
      state.heldPeaks[index] = Math.max(value, held - frame.delta * 0.28);
      context.globalAlpha = 0.88;
      context.fillRect(
        x,
        baseline - (state.heldPeaks[index] ?? value) * maxHeight - 4,
        barWidth,
        2,
      );
      continue;
    }

    if (mode === "stereo-split-spectrum") {
      const leftHeight = 3 + left * maxHeight;
      const rightHeight = 3 + right * maxHeight;
      context.fillRect(x, centerY - leftHeight - 2, barWidth, leftHeight);
      context.globalAlpha = frame.live ? 0.42 : 0.18;
      context.fillRect(x, centerY + 2, barWidth, rightHeight);
      context.globalAlpha = frame.live ? 0.7 : 0.28;
    } else if (mirrored) {
      context.fillRect(x, baseline - barHeight - 2, barWidth, barHeight);
      context.globalAlpha = frame.live ? 0.34 : 0.14;
      context.fillRect(x, baseline + 2, barWidth, barHeight);
      context.globalAlpha = frame.live ? 0.7 : 0.28;
    } else {
      context.fillRect(x, baseline - barHeight, barWidth, barHeight);
      context.globalAlpha = frame.live ? 0.16 : 0.07;
      context.fillRect(x, baseline + 5, barWidth, barHeight * 0.24);
      context.globalAlpha = frame.live ? 0.7 : 0.28;
    }
  }
}

function drawRadialSpectrum(frame: VisualizationFrame, spiral: boolean) {
  const { context, colors, intensity, time } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.min(bounds.width, frame.height) * (spiral ? 0.1 : 0.18);
  const spokes = spiral ? 110 : 84;
  context.globalAlpha = frame.live ? 0.68 : 0.25;
  context.lineWidth = spiral ? 1.1 : 1.5;
  if (spiral) context.beginPath();
  for (let index = 0; index < spokes; index += 1) {
    const position = index / spokes;
    const value = frequencyValue(frame, position, true);
    const angle =
      position * Math.PI * (spiral ? 5.2 : 2) -
      Math.PI / 2 +
      (spiral ? time * 0.00008 : 0);
    const inner = spiral ? radius + position * radius * 1.6 : radius * 0.82;
    const outer = inner + value * radius * intensity * (spiral ? 0.7 : 0.82);
    const x0 = bounds.centerX + Math.cos(angle) * inner;
    const y0 = bounds.centerY + Math.sin(angle) * inner;
    const x1 = bounds.centerX + Math.cos(angle) * outer;
    const y1 = bounds.centerY + Math.sin(angle) * outer;
    if (spiral) {
      if (index === 0) context.moveTo(x1, y1);
      else context.lineTo(x1, y1);
    } else {
      context.strokeStyle = paletteColor(colors, index, spokes);
      context.beginPath();
      context.moveTo(x0, y0);
      context.lineTo(x1, y1);
      context.stroke();
    }
  }
  if (spiral) {
    context.strokeStyle = paletteGradient(
      context,
      colors,
      bounds.centerX - radius * 2,
      0,
      bounds.centerX + radius * 2,
      0,
    );
    context.stroke();
  }
}

function drawSkyline(frame: VisualizationFrame) {
  const { context, colors, intensity, height } = frame;
  const bounds = mainBounds(frame);
  const layers = 3;
  const base = height * 0.76;
  for (let layer = layers - 1; layer >= 0; layer -= 1) {
    const columns = 28 + layer * 9;
    const width = bounds.width / columns;
    context.globalAlpha = 0.16 + (layers - layer) * 0.12;
    context.fillStyle = paletteColor(colors, layer, layers);
    for (let index = 0; index < columns; index += 1) {
      const value = frequencyValue(frame, index / columns, true);
      const building =
        10 +
        value * Math.min(height * 0.3, 280) * intensity * (1 - layer * 0.14);
      context.fillRect(
        bounds.left + index * width + layer,
        base - building - layer * 14,
        Math.max(2, width - 5),
        building,
      );
    }
  }
}

function drawFrequencyHistory(
  frame: VisualizationFrame,
  state: VisualizationState,
  heatmap: boolean,
) {
  const { context, colors, height } = frame;
  const bounds = mainBounds(frame);
  const history = state.frequencyHistory;
  if (heatmap) {
    const rows = Math.max(history.length, 1);
    const cellWidth = bounds.width / 72;
    const cellHeight = Math.min(10, (height * 0.48) / rows);
    const startY = height * 0.25;
    for (let row = 0; row < rows; row += 1) {
      const values = history[row] ?? [];
      for (let column = 0; column < 72; column += 1) {
        const value = values[column] ?? 0.08;
        context.globalAlpha = 0.04 + value * 0.65 * frame.intensity;
        context.fillStyle = paletteColor(colors, column + row, 72 + rows);
        context.fillRect(
          bounds.left + column * cellWidth,
          startY + row * cellHeight,
          Math.ceil(cellWidth),
          Math.ceil(cellHeight),
        );
      }
    }
    return;
  }

  context.lineWidth = 1;
  history.forEach((values, row) => {
    const depth = row / Math.max(history.length - 1, 1);
    const scale = 1 - depth * 0.52;
    const rowWidth = bounds.width * scale;
    const left = bounds.centerX - rowWidth / 2;
    const baseline = height * 0.72 - row * 10;
    context.globalAlpha = (1 - depth) * 0.55 + 0.04;
    context.strokeStyle = paletteColor(colors, row, history.length);
    context.beginPath();
    values.forEach((value, index) => {
      const x = left + (index / Math.max(values.length - 1, 1)) * rowWidth;
      const y =
        baseline - value * Math.min(height * 0.17, 120) * frame.intensity;
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.stroke();
  });
}

function drawSpectrumTunnel(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity, height } = frame;
  const bounds = mainBounds(frame);
  const depthCount = Math.min(18, Math.max(8, state.frequencyHistory.length));
  for (let depth = depthCount - 1; depth >= 0; depth -= 1) {
    const amount = depth / Math.max(depthCount - 1, 1);
    const size = 0.16 + (1 - amount) * 0.68;
    const values = state.frequencyHistory[depth] ?? [];
    const pulse = values.length
      ? values.reduce((sum, value) => sum + value, 0) / values.length
      : 0.1;
    const width = bounds.width * size * (1 + pulse * 0.08 * intensity);
    const boxHeight = height * size * 0.62;
    context.globalAlpha = 0.08 + (1 - amount) * 0.44;
    context.strokeStyle = paletteColor(colors, depth, depthCount);
    context.lineWidth = 1;
    context.strokeRect(
      bounds.centerX - width / 2,
      bounds.centerY - boxHeight / 2,
      width,
      boxHeight,
    );
  }
}

function renderWave(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "circular-waveform") {
    drawCircularWave(frame);
  } else if (mode === "stacked-echoes" || mode === "persistence-scope") {
    drawWaveHistory(frame, state, mode === "persistence-scope");
  } else if (mode === "lissajous-scope") {
    drawLissajous(frame);
  } else if (mode === "waveform-tunnel") {
    drawWaveTunnel(frame, state);
  } else if (mode === "seismograph") {
    drawSeismograph(frame, state);
  } else if (mode === "braided-waves") {
    drawBraidedWaves(frame);
  } else {
    drawLinearWave(frame, mode);
  }
}

function drawLinearWave(
  frame: VisualizationFrame,
  mode: AudioVisualizationMode,
) {
  const { context, height, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const center = height * 0.56;
  const amplitude = Math.min(height * 0.2, 190) * intensity;
  const filled = mode === "filled-waveform";
  const mirrored = mode === "mirrored-waveform";
  const ribbon = mode === "wave-ribbon";
  const layers = ribbon ? 5 : 1;
  for (let layer = layers - 1; layer >= 0; layer -= 1) {
    const offset = ribbon ? (layer - 2) * 8 : 0;
    context.globalAlpha = ribbon
      ? 0.1 + (layers - layer) * 0.09
      : frame.live
        ? 0.7
        : 0.25;
    context.strokeStyle = paletteColor(colors, layer, layers);
    context.fillStyle = paletteColor(colors, layer, layers);
    context.lineWidth = layer === 0 ? 1.6 : 1;
    context.beginPath();
    for (let index = 0; index < 192; index += 1) {
      const position = index / 191;
      const x = bounds.left + position * bounds.width;
      const y = center + offset + waveformValue(frame, position) * amplitude;
      if (index === 0) context.moveTo(x, filled ? center : y);
      if (index === 0 && filled) context.lineTo(x, y);
      else if (index > 0) context.lineTo(x, y);
    }
    if (filled) {
      context.lineTo(bounds.left + bounds.width, center);
      context.closePath();
      context.globalAlpha *= 0.48;
      context.fill();
    } else {
      context.stroke();
    }
    if (mirrored) {
      context.beginPath();
      for (let index = 0; index < 192; index += 1) {
        const position = index / 191;
        const x = bounds.left + position * bounds.width;
        const y = center - waveformValue(frame, position) * amplitude;
        if (index === 0) context.moveTo(x, y);
        else context.lineTo(x, y);
      }
      context.stroke();
    }
  }
}

function drawCircularWave(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.min(bounds.width, frame.height) * 0.18;
  context.globalAlpha = frame.live ? 0.68 : 0.25;
  context.strokeStyle = paletteGradient(
    context,
    colors,
    bounds.centerX - radius,
    0,
    bounds.centerX + radius,
    0,
  );
  context.lineWidth = 1.5;
  context.beginPath();
  for (let index = 0; index <= 192; index += 1) {
    const position = index / 192;
    const angle = position * Math.PI * 2 - Math.PI / 2;
    const distance =
      radius + waveformValue(frame, position) * radius * 0.52 * intensity;
    const x = bounds.centerX + Math.cos(angle) * distance;
    const y = bounds.centerY + Math.sin(angle) * distance;
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.closePath();
  context.stroke();
}

function drawWaveHistory(
  frame: VisualizationFrame,
  state: VisualizationState,
  persistence: boolean,
) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  state.waveformHistory.forEach((values, row) => {
    const depth = row / Math.max(state.waveformHistory.length - 1, 1);
    const center = persistence ? height * 0.56 : height * 0.68 - row * 14;
    context.globalAlpha = persistence ? (1 - depth) * 0.18 : (1 - depth) * 0.45;
    context.strokeStyle = paletteColor(
      colors,
      row,
      state.waveformHistory.length,
    );
    context.lineWidth = persistence && row === 0 ? 2 : 1;
    context.beginPath();
    values.forEach((value, index) => {
      const x =
        bounds.left + (index / Math.max(values.length - 1, 1)) * bounds.width;
      const y = center + value * Math.min(height * 0.18, 170) * intensity;
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.stroke();
  });
}

function drawLissajous(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.min(bounds.width, frame.height) * 0.26 * intensity;
  context.globalAlpha = frame.live ? 0.62 : 0.24;
  context.strokeStyle = paletteGradient(
    context,
    colors,
    bounds.centerX - radius,
    bounds.centerY,
    bounds.centerX + radius,
    bounds.centerY,
  );
  context.lineWidth = 1.35;
  context.beginPath();
  for (let index = 0; index < 192; index += 1) {
    const position = index / 191;
    const x = bounds.centerX + waveformValue(frame, position, "left") * radius;
    const y = bounds.centerY + waveformValue(frame, position, "right") * radius;
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.stroke();
}

function drawWaveTunnel(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  state.waveformHistory.slice(0, 12).forEach((values, row) => {
    const scale = 1 - row * 0.055;
    const radius = Math.min(bounds.width, frame.height) * 0.24 * scale;
    context.globalAlpha = (1 - row / 12) * 0.42;
    context.strokeStyle = paletteColor(colors, row, 12);
    context.beginPath();
    values.forEach((value, index) => {
      const angle = (index / values.length) * Math.PI * 2;
      const distance = radius + value * radius * 0.38 * intensity;
      const x = bounds.centerX + Math.cos(angle) * distance;
      const y = bounds.centerY + Math.sin(angle) * distance * 0.64;
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.closePath();
    context.stroke();
  });
}

function drawSeismograph(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const values = state.peakHistory;
  const center = height * 0.58;
  context.globalAlpha = 0.62;
  context.strokeStyle = paletteGradient(
    context,
    colors,
    bounds.left,
    0,
    bounds.left + bounds.width,
    0,
  );
  context.beginPath();
  values.forEach((value, index) => {
    const x =
      bounds.left +
      bounds.width -
      (index / Math.max(values.length - 1, 1)) * bounds.width;
    const polarity = index % 2 === 0 ? 1 : -1;
    const y =
      center + polarity * value * Math.min(height * 0.2, 170) * intensity;
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  });
  context.stroke();
  context.globalAlpha = 0.15;
  context.fillStyle = colors[0] ?? "currentColor";
  for (let line = -3; line <= 3; line += 1) {
    context.fillRect(bounds.left, center + line * 22, bounds.width, 1);
  }
}

function drawBraidedWaves(frame: VisualizationFrame) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const center = height * 0.56;
  const amplitude = Math.min(height * 0.19, 170) * intensity;
  (["left", "right"] as const).forEach((channel, channelIndex) => {
    context.globalAlpha = channelIndex === 0 ? 0.68 : 0.44;
    context.strokeStyle = paletteColor(colors, channelIndex, 2);
    context.lineWidth = 1.5;
    context.beginPath();
    for (let index = 0; index < 192; index += 1) {
      const position = index / 191;
      const x = bounds.left + position * bounds.width;
      const y =
        center +
        waveformValue(frame, position, channel) * amplitude +
        (channelIndex ? 7 : -7);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.stroke();
  });
}

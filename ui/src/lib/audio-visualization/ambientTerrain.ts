import type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";
import {
  deterministic,
  energy,
  frequencyValue,
  mainBounds,
  paletteColor,
  paletteGradient,
  radialGradient,
  type VisualizationFrame,
  type VisualizationState,
} from "$lib/audio-visualization/core";

const AMBIENT_MODES = new Set<AudioVisualizationMode>([
  "aurora-bands",
  "prismatic-rays",
  "plasma-field",
  "fluid-ink",
  "metaballs",
  "nebula",
  "audio-tunnel",
  "moire-interference",
  "reaction-diffusion",
  "chromatic-fog",
  "electric-field",
  "liquid-lens",
  "heat-distortion",
  "topographic-flow",
]);

const TERRAIN_MODES = new Set<AudioVisualizationMode>([
  "frequency-terrain",
  "wireframe-mountains",
  "voxel-equalizer",
  "audio-city",
  "data-canyon",
  "reactive-horizon",
  "circuit-field",
  "sonar-landscape",
]);

export function renderAmbientOrTerrain(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
): boolean {
  if (AMBIENT_MODES.has(mode)) {
    renderAmbient(mode, frame, state);
    return true;
  }
  if (TERRAIN_MODES.has(mode)) {
    renderTerrain(mode, frame, state);
    return true;
  }
  return false;
}

function renderAmbient(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "aurora-bands") drawAurora(frame, state);
  else if (mode === "prismatic-rays") drawRays(frame, state);
  else if (mode === "plasma-field") drawPlasma(frame, state);
  else if (mode === "fluid-ink") drawFluidInk(frame, state);
  else if (mode === "metaballs") drawMetaballs(frame, state);
  else if (mode === "nebula") drawNebula(frame, state);
  else if (mode === "audio-tunnel") drawAudioTunnel(frame, state);
  else if (mode === "moire-interference") drawMoire(frame, state);
  else if (mode === "reaction-diffusion") drawReactionDiffusion(frame, state);
  else if (mode === "chromatic-fog") drawFog(frame, state);
  else if (mode === "electric-field") drawElectricField(frame, state);
  else if (mode === "liquid-lens") drawLiquidLens(frame, state);
  else if (mode === "heat-distortion") drawHeat(frame, state);
  else drawTopographicFlow(frame, state);
}

function drawAurora(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  for (let band = 0; band < 6; band += 1) {
    const gradient = context.createLinearGradient(
      bounds.left,
      0,
      bounds.left + bounds.width,
      0,
    );
    const color = paletteColor(colors, band, 6);
    gradient.addColorStop(0, "transparent");
    gradient.addColorStop(0.22, color);
    gradient.addColorStop(0.72, color);
    gradient.addColorStop(1, "transparent");
    context.globalAlpha =
      0.045 + energy(frame, band / 7, (band + 1) / 7) * 0.12;
    context.strokeStyle = gradient;
    context.lineWidth = 18 + band * 8;
    context.beginPath();
    for (let index = 0; index <= 72; index += 1) {
      const position = index / 72;
      const x = bounds.left + position * bounds.width;
      const signal = frequencyValue(frame, position, true);
      const y =
        height * (0.24 + band * 0.075) +
        Math.sin(
          position * Math.PI * (2.1 + band * 0.12) + state.phase + band,
        ) *
          (34 + signal * 80 * intensity);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.stroke();
  }
}

function drawRays(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const sourceX = bounds.centerX;
  const sourceY = height * 0.78;
  const rays = 62;
  for (let index = 0; index < rays; index += 1) {
    const position = index / (rays - 1);
    const value = frequencyValue(frame, position, true);
    const angle =
      -Math.PI * 0.92 +
      position * Math.PI * 0.84 +
      Math.sin(state.phase) * 0.03;
    const length = height * (0.55 + value * 0.34 * intensity);
    context.globalAlpha = 0.02 + value * 0.2;
    context.strokeStyle = paletteColor(colors, index, rays);
    context.lineWidth = 1 + value * 2;
    context.beginPath();
    context.moveTo(sourceX, sourceY);
    context.lineTo(
      sourceX + Math.cos(angle) * length,
      sourceY + Math.sin(angle) * length,
    );
    context.stroke();
  }
}

function drawPlasma(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const cell = Math.max(30, Math.min(64, bounds.width / 18));
  const columns = Math.ceil(bounds.width / cell);
  const rows = Math.ceil(frame.height / cell);
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      const position = (column + row * 0.6) / Math.max(columns + rows * 0.6, 1);
      const value = frequencyValue(frame, position % 1, true);
      const phase =
        Math.sin(column * 0.55 + state.phase) +
        Math.cos(row * 0.7 - state.phase * 0.8);
      context.globalAlpha =
        0.025 + (phase + 2) * 0.018 + value * 0.11 * intensity;
      context.fillStyle = paletteColor(colors, column + row, columns + rows);
      context.fillRect(
        bounds.left + column * cell,
        row * cell,
        cell + 1,
        cell + 1,
      );
    }
  }
}

function drawFluidInk(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  context.globalCompositeOperation = "screen";
  for (let contour = 0; contour < 9; contour += 1) {
    const bass = energy(frame, 0, 0.26);
    context.globalAlpha = 0.035 + bass * 0.045;
    context.fillStyle = paletteColor(colors, contour, 9);
    context.beginPath();
    context.moveTo(bounds.left, height);
    for (let index = 0; index <= 64; index += 1) {
      const position = index / 64;
      const x = bounds.left + position * bounds.width;
      const wave = Math.sin(
        position * Math.PI * (2 + contour * 0.15) +
          state.phase * (0.6 + contour * 0.04),
      );
      const signal = frequencyValue(frame, position, true);
      const y =
        height * (0.42 + contour * 0.035) +
        wave * (45 + signal * 100 * intensity);
      context.lineTo(x, y);
    }
    context.lineTo(bounds.left + bounds.width, height);
    context.closePath();
    context.fill();
  }
  context.globalCompositeOperation = "source-over";
}

function drawMetaballs(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  context.globalCompositeOperation = "screen";
  for (let index = 0; index < 12; index += 1) {
    const value = frequencyValue(frame, index / 12, true);
    const x =
      bounds.centerX +
      Math.cos(state.phase * (0.32 + index * 0.015) + index) *
        bounds.width *
        (0.1 + (index % 4) * 0.045);
    const y =
      bounds.centerY +
      Math.sin(state.phase * (0.25 + index * 0.02) + index * 1.7) *
        frame.height *
        0.2;
    const radius = 36 + value * 120 * intensity;
    context.globalAlpha = 0.055 + value * 0.09;
    context.fillStyle = radialGradient(
      context,
      [paletteColor(colors, index, 12)],
      x,
      y,
      radius,
    );
    context.beginPath();
    context.arc(x, y, radius, 0, Math.PI * 2);
    context.fill();
  }
  context.globalCompositeOperation = "source-over";
}

function drawNebula(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  context.globalCompositeOperation = "screen";
  for (let index = 0; index < 34; index += 1) {
    const value = frequencyValue(frame, index / 34, true);
    const x =
      bounds.left +
      deterministic(index, 21) * bounds.width +
      Math.sin(state.phase + index) * 8;
    const y = deterministic(index, 22) * frame.height;
    const radius = 12 + deterministic(index, 23) * 54 + value * 28 * intensity;
    context.globalAlpha = 0.018 + value * 0.065;
    context.fillStyle = radialGradient(
      context,
      [paletteColor(colors, index, 34)],
      x,
      y,
      radius,
    );
    context.beginPath();
    context.arc(x, y, radius, 0, Math.PI * 2);
    context.fill();
  }
  context.globalCompositeOperation = "source-over";
}

function drawAudioTunnel(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.18);
  for (let ring = 0; ring < 24; ring += 1) {
    const travel = (ring / 24 + state.phase * 0.08) % 1;
    const radius =
      18 + Math.pow(travel, 1.8) * Math.min(bounds.width, frame.height) * 0.48;
    const wobble = Math.sin(ring * 0.7 + state.phase) * 12 * travel * intensity;
    context.globalAlpha = (1 - travel) * 0.08 + travel * 0.32;
    context.strokeStyle = paletteColor(colors, ring, 24);
    context.lineWidth = 1 + bass * 1.5;
    context.beginPath();
    context.ellipse(
      bounds.centerX + wobble,
      bounds.centerY,
      radius * (1 + bass * 0.12 * intensity),
      radius * 0.62,
      state.phase * 0.025,
      0,
      Math.PI * 2,
    );
    context.stroke();
  }
}

function drawMoire(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.24);
  context.strokeStyle = colors[0] ?? "currentColor";
  context.lineWidth = 1;
  for (let field = 0; field < 2; field += 1) {
    context.save();
    context.translate(bounds.centerX, bounds.centerY);
    context.rotate((field ? -1 : 1) * (0.12 + state.phase * 0.025));
    for (let line = -34; line <= 34; line += 1) {
      context.globalAlpha = 0.045 + bass * 0.035;
      const offset = line * (9 + bass * 3 * intensity);
      context.beginPath();
      context.moveTo(
        -bounds.width * 0.6,
        offset + Math.sin(line + state.phase) * 8,
      );
      context.lineTo(
        bounds.width * 0.6,
        offset - Math.sin(line + state.phase) * 8,
      );
      context.stroke();
    }
    context.restore();
  }
}

function drawReactionDiffusion(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const spacing = Math.max(20, Math.min(34, bounds.width / 36));
  const columns = Math.ceil(bounds.width / spacing);
  const rows = Math.ceil(frame.height / spacing);
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      const phase =
        Math.sin(column * 1.32 + state.phase) *
        Math.cos(row * 1.15 - state.phase * 0.8);
      const value = frequencyValue(
        frame,
        ((column + row) % columns) / columns,
        true,
      );
      if (phase + value * intensity < 0.35) continue;
      const radius = 1.5 + (phase + 1) * 2.4 + value * 4;
      context.globalAlpha = 0.04 + value * 0.16;
      context.strokeStyle = paletteColor(colors, column + row, columns + rows);
      context.beginPath();
      context.arc(
        bounds.left + column * spacing,
        row * spacing,
        radius,
        0,
        Math.PI * 2,
      );
      context.stroke();
    }
  }
}

function drawFog(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  context.globalCompositeOperation = "screen";
  for (let index = 0; index < 18; index += 1) {
    const band = frequencyValue(frame, index / 18, true);
    const x =
      bounds.left +
      ((deterministic(index, 31) + state.phase * (0.006 + index * 0.0002)) %
        1) *
        bounds.width;
    const y = frame.height * (0.2 + deterministic(index, 32) * 0.62);
    const radius = 80 + deterministic(index, 33) * 130 + band * 70 * intensity;
    context.globalAlpha = 0.018 + band * 0.035;
    context.fillStyle = radialGradient(
      context,
      [paletteColor(colors, index, 18)],
      x,
      y,
      radius,
    );
    context.beginPath();
    context.ellipse(x, y, radius, radius * 0.42, 0, 0, Math.PI * 2);
    context.fill();
  }
  context.globalCompositeOperation = "source-over";
}

function drawElectricField(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const poles = [
    { x: bounds.centerX - bounds.width * 0.19, y: bounds.centerY },
    { x: bounds.centerX + bounds.width * 0.19, y: bounds.centerY },
  ];
  const bass = energy(frame, 0, 0.22);
  for (let line = 0; line < 38; line += 1) {
    const offset = (line / 37 - 0.5) * Math.PI;
    context.globalAlpha = 0.08 + bass * 0.13;
    context.strokeStyle = paletteColor(colors, line, 38);
    context.beginPath();
    for (let step = 0; step <= 42; step += 1) {
      const position = step / 42;
      const x = poles[0]!.x + (poles[1]!.x - poles[0]!.x) * position;
      const arch =
        Math.sin(position * Math.PI) * Math.sin(offset) * frame.height * 0.28;
      const jitter =
        Math.sin(step * 1.7 + state.phase + line) * bass * 6 * intensity;
      const y = bounds.centerY + arch + jitter;
      if (step === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.stroke();
  }
}

function drawLiquidLens(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.2);
  for (let ring = 0; ring < 17; ring += 1) {
    const position = ring / 16;
    const radius = 18 + position * Math.min(bounds.width, frame.height) * 0.32;
    const wobble =
      Math.sin(position * Math.PI * 4 + state.phase) * bass * 14 * intensity;
    context.globalAlpha = 0.06 + (1 - position) * 0.16;
    context.strokeStyle = paletteColor(colors, ring, 17);
    context.beginPath();
    context.ellipse(
      bounds.centerX + wobble,
      bounds.centerY,
      radius * (1 + bass * 0.08),
      radius * (0.76 - bass * 0.05),
      state.phase * 0.04,
      0,
      Math.PI * 2,
    );
    context.stroke();
  }
}

function drawHeat(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  for (let row = 0; row < 62; row += 1) {
    const y = (row / 61) * frame.height;
    const value = frequencyValue(frame, row / 61, true);
    const offset =
      Math.sin(row * 0.42 + state.phase * 1.4) * value * 28 * intensity;
    context.globalAlpha = 0.025 + value * 0.13;
    context.strokeStyle = paletteColor(colors, row, 62);
    context.beginPath();
    context.moveTo(bounds.left + offset, y);
    context.lineTo(bounds.left + bounds.width + offset, y);
    context.stroke();
  }
}

function drawTopographicFlow(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  for (let contour = 0; contour < 30; contour += 1) {
    const yBase = (contour / 29) * frame.height;
    context.globalAlpha = 0.055 + (contour % 5 === 0 ? 0.12 : 0);
    context.strokeStyle = paletteColor(colors, contour, 30);
    context.beginPath();
    for (let index = 0; index <= 64; index += 1) {
      const position = index / 64;
      const x = bounds.left + position * bounds.width;
      const value = frequencyValue(
        frame,
        (position + contour * 0.03) % 1,
        true,
      );
      const y =
        yBase +
        Math.sin(position * Math.PI * 3 + state.phase + contour * 0.27) *
          (5 + value * 30 * intensity);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.stroke();
  }
}

function renderTerrain(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "frequency-terrain") drawFrequencyTerrain(frame, state);
  else if (mode === "wireframe-mountains") drawMountains(frame, state);
  else if (mode === "voxel-equalizer") drawVoxels(frame);
  else if (mode === "audio-city") drawCity(frame);
  else if (mode === "data-canyon") drawCanyon(frame, state);
  else if (mode === "reactive-horizon") drawHorizon(frame, state);
  else if (mode === "circuit-field") drawCircuit(frame, state);
  else drawSonarLandscape(frame, state);
}

function drawFrequencyTerrain(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  state.frequencyHistory.slice(0, 24).forEach((values, row) => {
    const depth = row / 23;
    const scale = 1 - depth * 0.58;
    const rowWidth = bounds.width * scale;
    const left = bounds.centerX - rowWidth / 2;
    const baseline = height * 0.82 - row * 13;
    context.globalAlpha = 0.08 + (1 - depth) * 0.42;
    context.strokeStyle = paletteColor(colors, row, 24);
    context.beginPath();
    values.forEach((value, index) => {
      const x = left + (index / Math.max(values.length - 1, 1)) * rowWidth;
      const y = baseline - value * height * 0.14 * intensity;
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.stroke();
  });
}

function drawMountains(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  for (let ridge = 0; ridge < 9; ridge += 1) {
    const baseline = height * (0.77 - ridge * 0.045);
    context.globalAlpha = 0.12 + (8 - ridge) * 0.035;
    context.strokeStyle = paletteColor(colors, ridge, 9);
    context.beginPath();
    for (let index = 0; index <= 72; index += 1) {
      const position = index / 72;
      const x = bounds.left + position * bounds.width;
      const signal = frequencyValue(frame, (position + ridge * 0.04) % 1, true);
      const mountain = Math.abs(
        Math.sin(position * Math.PI * (3 + ridge * 0.18) + state.phase * 0.2),
      );
      const y = baseline - mountain * signal * height * 0.24 * intensity;
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.stroke();
  }
}

function drawVoxels(frame: VisualizationFrame) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const columns = Math.min(24, Math.max(12, Math.floor(bounds.width / 65)));
  const block = Math.min(24, bounds.width / columns / 2.2);
  for (let index = 0; index < columns; index += 1) {
    const value = frequencyValue(frame, index / columns, true);
    const levels = Math.max(1, Math.floor(value * 12 * intensity));
    for (let level = 0; level < levels; level += 1) {
      const x =
        bounds.centerX +
        (index - columns / 2) * block * 1.9 -
        level * block * 0.35;
      const y =
        height * 0.75 -
        level * block * 0.72 +
        (index - columns / 2) * block * 0.22;
      context.globalAlpha = 0.16 + (level / Math.max(levels, 1)) * 0.38;
      context.fillStyle = paletteColor(colors, index + level, columns + levels);
      context.fillRect(x, y, block * 1.5, block * 0.64);
      context.globalAlpha *= 0.55;
      context.fillRect(
        x + block * 1.5,
        y - block * 0.34,
        block * 0.42,
        block * 0.64,
      );
    }
  }
}

function drawCity(frame: VisualizationFrame) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const rows = 7;
  for (let row = rows - 1; row >= 0; row -= 1) {
    const depth = row / rows;
    const columns = 12 + row * 4;
    const rowWidth = bounds.width * (0.28 + depth * 0.72);
    const left = bounds.centerX - rowWidth / 2;
    const cell = rowWidth / columns;
    const base = height * (0.44 + depth * 0.42);
    for (let column = 0; column < columns; column += 1) {
      const value = frequencyValue(frame, column / columns, true);
      const building = 5 + value * (38 + depth * 82) * intensity;
      context.globalAlpha = 0.08 + depth * 0.28;
      context.fillStyle = paletteColor(colors, column + row, columns + rows);
      context.fillRect(
        left + column * cell + 1,
        base - building,
        Math.max(2, cell - 4),
        building,
      );
    }
  }
}

function drawCanyon(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  for (let depth = 0; depth < 20; depth += 1) {
    const amount = depth / 19;
    const y = height * 0.32 + amount * height * 0.55;
    const width = bounds.width * (0.08 + amount * 0.42);
    const signal = frequencyValue(
      frame,
      (amount + state.phase * 0.02) % 1,
      true,
    );
    const wall = signal * height * 0.15 * intensity;
    context.globalAlpha = 0.07 + amount * 0.3;
    context.strokeStyle = paletteColor(colors, depth, 20);
    context.beginPath();
    context.moveTo(bounds.centerX - width, y - wall);
    context.lineTo(bounds.centerX - width * 0.72, y);
    context.lineTo(bounds.centerX + width * 0.72, y);
    context.lineTo(bounds.centerX + width, y - wall);
    context.stroke();
  }
}

function drawHorizon(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const center = height * 0.55;
  context.strokeStyle = paletteGradient(
    context,
    colors,
    bounds.left,
    0,
    bounds.left + bounds.width,
    0,
  );
  context.globalAlpha = 0.58;
  context.lineWidth = 1.5;
  context.beginPath();
  for (let index = 0; index <= 120; index += 1) {
    const position = index / 120;
    const x = bounds.left + position * bounds.width;
    const value = frequencyValue(frame, position, true);
    const y =
      center +
      Math.sin(position * Math.PI * 4 + state.phase) *
        (8 + value * 72 * intensity);
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.stroke();
  context.globalAlpha = 0.08;
  for (let row = 1; row <= 10; row += 1) {
    context.beginPath();
    context.moveTo(bounds.left, center + row * 18);
    context.lineTo(bounds.left + bounds.width, center + row * 18);
    context.stroke();
  }
}

function drawCircuit(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const paths = 34;
  context.lineWidth = 1;
  for (let index = 0; index < paths; index += 1) {
    const value = frequencyValue(frame, index / paths, true);
    const y = (index / paths) * frame.height;
    const x0 = bounds.left + deterministic(index, 41) * bounds.width * 0.2;
    const x1 =
      bounds.left + bounds.width * (0.28 + deterministic(index, 42) * 0.22);
    const x2 =
      bounds.left + bounds.width * (0.58 + deterministic(index, 43) * 0.18);
    context.globalAlpha = 0.05 + value * 0.32;
    context.strokeStyle = paletteColor(colors, index, paths);
    context.beginPath();
    context.moveTo(x0, y);
    context.lineTo(x1, y);
    context.lineTo(x1, y + Math.sin(state.phase + index) * 20 * intensity);
    context.lineTo(x2, y + Math.sin(state.phase + index) * 20 * intensity);
    context.lineTo(
      bounds.left + bounds.width,
      y + (deterministic(index, 44) - 0.5) * 60,
    );
    context.stroke();
    if (value > 0.24) {
      context.fillStyle = context.strokeStyle;
      context.fillRect(x2 - 2, y - 2, 4, 4);
    }
  }
}

function drawSonarLandscape(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  drawFrequencyTerrain(frame, state);
  const { context, colors } = frame;
  const bounds = mainBounds(frame);
  const maxRadius = Math.min(bounds.width, frame.height) * 0.4;
  for (let ring = 0; ring < 5; ring += 1) {
    const travel = (ring / 5 + state.phase * 0.12) % 1;
    context.globalAlpha = (1 - travel) * 0.22;
    context.strokeStyle = paletteColor(colors, ring, 5);
    context.beginPath();
    context.ellipse(
      bounds.centerX,
      frame.height * 0.72,
      maxRadius * travel,
      maxRadius * travel * 0.34,
      0,
      0,
      Math.PI * 2,
    );
    context.stroke();
  }
}

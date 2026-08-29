import type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";
import {
  deterministic,
  energy,
  frequencyValue,
  mainBounds,
  paletteColor,
  paletteGradient,
  waveformValue,
  type VisualParticle,
  type VisualizationFrame,
  type VisualizationState,
} from "$lib/audio-visualization/core";

const GEOMETRY_MODES = new Set<AudioVisualizationMode>([
  "orbit",
  "starburst",
  "concentric-pulse",
  "orbital-rings",
  "polygon-morph",
  "reactive-grid",
  "hex-field",
  "radar-sweep",
  "mandala",
  "kaleidoscope",
  "spirograph",
  "wireframe-sphere",
]);

const PARTICLE_MODES = new Set<AudioVisualizationMode>([
  "particle-constellation",
  "particle-fountain",
  "particle-vortex",
  "firefly-field",
  "audio-rain",
  "dust-waves",
  "magnetic-swarm",
  "point-cloud-sphere",
  "frequency-comets",
  "reactive-snow",
]);

export function renderGeometryOrParticles(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
): boolean {
  if (GEOMETRY_MODES.has(mode)) {
    renderGeometry(mode, frame, state);
    return true;
  }
  if (PARTICLE_MODES.has(mode)) {
    renderParticles(mode, frame, state);
    return true;
  }
  return false;
}

function renderGeometry(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "orbit" || mode === "starburst") {
    drawRadialSpokes(frame, mode === "starburst");
  } else if (mode === "concentric-pulse") {
    drawConcentricPulse(frame, state);
  } else if (mode === "orbital-rings") {
    drawOrbitalRings(frame, state);
  } else if (mode === "polygon-morph") {
    drawPolygonMorph(frame);
  } else if (mode === "reactive-grid") {
    drawReactiveGrid(frame, state);
  } else if (mode === "hex-field") {
    drawHexField(frame);
  } else if (mode === "radar-sweep") {
    drawRadar(frame, state);
  } else if (mode === "mandala" || mode === "kaleidoscope") {
    drawSymmetry(frame, mode === "kaleidoscope");
  } else if (mode === "spirograph") {
    drawSpirograph(frame, state);
  } else {
    drawWireSphere(frame, state);
  }
}

function drawRadialSpokes(frame: VisualizationFrame, starburst: boolean) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius =
    Math.min(bounds.width, frame.height) * (starburst ? 0.12 : 0.19);
  const spokes = starburst ? 48 : 76;
  context.lineWidth = starburst ? 1.1 : 1.4;
  for (let index = 0; index < spokes; index += 1) {
    const position = index / spokes;
    const value = frequencyValue(frame, position, true);
    const shaped = starburst ? Math.pow(value, 1.6) : value;
    const angle = position * Math.PI * 2 - Math.PI / 2;
    const inner = starburst ? radius * 0.2 : radius * 0.82;
    const outer =
      radius + shaped * radius * intensity * (starburst ? 1.8 : 0.74);
    context.globalAlpha = (frame.live ? 0.34 : 0.18) + shaped * 0.48;
    context.strokeStyle = paletteColor(colors, index, spokes);
    context.beginPath();
    context.moveTo(
      bounds.centerX + Math.cos(angle) * inner,
      bounds.centerY + Math.sin(angle) * inner,
    );
    context.lineTo(
      bounds.centerX + Math.cos(angle) * outer,
      bounds.centerY + Math.sin(angle) * outer,
    );
    context.stroke();
  }
}

function drawConcentricPulse(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.16);
  const base = Math.min(bounds.width, frame.height) * 0.08;
  for (let index = 0; index < 12; index += 1) {
    const travel = (index / 12 + state.phase * 0.14) % 1;
    const radius =
      base +
      travel *
        Math.min(bounds.width, frame.height) *
        0.35 *
        (1 + bass * 0.16 * intensity);
    context.globalAlpha = (1 - travel) * 0.5;
    context.strokeStyle = paletteColor(colors, index, 12);
    context.lineWidth = 1 + bass * 2;
    context.beginPath();
    context.arc(bounds.centerX, bounds.centerY, radius, 0, Math.PI * 2);
    context.stroke();
  }
}

function drawOrbitalRings(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.2);
  const radius =
    Math.min(bounds.width, frame.height) * 0.22 * (1 + bass * 0.2 * intensity);
  context.save();
  context.translate(bounds.centerX, bounds.centerY);
  context.rotate(state.phase * 0.16);
  for (let index = 0; index < 7; index += 1) {
    context.save();
    context.rotate((index / 7) * Math.PI);
    context.globalAlpha = 0.16 + index * 0.055;
    context.strokeStyle = paletteColor(colors, index, 7);
    context.lineWidth = 1.2;
    context.beginPath();
    context.ellipse(
      0,
      0,
      radius,
      radius * (0.18 + index * 0.07),
      0,
      0,
      Math.PI * 2,
    );
    context.stroke();
    context.restore();
  }
  context.restore();
}

function drawPolygonMorph(frame: VisualizationFrame) {
  const { context, colors, intensity, time } = frame;
  const bounds = mainBounds(frame);
  const sides = 18;
  const radius = Math.min(bounds.width, frame.height) * 0.23;
  for (let layer = 0; layer < 4; layer += 1) {
    context.globalAlpha = 0.42 - layer * 0.075;
    context.strokeStyle = paletteColor(colors, layer, 4);
    context.beginPath();
    for (let index = 0; index <= sides; index += 1) {
      const position = (index % sides) / sides;
      const angle =
        position * Math.PI * 2 -
        Math.PI / 2 +
        time * 0.00003 * (layer % 2 ? -1 : 1);
      const value = frequencyValue(frame, position, true);
      const distance =
        radius * (1 - layer * 0.1) + value * radius * 0.42 * intensity;
      const x = bounds.centerX + Math.cos(angle) * distance;
      const y = bounds.centerY + Math.sin(angle) * distance;
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.stroke();
  }
}

function drawReactiveGrid(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, height, intensity } = frame;
  const bounds = mainBounds(frame);
  const horizon = height * 0.42;
  context.strokeStyle = colors[0] ?? "currentColor";
  context.lineWidth = 1;
  for (let row = 0; row < 16; row += 1) {
    const depth = row / 15;
    const y = horizon + Math.pow(depth, 1.8) * height * 0.48;
    context.globalAlpha = 0.08 + depth * 0.32;
    context.beginPath();
    for (let column = 0; column <= 48; column += 1) {
      const position = column / 48;
      const signal = frequencyValue(frame, position, true);
      const x = bounds.left + position * bounds.width;
      const lift =
        Math.sin(position * Math.PI * 8 + state.phase + row * 0.22) *
        signal *
        24 *
        intensity *
        (1 - depth * 0.4);
      if (column === 0) context.moveTo(x, y - lift);
      else context.lineTo(x, y - lift);
    }
    context.stroke();
  }
  for (let column = 0; column <= 16; column += 1) {
    const position = column / 16;
    context.globalAlpha = 0.18;
    context.beginPath();
    context.moveTo(
      bounds.centerX + (position - 0.5) * bounds.width * 0.08,
      horizon,
    );
    context.lineTo(bounds.left + position * bounds.width, height * 0.9);
    context.stroke();
  }
}

function drawHexField(frame: VisualizationFrame) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.max(16, Math.min(34, bounds.width / 34));
  const xStep = radius * 1.72;
  const yStep = radius * 1.5;
  const rows = Math.ceil(frame.height / yStep);
  const columns = Math.ceil(bounds.width / xStep);
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      const x = bounds.left + column * xStep + (row % 2 ? xStep / 2 : 0);
      const y = row * yStep;
      const position = (column + row * 0.7) / Math.max(columns + rows * 0.7, 1);
      const value = frequencyValue(frame, position % 1, true);
      const cellRadius = radius * (0.42 + value * 0.34 * intensity);
      context.globalAlpha = 0.05 + value * 0.28;
      context.strokeStyle = paletteColor(colors, column + row, columns + rows);
      context.beginPath();
      for (let side = 0; side <= 6; side += 1) {
        const angle = (side / 6) * Math.PI * 2;
        const px = x + Math.cos(angle) * cellRadius;
        const py = y + Math.sin(angle) * cellRadius;
        if (side === 0) context.moveTo(px, py);
        else context.lineTo(px, py);
      }
      context.stroke();
    }
  }
}

function drawRadar(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.min(bounds.width, frame.height) * 0.27;
  context.strokeStyle = colors[0] ?? "currentColor";
  for (let ring = 1; ring <= 4; ring += 1) {
    context.globalAlpha = 0.12;
    context.beginPath();
    context.arc(
      bounds.centerX,
      bounds.centerY,
      (radius * ring) / 4,
      0,
      Math.PI * 2,
    );
    context.stroke();
  }
  const angle = state.phase * 0.8 - Math.PI / 2;
  const gradient = context.createLinearGradient(
    bounds.centerX,
    bounds.centerY,
    bounds.centerX + Math.cos(angle) * radius,
    bounds.centerY + Math.sin(angle) * radius,
  );
  gradient.addColorStop(0, "transparent");
  gradient.addColorStop(1, colors[0] ?? "currentColor");
  context.globalAlpha = 0.62;
  context.strokeStyle = gradient;
  context.lineWidth = 2;
  context.beginPath();
  context.moveTo(bounds.centerX, bounds.centerY);
  context.lineTo(
    bounds.centerX + Math.cos(angle) * radius,
    bounds.centerY + Math.sin(angle) * radius,
  );
  context.stroke();
  for (let index = 0; index < 28; index += 1) {
    const value = frequencyValue(frame, index / 28, true);
    if (value < 0.18) continue;
    const pointAngle = (index / 28) * Math.PI * 2;
    const distance = radius * (0.2 + ((index * 17) % 23) / 30);
    context.globalAlpha = value * 0.62;
    context.fillStyle = paletteColor(colors, index, 28);
    context.fillRect(
      bounds.centerX + Math.cos(pointAngle) * distance,
      bounds.centerY + Math.sin(pointAngle) * distance,
      2 + value * 3 * intensity,
      2 + value * 3 * intensity,
    );
  }
}

function drawSymmetry(frame: VisualizationFrame, kaleidoscope: boolean) {
  const { context, colors, intensity, time } = frame;
  const bounds = mainBounds(frame);
  const segments = kaleidoscope ? 12 : 8;
  const radius = Math.min(bounds.width, frame.height) * 0.27;
  context.save();
  context.translate(bounds.centerX, bounds.centerY);
  context.rotate(time * 0.000025);
  for (let segment = 0; segment < segments; segment += 1) {
    context.save();
    context.rotate((segment / segments) * Math.PI * 2);
    if (kaleidoscope && segment % 2) context.scale(1, -1);
    context.globalAlpha = 0.2 + (segment % 3) * 0.08;
    context.strokeStyle = paletteColor(colors, segment, segments);
    context.beginPath();
    for (let index = 0; index < 42; index += 1) {
      const position = index / 41;
      const x = position * radius;
      const y = waveformValue(frame, position) * radius * 0.46 * intensity;
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    context.stroke();
    context.restore();
  }
  context.restore();
}

function drawSpirograph(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.min(bounds.width, frame.height) * 0.22;
  const bass = energy(frame, 0, 0.2);
  const ratio = 2.7 + energy(frame, 0.2, 0.75) * 2.2;
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
  for (let index = 0; index <= 420; index += 1) {
    const angle = (index / 420) * Math.PI * 12 + state.phase * 0.2;
    const modulation = radius * (0.22 + bass * 0.18 * intensity);
    const x =
      bounds.centerX +
      Math.cos(angle) * (radius - modulation) +
      Math.cos(angle * ratio) * modulation;
    const y =
      bounds.centerY +
      Math.sin(angle) * (radius - modulation) +
      Math.sin(angle * ratio) * modulation;
    if (index === 0) context.moveTo(x, y);
    else context.lineTo(x, y);
  }
  context.stroke();
}

function drawWireSphere(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.2);
  const radius =
    Math.min(bounds.width, frame.height) * 0.24 * (1 + bass * 0.15 * intensity);
  context.save();
  context.translate(bounds.centerX, bounds.centerY);
  context.rotate(state.phase * 0.08);
  context.strokeStyle = colors[0] ?? "currentColor";
  context.lineWidth = 1;
  for (let latitude = -4; latitude <= 4; latitude += 1) {
    const y = (latitude / 5) * radius;
    const width = Math.sqrt(Math.max(0, radius * radius - y * y));
    context.globalAlpha = 0.12 + (5 - Math.abs(latitude)) * 0.045;
    context.beginPath();
    context.ellipse(0, y, width, width * 0.24, 0, 0, Math.PI * 2);
    context.stroke();
  }
  for (let longitude = 0; longitude < 9; longitude += 1) {
    context.globalAlpha = 0.2;
    context.save();
    context.rotate((longitude / 9) * Math.PI);
    context.beginPath();
    context.ellipse(0, 0, radius * 0.25, radius, 0, 0, Math.PI * 2);
    context.stroke();
    context.restore();
  }
  context.restore();
}

function renderParticles(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const count =
    mode === "dust-waves" || mode === "point-cloud-sphere" ? 150 : 90;
  ensureParticles(state, count);
  if (mode === "particle-constellation") drawConstellation(frame, state);
  else if (mode === "point-cloud-sphere") drawPointSphere(frame, state);
  else updateAndDrawParticles(mode, frame, state);
}

function ensureParticles(state: VisualizationState, count: number) {
  while (state.particles.length < count) {
    const index = state.particles.length;
    state.particles.push({
      x: deterministic(index, 1),
      y: deterministic(index, 2),
      vx: (deterministic(index, 3) - 0.5) * 0.08,
      vy: (deterministic(index, 4) - 0.5) * 0.08,
      life: deterministic(index, 5),
      size: 0.7 + deterministic(index, 6) * 2.2,
      phase: deterministic(index, 7) * Math.PI * 2,
    });
  }
  state.particles.length = count;
}

function drawConstellation(
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const particles = state.particles.slice(0, 70);
  particles.forEach((particle, index) => {
    const band = frequencyValue(frame, index / particles.length, true);
    if (frame.live) {
      particle.x =
        (particle.x +
          particle.vx * frame.delta * (0.3 + band * intensity) +
          1) %
        1;
      particle.y =
        (particle.y +
          particle.vy * frame.delta * (0.3 + band * intensity) +
          1) %
        1;
    }
    const x = bounds.left + particle.x * bounds.width;
    const y = particle.y * frame.height;
    context.globalAlpha = 0.18 + band * 0.48;
    context.fillStyle = paletteColor(colors, index, particles.length);
    context.fillRect(x, y, particle.size, particle.size);
    for (
      let otherIndex = index + 1;
      otherIndex < Math.min(particles.length, index + 7);
      otherIndex += 1
    ) {
      const other = particles[otherIndex];
      if (!other) continue;
      const ox = bounds.left + other.x * bounds.width;
      const oy = other.y * frame.height;
      const distance = Math.hypot(ox - x, oy - y);
      if (distance > 105) continue;
      context.globalAlpha = (1 - distance / 105) * 0.13 * intensity;
      context.strokeStyle = colors[0] ?? "currentColor";
      context.beginPath();
      context.moveTo(x, y);
      context.lineTo(ox, oy);
      context.stroke();
    }
  });
}

function drawPointSphere(frame: VisualizationFrame, state: VisualizationState) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const radius = Math.min(bounds.width, frame.height) * 0.25;
  state.particles.forEach((particle, index) => {
    const latitude = Math.acos(2 * particle.y - 1);
    const longitude = particle.phase + state.phase * 0.22;
    const band = frequencyValue(frame, index / state.particles.length, true);
    const reactiveRadius = radius * (1 + band * 0.2 * intensity);
    const x3 = Math.sin(latitude) * Math.cos(longitude);
    const y3 = Math.cos(latitude);
    const z3 = Math.sin(latitude) * Math.sin(longitude);
    const x = bounds.centerX + x3 * reactiveRadius;
    const y = bounds.centerY + y3 * reactiveRadius;
    context.globalAlpha = 0.08 + (z3 + 1) * 0.22 + band * 0.22;
    context.fillStyle = paletteColor(colors, index, state.particles.length);
    const size = 1 + (z3 + 1) * 1.2;
    context.fillRect(x, y, size, size);
  });
}

function updateAndDrawParticles(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  const { context, colors, intensity } = frame;
  const bounds = mainBounds(frame);
  const bass = energy(frame, 0, 0.2);
  state.particles.forEach((particle, index) => {
    const band = frequencyValue(frame, index / state.particles.length, true);
    const previousX = particle.x;
    const previousY = particle.y;
    if (frame.live) {
      const speed = frame.delta * (0.14 + intensity * 0.09);
      if (mode === "particle-fountain") {
        particle.vy += 0.08 * frame.delta;
        particle.x += particle.vx * speed * 5;
        particle.y += particle.vy * speed * 2;
        if (particle.y > 1 || particle.life <= 0)
          resetFountain(particle, index, bass, intensity);
        particle.life -= frame.delta * 0.18;
      } else if (mode === "particle-vortex") {
        const angle =
          Math.atan2(particle.y - 0.5, particle.x - 0.5) + speed * (0.8 + band);
        const radius = Math.max(
          0.03,
          Math.hypot(particle.x - 0.5, particle.y - 0.5) - frame.delta * 0.005,
        );
        particle.x = 0.5 + Math.cos(angle) * radius;
        particle.y = 0.5 + Math.sin(angle) * radius;
        if (radius < 0.04) {
          particle.x = deterministic(index + Math.floor(frame.time / 1000), 8);
          particle.y = deterministic(index + Math.floor(frame.time / 1000), 9);
        }
      } else if (mode === "audio-rain") {
        particle.y += speed * (0.3 + band * 1.5);
        if (particle.y > 1.05) particle.y = -0.05;
      } else if (mode === "reactive-snow") {
        particle.x += Math.sin(state.phase + particle.phase) * speed * 0.08;
        particle.y += speed * (0.08 + particle.size * 0.025 - bass * 0.08);
        if (particle.y > 1.04) particle.y = -0.04;
      } else if (mode === "dust-waves") {
        particle.x = (particle.x + speed * 0.025) % 1;
        particle.y =
          0.5 +
          waveformValue(frame, particle.x) * 0.75 * intensity +
          (deterministic(index, 10) - 0.5) * 0.18;
      } else if (mode === "magnetic-swarm") {
        const targetX = 0.5 + Math.cos(state.phase * 0.7) * 0.2;
        const targetY = 0.5 + Math.sin(state.phase * 0.9) * 0.16;
        particle.vx += (targetX - particle.x) * speed * 0.05;
        particle.vy += (targetY - particle.y) * speed * 0.05;
        particle.vx *= 0.985;
        particle.vy *= 0.985;
        particle.x += particle.vx * speed;
        particle.y += particle.vy * speed;
      } else if (mode === "frequency-comets") {
        particle.x += speed * (0.08 + band * 0.7);
        particle.y =
          0.12 +
          (index / state.particles.length) * 0.76 +
          Math.sin(state.phase + particle.phase) * 0.04;
        if (particle.x > 1.06) particle.x = -0.06;
      } else {
        particle.x = (particle.x + particle.vx * speed + 1) % 1;
        particle.y = (particle.y + particle.vy * speed + 1) % 1;
      }
    }
    const x = bounds.left + particle.x * bounds.width;
    const y = particle.y * frame.height;
    const px = bounds.left + previousX * bounds.width;
    const py = previousY * frame.height;
    context.globalAlpha = 0.14 + band * (mode === "firefly-field" ? 0.7 : 0.48);
    context.fillStyle = paletteColor(colors, index, state.particles.length);
    if (mode === "audio-rain" || mode === "frequency-comets") {
      context.strokeStyle = context.fillStyle;
      context.beginPath();
      context.moveTo(px, py);
      context.lineTo(x, y + (mode === "audio-rain" ? 12 + band * 26 : 0));
      context.stroke();
    } else {
      const size = particle.size * (0.7 + band * intensity);
      context.beginPath();
      context.arc(x, y, size, 0, Math.PI * 2);
      context.fill();
    }
  });
}

function resetFountain(
  particle: VisualParticle,
  index: number,
  bass: number,
  intensity: number,
) {
  particle.x = 0.47 + deterministic(index, 11) * 0.06;
  particle.y = 0.94;
  particle.vx = (deterministic(index, 12) - 0.5) * 0.9;
  particle.vy = -(0.55 + deterministic(index, 13) * 0.7 + bass * intensity);
  particle.life = 0.7 + deterministic(index, 14) * 0.8;
}

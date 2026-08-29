import type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";
import { renderAmbientOrTerrain } from "$lib/audio-visualization/ambientTerrain";
import { renderArtworkOrAnalytical } from "$lib/audio-visualization/artworkAnalytical";
import {
  resetVisualizationState,
  updateVisualizationState,
  type VisualizationFrame,
  type VisualizationState,
} from "$lib/audio-visualization/core";
import { renderFrequencyOrWave } from "$lib/audio-visualization/frequencyWave";
import { renderGeometryOrParticles } from "$lib/audio-visualization/geometryParticles";

export {
  createVisualizationState,
  resetVisualizationState,
  type VisualizationFrame,
  type VisualizationState,
} from "$lib/audio-visualization/core";

export function renderVisualization(
  mode: AudioVisualizationMode,
  frame: VisualizationFrame,
  state: VisualizationState,
) {
  if (mode === "off") return;
  if (state.mode !== mode) resetVisualizationState(state, mode);
  updateVisualizationState(frame, state);

  const { context } = frame;
  context.save();
  context.globalAlpha = 1;
  context.globalCompositeOperation = "source-over";
  context.lineCap = "square";
  context.lineJoin = "miter";

  const rendered =
    renderFrequencyOrWave(mode, frame, state) ||
    renderGeometryOrParticles(mode, frame, state) ||
    renderAmbientOrTerrain(mode, frame, state) ||
    renderArtworkOrAnalytical(mode, frame, state);

  if (!rendered) {
    context.globalAlpha = 0.35;
    context.strokeStyle = frame.colors[0] ?? "currentColor";
    context.beginPath();
    context.moveTo(0, frame.height * 0.5);
    context.lineTo(frame.width, frame.height * 0.5);
    context.stroke();
  }
  context.restore();
}

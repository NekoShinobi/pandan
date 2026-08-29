<script lang="ts">
  import { prefersReducedMotion } from "svelte/motion";
  import { audioVisualizationNeedsStereo } from "$lib/audioVisualizationCatalog";
  import type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";
  import {
    createVisualizationState,
    renderVisualization,
  } from "$lib/audio-visualization/render";
  import {
    audioVisualizationPaletteColors,
    podcastPlayer,
  } from "$lib/podcastPlayer.svelte";

  const SAMPLE_COUNT = podcastPlayer.visualizationBinCount;
  const frequency = new Uint8Array(SAMPLE_COUNT);
  const waveform = new Uint8Array(SAMPLE_COUNT);
  const leftFrequency = new Uint8Array(SAMPLE_COUNT);
  const rightFrequency = new Uint8Array(SAMPLE_COUNT);
  const leftWaveform = new Uint8Array(SAMPLE_COUNT);
  const rightWaveform = new Uint8Array(SAMPLE_COUNT);

  function attachVisualizer(canvas: HTMLCanvasElement) {
    const context = canvas.getContext("2d");
    if (!context) return;

    const state = createVisualizationState();
    let frameRequest = 0;
    let width = 0;
    let height = 0;
    let lastFrameAt = 0;
    let artwork: HTMLImageElement | null = null;
    let artworkRequest = 0;
    let loadedArtworkUrl = "";
    let mode = podcastPlayer.visualizationMode;
    let colors = audioVisualizationPaletteColors(
      podcastPlayer.visualizationPalette,
      podcastPlayer.visualizationHue,
    );
    let intensity = podcastPlayer.visualizationIntensity;
    let animationRunning = false;

    const resize = () => {
      const bounds = canvas.getBoundingClientRect();
      const nextWidth = Math.max(1, bounds.width);
      const nextHeight = Math.max(1, bounds.height);
      const density = Math.min(globalThis.devicePixelRatio || 1, 1.75);
      if (nextWidth === width && nextHeight === height) return;
      width = nextWidth;
      height = nextHeight;
      canvas.width = Math.round(width * density);
      canvas.height = Math.round(height * density);
      context.setTransform(density, 0, 0, density, 0, 0);
    };

    const draw = (
      activeMode: AudioVisualizationMode,
      live: boolean,
      activeColors: string[],
      activeIntensity: number,
      time = performance.now(),
    ) => {
      resize();
      const mainFrequency =
        live && podcastPlayer.readVisualizationFrequency(frequency);
      const mainWaveform =
        live && podcastPlayer.readVisualizationWaveform(waveform);
      let stereo = false;
      if (live && audioVisualizationNeedsStereo(activeMode)) {
        const stereoFrequency = podcastPlayer.readVisualizationStereoFrequency(
          leftFrequency,
          rightFrequency,
        );
        const stereoWaveform = podcastPlayer.readVisualizationStereoWaveform(
          leftWaveform,
          rightWaveform,
        );
        stereo = stereoFrequency && stereoWaveform;
      }

      context.clearRect(0, 0, width, height);
      const delta = lastFrameAt
        ? Math.min(0.05, Math.max(0, (time - lastFrameAt) / 1000))
        : 0;
      lastFrameAt = time;
      renderVisualization(
        activeMode,
        {
          context,
          width,
          height,
          frequency,
          waveform,
          leftFrequency,
          rightFrequency,
          leftWaveform,
          rightWaveform,
          stereo,
          live: Boolean(mainFrequency && mainWaveform),
          colors: activeColors,
          intensity: activeIntensity,
          time,
          delta,
          sampleRate: podcastPlayer.visualizationSampleRate,
          artwork,
        },
        state,
      );
    };

    const animate = (time: number) => {
      draw(mode, true, colors, intensity, time);
      frameRequest = requestAnimationFrame(animate);
    };

    const loadArtwork = (url: string) => {
      if (url === loadedArtworkUrl) return;
      loadedArtworkUrl = url;
      artworkRequest += 1;
      const request = artworkRequest;
      artwork = null;
      if (!url) return;
      const image = new Image();
      image.decoding = "async";
      image.onload = () => {
        if (request !== artworkRequest) return;
        artwork = image;
        if (!animationRunning) draw(mode, false, colors, intensity);
      };
      image.onerror = () => {
        if (request === artworkRequest) artwork = null;
      };
      image.src = url;
    };

    const observer = new ResizeObserver(() => {
      resize();
      if (!animationRunning) draw(mode, false, colors, intensity);
    });
    observer.observe(canvas);

    $effect(() => {
      mode = podcastPlayer.visualizationMode;
      colors = audioVisualizationPaletteColors(
        podcastPlayer.visualizationPalette,
        podcastPlayer.visualizationHue,
      );
      intensity = podcastPlayer.visualizationIntensity;
      loadArtwork(podcastPlayer.artworkUrl);
      animationRunning =
        podcastPlayer.playing && !prefersReducedMotion.current;
      cancelAnimationFrame(frameRequest);
      lastFrameAt = 0;
      if (animationRunning) {
        frameRequest = requestAnimationFrame(animate);
      } else {
        draw(mode, false, colors, intensity);
      }
      return () => cancelAnimationFrame(frameRequest);
    });

    return () => {
      artworkRequest += 1;
      observer.disconnect();
      cancelAnimationFrame(frameRequest);
    };
  }
</script>

<canvas
  class="audio-visualization-layer"
  aria-hidden="true"
  {@attach attachVisualizer}
  style:opacity={podcastPlayer.visualizationVisibility}
  style:filter={`brightness(${podcastPlayer.visualizationBrightness}) contrast(${podcastPlayer.visualizationContrast})`}
  data-od-id="audio-visualization-layer"
></canvas>

<style>
  .audio-visualization-layer {
    position: fixed;
    inset: 0;
    z-index: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    contain: strict;
  }
</style>

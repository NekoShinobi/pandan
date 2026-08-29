export const AUDIO_VISUALIZATION_GROUPS = [
  {
    id: "frequency",
    label: "Frequency",
    styles: [
      {
        id: "spectrum",
        label: "Spectrum Bars",
        description: "Classic vertical frequency bars.",
      },
      {
        id: "mirrored-spectrum",
        label: "Mirrored Spectrum",
        description: "Frequency bars reflected around a center line.",
      },
      {
        id: "stereo-split-spectrum",
        label: "Stereo Split Spectrum",
        description: "Left and right channels face each other.",
      },
      {
        id: "led-equalizer",
        label: "LED Equalizer",
        description: "Segmented meter columns with held peaks.",
      },
      {
        id: "octave-bands",
        label: "Octave Bands",
        description: "Broad musical bands on a logarithmic scale.",
      },
      {
        id: "logarithmic-spectrum",
        label: "Logarithmic Spectrum",
        description: "Detailed bass-weighted frequency analysis.",
      },
      {
        id: "radial-spectrum",
        label: "Radial Spectrum",
        description: "Frequency bars arranged around a ring.",
      },
      {
        id: "spiral-spectrum",
        label: "Spiral Spectrum",
        description: "A rotating frequency trace on a spiral.",
      },
      {
        id: "spectrum-skyline",
        label: "Spectrum Skyline",
        description: "Layered bars composed like a distant city.",
      },
      {
        id: "waterfall-spectrogram",
        label: "Waterfall Spectrogram",
        description: "Recent spectra recede through depth.",
      },
      {
        id: "frequency-heatmap",
        label: "Frequency Heatmap",
        description: "A time-by-frequency field of signal energy.",
      },
      {
        id: "spectrum-tunnel",
        label: "Spectrum Tunnel",
        description: "Nested spectral frames draw toward a vanishing point.",
      },
    ],
  },
  {
    id: "waveform",
    label: "Waveform",
    styles: [
      {
        id: "wave",
        label: "Wave",
        description: "A precise oscilloscope trace.",
      },
      {
        id: "filled-waveform",
        label: "Filled Waveform",
        description: "A solid waveform silhouette.",
      },
      {
        id: "mirrored-waveform",
        label: "Mirrored Waveform",
        description: "A symmetrical waveform around the center.",
      },
      {
        id: "circular-waveform",
        label: "Circular Waveform",
        description: "The time-domain signal wrapped around a ring.",
      },
      {
        id: "wave-ribbon",
        label: "Wave Ribbon",
        description: "A layered translucent audio ribbon.",
      },
      {
        id: "stacked-echoes",
        label: "Stacked Echoes",
        description: "Recent waveforms trail upward in time.",
      },
      {
        id: "persistence-scope",
        label: "Persistence Scope",
        description: "A luminous scope trace with fading history.",
      },
      {
        id: "lissajous-scope",
        label: "Lissajous Scope",
        description: "Stereo phase drawn as an X/Y figure.",
      },
      {
        id: "waveform-tunnel",
        label: "Waveform Tunnel",
        description: "Concentric waveform loops retreat through space.",
      },
      {
        id: "seismograph",
        label: "Seismograph",
        description: "A restrained scrolling signal recorder.",
      },
      {
        id: "braided-waves",
        label: "Braided Waves",
        description: "Stereo traces weave into one another.",
      },
    ],
  },
  {
    id: "geometry",
    label: "Radial & geometric",
    styles: [
      {
        id: "orbit",
        label: "Orbit",
        description: "Frequency spokes orbit a quiet central core.",
      },
      {
        id: "starburst",
        label: "Starburst",
        description: "Sharp rays respond to spectral peaks.",
      },
      {
        id: "concentric-pulse",
        label: "Concentric Pulse",
        description: "Expanding rings follow the signal envelope.",
      },
      {
        id: "orbital-rings",
        label: "Orbital Rings",
        description: "Tilted reactive rings cross in three dimensions.",
      },
      {
        id: "polygon-morph",
        label: "Polygon Morph",
        description: "A many-sided form bends with the spectrum.",
      },
      {
        id: "reactive-grid",
        label: "Reactive Grid",
        description: "A perspective grid rises under frequency energy.",
      },
      {
        id: "hex-field",
        label: "Hex Field",
        description: "A field of hexagonal cells pulses by band.",
      },
      {
        id: "radar-sweep",
        label: "Radar Sweep",
        description: "A scanning radar records transient returns.",
      },
      {
        id: "mandala",
        label: "Mandala",
        description: "Mirrored spectral geometry forms a radial motif.",
      },
      {
        id: "kaleidoscope",
        label: "Kaleidoscope",
        description: "Waveform segments repeat through angular symmetry.",
      },
      {
        id: "spirograph",
        label: "Spirograph",
        description: "Harmonic ratios trace a changing geometric curve.",
      },
      {
        id: "wireframe-sphere",
        label: "Wireframe Sphere",
        description: "A reactive latitude-and-longitude globe.",
      },
    ],
  },
  {
    id: "particles",
    label: "Particles",
    styles: [
      {
        id: "particle-constellation",
        label: "Particle Constellation",
        description: "Nearby signal particles join into constellations.",
      },
      {
        id: "particle-fountain",
        label: "Particle Fountain",
        description: "Bass launches particles from the lower edge.",
      },
      {
        id: "particle-vortex",
        label: "Particle Vortex",
        description: "Particles spiral around the listening canvas.",
      },
      {
        id: "firefly-field",
        label: "Firefly Field",
        description: "Slow points brighten with midrange detail.",
      },
      {
        id: "audio-rain",
        label: "Audio Rain",
        description: "Frequency-weighted streaks fall through the frame.",
      },
      {
        id: "dust-waves",
        label: "Dust Waves",
        description: "Fine particles ride the waveform envelope.",
      },
      {
        id: "magnetic-swarm",
        label: "Magnetic Swarm",
        description: "A point swarm bends around moving attractors.",
      },
      {
        id: "point-cloud-sphere",
        label: "Point-cloud Sphere",
        description: "A rotating sphere of reactive signal points.",
      },
      {
        id: "frequency-comets",
        label: "Frequency Comets",
        description: "Spectral peaks travel with short luminous tails.",
      },
      {
        id: "reactive-snow",
        label: "Reactive Snow",
        description: "Soft particles drift and lift with the beat.",
      },
    ],
  },
  {
    id: "ambient",
    label: "Shader & fluid",
    styles: [
      {
        id: "aurora-bands",
        label: "Aurora Bands",
        description: "Layered ribbons drift across the background.",
      },
      {
        id: "prismatic-rays",
        label: "Prismatic Rays",
        description: "Long spectral rays spread from one horizon.",
      },
      {
        id: "plasma-field",
        label: "Plasma Field",
        description: "A cellular color field follows the audio envelope.",
      },
      {
        id: "fluid-ink",
        label: "Fluid Ink",
        description: "Soft ink-like contours billow with low frequencies.",
      },
      {
        id: "metaballs",
        label: "Metaballs",
        description: "Blended energy forms merge and separate.",
      },
      {
        id: "nebula",
        label: "Nebula",
        description: "A sparse cloud field responds in layered depth.",
      },
      {
        id: "audio-tunnel",
        label: "Audio Tunnel",
        description: "A flowing tunnel follows rhythmic energy.",
      },
      {
        id: "moire-interference",
        label: "Moiré Interference",
        description: "Overlapping line fields create acoustic interference.",
      },
      {
        id: "reaction-diffusion",
        label: "Reaction–Diffusion",
        description: "Organic cells appear to spread across the field.",
      },
      {
        id: "chromatic-fog",
        label: "Chromatic Fog",
        description: "Translucent color clouds drift with broad bands.",
      },
      {
        id: "electric-field",
        label: "Electric Field",
        description: "Charged lines arc between spectral poles.",
      },
      {
        id: "liquid-lens",
        label: "Liquid Lens",
        description: "Concentric refraction-like contours flex with bass.",
      },
      {
        id: "heat-distortion",
        label: "Heat Distortion",
        description: "Horizontal bands shimmer with high-frequency detail.",
      },
      {
        id: "topographic-flow",
        label: "Topographic Flow",
        description: "Animated contour lines map the audio field.",
      },
    ],
  },
  {
    id: "terrain",
    label: "Terrain & structural",
    styles: [
      {
        id: "frequency-terrain",
        label: "Frequency Terrain",
        description: "Recent spectra form a receding landscape.",
      },
      {
        id: "wireframe-mountains",
        label: "Wireframe Mountains",
        description: "Audio energy raises a mountain profile.",
      },
      {
        id: "voxel-equalizer",
        label: "Voxel Equalizer",
        description: "Isometric blocks encode frequency levels.",
      },
      {
        id: "audio-city",
        label: "Audio City",
        description: "A perspective city grows from the spectrum.",
      },
      {
        id: "data-canyon",
        label: "Data Canyon",
        description: "Mirrored signal walls create a central corridor.",
      },
      {
        id: "reactive-horizon",
        label: "Reactive Horizon",
        description: "A quiet horizon rolls under the music.",
      },
      {
        id: "circuit-field",
        label: "Circuit Field",
        description: "Orthogonal traces route between signal nodes.",
      },
      {
        id: "sonar-landscape",
        label: "Sonar Landscape",
        description: "Sweeping rings reveal a low relief terrain.",
      },
    ],
  },
  {
    id: "artwork",
    label: "Album & media",
    styles: [
      {
        id: "artwork-displacement",
        label: "Artwork Displacement",
        description: "Album art breaks into reactive horizontal slices.",
      },
      {
        id: "artwork-particles",
        label: "Artwork Particles",
        description: "Artwork colors become a field of moving points.",
      },
      {
        id: "artwork-mosaic",
        label: "Artwork Mosaic",
        description: "A tiled cover mosaic responds by frequency band.",
      },
      {
        id: "vinyl-groove",
        label: "Vinyl Groove",
        description: "Artwork sits inside a rotating grooved disc.",
      },
      {
        id: "cassette-scope",
        label: "Cassette Scope",
        description: "A compact cassette frame carries the live waveform.",
      },
      {
        id: "cover-halo",
        label: "Cover Halo",
        description: "A spectral halo surrounds the current artwork.",
      },
      {
        id: "color-extraction",
        label: "Color Extraction",
        description: "Artwork and the selected palette form broad fields.",
      },
      {
        id: "pixel-sort",
        label: "Pixel Sort",
        description: "Artwork columns stretch into frequency-led streaks.",
      },
    ],
  },
  {
    id: "analytical",
    label: "Analytical & terminal",
    styles: [
      {
        id: "vu-needles",
        label: "VU Needles",
        description: "Twin mechanical meters show channel level.",
      },
      {
        id: "digital-level-meter",
        label: "Digital Level Meter",
        description: "Segmented channel meters show live amplitude.",
      },
      {
        id: "phase-correlation-meter",
        label: "Phase Correlation Meter",
        description: "A stereo correlation scale tracks phase.",
      },
      {
        id: "goniometer",
        label: "Goniometer",
        description: "Stereo width is plotted as a vectorscope.",
      },
      {
        id: "chromagram-ring",
        label: "Chromagram Ring",
        description: "Twelve pitch classes form a circular profile.",
      },
      {
        id: "bass-mid-treble-triptych",
        label: "Bass/Mid/Treble Triptych",
        description: "Three panels isolate broad frequency regions.",
      },
      {
        id: "frequency-labels",
        label: "Frequency Labels",
        description: "A calibrated spectrum includes frequency markers.",
      },
      {
        id: "peak-history",
        label: "Peak History",
        description: "Recent signal peaks form a scrolling timeline.",
      },
      {
        id: "telemetry-matrix",
        label: "Telemetry Matrix",
        description: "A terminal grid reports band energy over time.",
      },
      {
        id: "spectral-scanner",
        label: "Spectral Scanner",
        description: "A scanning cursor inspects the live spectrum.",
      },
    ],
  },
] as const;

export type AudioVisualizationStyleId =
  (typeof AUDIO_VISUALIZATION_GROUPS)[number]["styles"][number]["id"];
export type AudioVisualizationStyle = {
  readonly id: AudioVisualizationStyleId;
  readonly label: string;
  readonly description: string;
};
export type AudioVisualizationMode = "off" | AudioVisualizationStyleId;

export const AUDIO_VISUALIZATION_STYLES: readonly AudioVisualizationStyle[] =
  AUDIO_VISUALIZATION_GROUPS.flatMap((group) => [...group.styles]);

const AUDIO_VISUALIZATION_MODE_IDS = new Set<string>([
  "off",
  ...AUDIO_VISUALIZATION_STYLES.map((style) => style.id),
]);

const STEREO_VISUALIZATION_MODE_IDS = new Set<AudioVisualizationMode>([
  "stereo-split-spectrum",
  "lissajous-scope",
  "braided-waves",
  "vu-needles",
  "digital-level-meter",
  "phase-correlation-meter",
  "goniometer",
]);

export function isAudioVisualizationMode(
  value: unknown,
): value is AudioVisualizationMode {
  return typeof value === "string" && AUDIO_VISUALIZATION_MODE_IDS.has(value);
}

export function findAudioVisualizationStyle(
  mode: AudioVisualizationMode,
): AudioVisualizationStyle | undefined {
  if (mode === "off") return undefined;
  return AUDIO_VISUALIZATION_STYLES.find((style) => style.id === mode);
}

export function audioVisualizationNeedsStereo(
  mode: AudioVisualizationMode,
): boolean {
  return STEREO_VISUALIZATION_MODE_IDS.has(mode);
}

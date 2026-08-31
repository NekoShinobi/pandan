<script lang="ts">
  import AudioLines from "lucide-svelte/icons/audio-lines";
  import Check from "lucide-svelte/icons/check";
  import RotateCcw from "lucide-svelte/icons/rotate-ccw";
  import PandanColorPicker from "$lib/components/PandanColorPicker.svelte";
  import {
    AUDIO_VISUALIZATION_GROUPS,
    AUDIO_VISUALIZATION_STYLES,
    findAudioVisualizationStyle,
    type AudioVisualizationMode,
  } from "$lib/audioVisualizationCatalog";
  import { motionPopover } from "$lib/motion.svelte";
  import {
    audioVisualizationPaletteColors,
    MAX_VISUALIZATION_BRIGHTNESS,
    MAX_VISUALIZATION_CONTRAST,
    MAX_VISUALIZATION_INTENSITY,
    MAX_VISUALIZATION_VISIBILITY,
    MIN_VISUALIZATION_BRIGHTNESS,
    MIN_VISUALIZATION_CONTRAST,
    MIN_VISUALIZATION_INTENSITY,
    MIN_VISUALIZATION_VISIBILITY,
    podcastPlayer,
  } from "$lib/podcastPlayer.svelte";
  import type {
    AudioVisualizationPalette,
    AudioVisualizationResponse,
  } from "$lib/podcastPlayer.svelte";

  const paletteOptions: Array<{
    palette: AudioVisualizationPalette;
    label: string;
    description: string;
  }> = [
    { palette: "mono", label: "Mono", description: "Single hue" },
    { palette: "pandan", label: "Pandan", description: "Tonal range" },
    { palette: "signal", label: "Signal", description: "Three colors" },
    { palette: "prism", label: "Prism", description: "Full spectrum" },
  ];

  const responseOptions: Array<{
    response: AudioVisualizationResponse;
    label: string;
  }> = [
    { response: "calm", label: "Calm" },
    { response: "balanced", label: "Balanced" },
    { response: "reactive", label: "Reactive" },
  ];

  let open = $state(false);
  let error = $state("");
  let root: HTMLDivElement | undefined;
  let trigger: HTMLButtonElement | undefined;
  const currentStyle = $derived(
    findAudioVisualizationStyle(podcastPlayer.visualizationMode),
  );
  const currentLabel = $derived(currentStyle?.label ?? "Off");

  function captureRoot(node: HTMLDivElement) {
    root = node;
    return () => {
      if (root === node) root = undefined;
    };
  }

  function captureTrigger(node: HTMLButtonElement) {
    trigger = node;
    return () => {
      if (trigger === node) trigger = undefined;
    };
  }

  function chooseMode(mode: AudioVisualizationMode) {
    error = "";
    if (!podcastPlayer.setVisualizationMode(mode)) {
      error = "Visualization is unavailable in this browser.";
    }
  }

  function resetSettings() {
    error = "";
    podcastPlayer.resetVisualizationSettings();
  }

  $effect(() => {
    if (!open) return;
    const dismiss = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && root?.contains(target)) return;
      open = false;
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      open = false;
      trigger?.focus();
    };
    globalThis.addEventListener("pointerdown", dismiss);
    globalThis.addEventListener("keydown", escape, true);
    return () => {
      globalThis.removeEventListener("pointerdown", dismiss);
      globalThis.removeEventListener("keydown", escape, true);
    };
  });
</script>

<div
  class="audio-visualizer-control"
  {@attach captureRoot}
  data-od-id="audio-visualization-control"
>
  <button
    class={[
      "ui-button",
      "ui-button--ghost",
      "ui-button--icon",
      podcastPlayer.visualizationMode !== "off" && "is-active",
    ]}
    type="button"
    data-tip="Visualization"
    aria-label={`Visualization, ${currentLabel}`}
    aria-expanded={open}
    aria-controls="audio-visualizer-panel"
    onclick={() => {
      error = "";
      open = !open;
    }}
    {@attach captureTrigger}
  >
    <AudioLines size={16} strokeWidth={1.9} aria-hidden="true" />
  </button>

  <div
    class="audio-visualizer-panel"
    id="audio-visualizer-panel"
    aria-hidden={!open}
    inert={!open}
    {@attach motionPopover(open)}
  >
    <div class="audio-visualizer-heading">
      <span class="audio-visualizer-heading-copy">
        <strong>Visualizer</strong>
        <span>Audio-responsive layer behind workspace content</span>
      </span>
      <button
        class="ui-button ui-button--ghost audio-visualizer-reset"
        type="button"
        data-od-id="audio-visualization-reset"
        onclick={resetSettings}
      >
        <RotateCcw size={13} strokeWidth={1.9} aria-hidden="true" />
        Reset
      </button>
    </div>

    <section class="audio-visualizer-section">
      <div class="audio-visualizer-section-heading">
        <strong>Animation</strong>
        <span>{AUDIO_VISUALIZATION_STYLES.length} styles</span>
      </div>
      <label
        class="audio-visualizer-select-label"
        for="audio-visualization-mode"
      >
        <span class="sr-only">Visualization animation</span>
        <select
          id="audio-visualization-mode"
          data-od-id="audio-visualization-mode"
          value={podcastPlayer.visualizationMode}
          onchange={(event) =>
            chooseMode(event.currentTarget.value as AudioVisualizationMode)}
        >
          <option value="off">Off</option>
          {#each AUDIO_VISUALIZATION_GROUPS as group (group.id)}
            <optgroup label={group.label}>
              {#each group.styles as style (style.id)}
                <option value={style.id}>{style.label}</option>
              {/each}
            </optgroup>
          {/each}
        </select>
      </label>
      <p class="audio-visualizer-style-note">
        {currentStyle?.description ??
          "Disable the ambient visualization layer."}
      </p>
    </section>

    <section class="audio-visualizer-section">
      <div class="audio-visualizer-section-heading">
        <strong>Color palette</strong>
        <span>Mono or multi-color</span>
      </div>
      <div
        class="audio-visualizer-palette-grid"
        role="radiogroup"
        aria-label="Visualization color palette"
      >
        {#each paletteOptions as option (option.palette)}
          {@const colors = audioVisualizationPaletteColors(
            option.palette,
            podcastPlayer.visualizationColor,
          )}
          <button
            class="audio-visualizer-palette"
            type="button"
            role="radio"
            aria-checked={podcastPlayer.visualizationPalette === option.palette}
            onclick={() =>
              podcastPlayer.setVisualizationPalette(option.palette)}
            data-od-id={`audio-visualization-palette-${option.palette}`}
          >
            <span class="audio-palette-swatches" aria-hidden="true">
              {#each colors as color, index (`${option.palette}-${index}`)}
                <i style:background-color={color}></i>
              {/each}
            </span>
            <span class="audio-palette-copy">
              <strong>{option.label}</strong>
              <small>{option.description}</small>
            </span>
            {#if podcastPlayer.visualizationPalette === option.palette}
              <Check size={14} strokeWidth={2} aria-hidden="true" />
            {/if}
          </button>
        {/each}
      </div>
    </section>

    <section class="audio-visualizer-section">
      <PandanColorPicker
        id="audio-visualization-color"
        label="Base color"
        value={podcastPlayer.visualizationColor}
        helpText="The palette derives from this exact color."
        onchange={(value) => podcastPlayer.setVisualizationColor(value)}
      />
    </section>

    <section class="audio-visualizer-section audio-visualizer-ranges">
      <label for="audio-visualization-visibility">
        <span>
          <strong>Visibility</strong>
          <output for="audio-visualization-visibility">
            {Math.round(podcastPlayer.visualizationVisibility * 100)}%
          </output>
        </span>
        <input
          id="audio-visualization-visibility"
          data-od-id="audio-visualization-visibility"
          type="range"
          min={MIN_VISUALIZATION_VISIBILITY}
          max={MAX_VISUALIZATION_VISIBILITY}
          step="0.05"
          value={podcastPlayer.visualizationVisibility}
          aria-valuetext={`${Math.round(podcastPlayer.visualizationVisibility * 100)} percent`}
          oninput={(event) =>
            podcastPlayer.setVisualizationVisibility(
              Number(event.currentTarget.value),
            )}
        />
      </label>

      <label for="audio-visualization-intensity">
        <span>
          <strong>Intensity</strong>
          <output for="audio-visualization-intensity">
            {Math.round(podcastPlayer.visualizationIntensity * 100)}%
          </output>
        </span>
        <input
          id="audio-visualization-intensity"
          data-od-id="audio-visualization-intensity"
          type="range"
          min={MIN_VISUALIZATION_INTENSITY}
          max={MAX_VISUALIZATION_INTENSITY}
          step="0.05"
          value={podcastPlayer.visualizationIntensity}
          aria-valuetext={`${Math.round(podcastPlayer.visualizationIntensity * 100)} percent`}
          oninput={(event) =>
            podcastPlayer.setVisualizationIntensity(
              Number(event.currentTarget.value),
            )}
        />
      </label>

      <label for="audio-visualization-brightness">
        <span>
          <strong>Brightness</strong>
          <output for="audio-visualization-brightness">
            {Math.round(podcastPlayer.visualizationBrightness * 100)}%
          </output>
        </span>
        <input
          id="audio-visualization-brightness"
          data-od-id="audio-visualization-brightness"
          type="range"
          min={MIN_VISUALIZATION_BRIGHTNESS}
          max={MAX_VISUALIZATION_BRIGHTNESS}
          step="0.05"
          value={podcastPlayer.visualizationBrightness}
          aria-valuetext={`${Math.round(podcastPlayer.visualizationBrightness * 100)} percent`}
          oninput={(event) =>
            podcastPlayer.setVisualizationBrightness(
              Number(event.currentTarget.value),
            )}
        />
      </label>

      <label for="audio-visualization-contrast">
        <span>
          <strong>Contrast</strong>
          <output for="audio-visualization-contrast">
            {Math.round(podcastPlayer.visualizationContrast * 100)}%
          </output>
        </span>
        <input
          id="audio-visualization-contrast"
          data-od-id="audio-visualization-contrast"
          type="range"
          min={MIN_VISUALIZATION_CONTRAST}
          max={MAX_VISUALIZATION_CONTRAST}
          step="0.05"
          value={podcastPlayer.visualizationContrast}
          aria-valuetext={`${Math.round(podcastPlayer.visualizationContrast * 100)} percent`}
          oninput={(event) =>
            podcastPlayer.setVisualizationContrast(
              Number(event.currentTarget.value),
            )}
        />
      </label>
    </section>

    <section class="audio-visualizer-section">
      <div class="audio-visualizer-section-heading">
        <strong>Response</strong>
        <span>How quickly the signal settles</span>
      </div>
      <div
        class="audio-visualizer-response-grid"
        role="radiogroup"
        aria-label="Visualization response"
      >
        {#each responseOptions as option (option.response)}
          <button
            class="audio-visualizer-choice"
            type="button"
            role="radio"
            data-od-id={`audio-visualization-response-${option.response}`}
            aria-checked={podcastPlayer.visualizationResponse ===
              option.response}
            onclick={() =>
              podcastPlayer.setVisualizationResponse(option.response)}
          >
            {option.label}
          </button>
        {/each}
      </div>
    </section>

    {#if error}
      <p class="audio-visualizer-error" role="status">{error}</p>
    {/if}
  </div>
</div>

<style>
  .audio-visualizer-control {
    position: relative;
    display: flex;
    flex: 0 0 auto;
  }

  .audio-visualizer-control > .is-active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .audio-visualizer-panel {
    position: absolute;
    right: 0;
    bottom: calc(100% + 9px);
    z-index: 3;
    width: min(520px, calc(100vw - 24px));
    max-height: min(680px, calc(100dvh - 120px - env(safe-area-inset-top)));
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    padding: 0 10px 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: 10px 10px 0 color-mix(in oklch, var(--bg) 72%, transparent);
    visibility: hidden;
    opacity: 0;
    pointer-events: none;
    transform: translateY(6px);
    will-change: opacity, transform;
  }

  .audio-visualizer-heading {
    position: sticky;
    top: 0;
    z-index: 1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 10px 4px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .audio-visualizer-heading-copy,
  .audio-palette-copy {
    display: grid;
    gap: 3px;
  }

  .audio-visualizer-heading-copy > strong {
    font-size: 11px;
    font-weight: 620;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .audio-visualizer-heading-copy > span,
  .audio-visualizer-style-note {
    color: var(--muted);
    font-size: 10px;
    line-height: 1.5;
  }

  .audio-visualizer-reset {
    min-height: 44px;
    padding-inline: 9px;
    gap: 6px;
    font-size: 9px;
  }

  .audio-visualizer-section {
    padding: 11px 4px 12px;
    border-bottom: 1px solid var(--border);
  }

  .audio-visualizer-section:last-of-type {
    border-bottom: 0;
  }

  .audio-visualizer-section-heading {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }

  .audio-visualizer-section-heading strong,
  .audio-visualizer-ranges strong {
    color: var(--fg);
    font-size: 10px;
    font-weight: 620;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .audio-visualizer-section-heading span {
    color: var(--muted);
    font-size: 9px;
    line-height: 1.45;
    text-align: right;
  }

  .audio-visualizer-select-label,
  .audio-visualizer-select-label select {
    display: block;
    width: 100%;
  }

  .audio-visualizer-select-label select {
    min-height: 44px;
    padding: 0 34px 0 10px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--bg);
    color: var(--fg);
    color-scheme: dark;
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .audio-visualizer-select-label select option,
  .audio-visualizer-select-label select optgroup {
    background-color: var(--bg);
    color: var(--fg);
  }

  .audio-visualizer-select-label select optgroup {
    font-weight: 620;
  }

  .audio-visualizer-select-label select option:checked {
    background-color: var(--fg-soft);
    color: var(--fg);
  }

  .audio-visualizer-style-note {
    min-height: 30px;
    margin: 7px 0 0;
  }

  .audio-visualizer-palette-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 5px;
  }

  .audio-visualizer-response-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 5px;
  }

  .audio-visualizer-choice,
  .audio-visualizer-palette {
    min-width: 0;
    min-height: 44px;
    width: 100%;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    text-align: left;
  }

  .audio-visualizer-choice {
    padding: 7px 8px;
    font-size: 10px;
    font-weight: 620;
    letter-spacing: 0.02em;
    text-align: center;
  }

  .audio-visualizer-palette {
    min-height: 66px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
  }

  .audio-visualizer-choice:hover,
  .audio-visualizer-palette:hover,
  .audio-visualizer-select-label select:hover {
    border-color: var(--fg);
    background: var(--fg-soft);
    color: var(--fg);
  }

  .audio-visualizer-choice:focus-visible,
  .audio-visualizer-palette:focus-visible,
  .audio-visualizer-select-label select:focus-visible,
  .audio-visualizer-ranges input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .audio-visualizer-choice[aria-checked="true"],
  .audio-visualizer-palette[aria-checked="true"] {
    border-color: var(--fg);
    background: var(--fg-soft);
    color: var(--fg);
  }

  .audio-palette-swatches {
    grid-column: 1 / -1;
    display: flex;
    height: 6px;
    border: 1px solid color-mix(in oklch, var(--fg) 22%, transparent);
    background: var(--bg);
  }

  .audio-palette-swatches i {
    min-width: 0;
    flex: 1 1 0;
  }

  .audio-palette-copy strong {
    font-size: 11px;
    font-weight: 620;
    letter-spacing: 0.02em;
  }

  .audio-palette-copy small {
    color: var(--muted);
    font-size: 9px;
    line-height: 1.45;
  }

  .audio-visualizer-ranges {
    display: grid;
    gap: 10px;
  }

  .audio-visualizer-ranges label {
    display: grid;
    gap: 2px;
    color: var(--fg);
    font-family: var(--font-mono);
  }

  .audio-visualizer-ranges label > span {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .audio-visualizer-ranges output {
    color: var(--muted);
    font-size: 10px;
  }

  .audio-visualizer-ranges input {
    min-height: 30px;
    width: 100%;
    accent-color: var(--accent);
  }

  .audio-visualizer-error {
    margin: 8px 4px 0;
    color: var(--danger, var(--fg));
    font-size: 10px;
    line-height: 1.5;
  }

  @media (max-width: 560px) {
    .audio-visualizer-panel {
      position: fixed;
      right: max(8px, env(safe-area-inset-right));
      bottom: calc(76px + env(safe-area-inset-bottom));
      left: max(8px, env(safe-area-inset-left));
      width: auto;
      max-height: min(68dvh, 680px);
    }
  }
</style>

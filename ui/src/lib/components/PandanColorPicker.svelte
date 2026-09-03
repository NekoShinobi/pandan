<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    DEFAULT_COLOR_PRESETS,
    clampColorChannel,
    hexToHsl,
    hexToRgb,
    hslToHex,
    normalizeHexColor,
    rgbToHex,
    type ColorPreset,
    type HexColor,
    type HslColor,
    type RgbColor,
  } from "$lib/color";

  type ColorModel = "hsl" | "rgb";

  let {
    id,
    label = "Color",
    value = $bindable("#2DD4BF" as HexColor),
    helpText = "Choose a preset or enter an exact color.",
    presets = DEFAULT_COLOR_PRESETS,
    disabled = false,
    onchange,
    onvaliditychange,
  }: {
    id: string;
    label?: string;
    value?: HexColor;
    helpText?: string;
    presets?: readonly ColorPreset[];
    disabled?: boolean;
    onchange?: (value: HexColor) => void;
    onvaliditychange?: (valid: boolean) => void;
  } = $props();

  let model = $state<ColorModel>("hsl");
  let copyLabel = $state("Copy");
  let hexInput: HTMLInputElement | undefined;
  let copyTimer: ReturnType<typeof setTimeout> | undefined;
  let hsl = $derived(hexToHsl(value));
  let rgb = $derived(hexToRgb(value));
  let hexDraft: string = $derived(value);
  let hexError = $derived(
    normalizeHexColor(hexDraft) ? "" : "Enter six hexadecimal characters.",
  );

  onDestroy(() => {
    if (copyTimer) clearTimeout(copyTimer);
  });

  function applyColor(nextValue: HexColor) {
    value = nextValue;
    hexDraft = nextValue;
    onvaliditychange?.(true);
    onchange?.(nextValue);
  }

  function updateHex(event: Event) {
    hexDraft = (event.currentTarget as HTMLInputElement).value.toUpperCase();
    const normalized = normalizeHexColor(hexDraft);
    if (normalized) {
      applyColor(normalized);
      return;
    }
    onvaliditychange?.(false);
  }

  function captureHexInput(node: HTMLInputElement) {
    hexInput = node;
    return () => {
      if (hexInput === node) hexInput = undefined;
    };
  }

  function updateHsl(channel: keyof HslColor, nextValue: number) {
    if (!Number.isFinite(nextValue)) return;
    const maximum = channel === "hue" ? 359 : 100;
    const next = {
      ...hsl,
      [channel]: clampColorChannel(nextValue, 0, maximum),
    };
    applyColor(hslToHex(next.hue, next.saturation, next.lightness));
  }

  function updateRgb(channel: keyof RgbColor, nextValue: number) {
    if (!Number.isFinite(nextValue)) return;
    const next = {
      ...rgb,
      [channel]: clampColorChannel(nextValue, 0, 255),
    };
    applyColor(rgbToHex(next.red, next.green, next.blue));
  }

  function updatePlane(button: HTMLButtonElement, event: PointerEvent) {
    const bounds = button.getBoundingClientRect();
    const saturation = clampColorChannel(
      ((event.clientX - bounds.left) / bounds.width) * 100,
      0,
      100,
    );
    const lightness = clampColorChannel(
      (1 - (event.clientY - bounds.top) / bounds.height) * 100,
      0,
      100,
    );
    applyColor(hslToHex(hsl.hue, saturation, lightness));
  }

  function handlePlanePointerDown(event: PointerEvent) {
    const button = event.currentTarget as HTMLButtonElement;
    button.setPointerCapture(event.pointerId);
    updatePlane(button, event);
  }

  function handlePlanePointerMove(event: PointerEvent) {
    const button = event.currentTarget as HTMLButtonElement;
    if (button.hasPointerCapture(event.pointerId)) updatePlane(button, event);
  }

  function handlePlaneKeydown(event: KeyboardEvent) {
    const step = event.shiftKey ? 10 : 1;
    let saturation = hsl.saturation;
    let lightness = hsl.lightness;
    if (event.key === "ArrowLeft") saturation -= step;
    else if (event.key === "ArrowRight") saturation += step;
    else if (event.key === "ArrowUp") lightness += step;
    else if (event.key === "ArrowDown") lightness -= step;
    else return;
    event.preventDefault();
    applyColor(
      hslToHex(
        hsl.hue,
        clampColorChannel(saturation, 0, 100),
        clampColorChannel(lightness, 0, 100),
      ),
    );
  }

  function handleModelKeydown(event: KeyboardEvent) {
    if (
      event.key !== "ArrowLeft" &&
      event.key !== "ArrowRight" &&
      event.key !== "Home" &&
      event.key !== "End"
    ) {
      return;
    }
    event.preventDefault();
    if (event.key === "Home") model = "hsl";
    else if (event.key === "End") model = "rgb";
    else model = model === "hsl" ? "rgb" : "hsl";
    document.getElementById(`${id}-${model}-tab`)?.focus();
  }

  function restoreNumber(event: FocusEvent, currentValue: number) {
    const input = event.currentTarget as HTMLInputElement;
    if (!Number.isFinite(input.valueAsNumber))
      input.value = String(currentValue);
  }

  async function copyColor() {
    try {
      await navigator.clipboard.writeText(value);
      copyLabel = "Copied";
    } catch {
      copyLabel = "Select";
      hexInput?.select();
    }
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      copyLabel = "Copy";
    }, 1100);
  }
</script>

<fieldset
  class="pandan-color-picker"
  {disabled}
  style:--selected-color={value}
  style:--picker-hue={String(hsl.hue)}
  style:--picker-saturation={`${hsl.saturation}%`}
  style:--picker-lightness={`${hsl.lightness}%`}
  style:--picker-red={String(rgb.red)}
  style:--picker-green={String(rgb.green)}
  style:--picker-blue={String(rgb.blue)}
  data-od-id={`${id}-color-picker`}
>
  <legend>{label}</legend>

  <div class="color-summary">
    <span class="color-preview" aria-hidden="true"></span>
    <strong>{value}</strong>
  </div>

  <div class="color-editor">
    <button
      class="color-plane"
      id={`${id}-plane`}
      type="button"
      aria-label={`Saturation ${hsl.saturation}%, lightness ${hsl.lightness}%. Use arrow keys to adjust.`}
      onpointerdown={handlePlanePointerDown}
      onpointermove={handlePlanePointerMove}
      onkeydown={handlePlaneKeydown}
      data-od-id={`${id}-color-plane`}
    >
      <span
        class="plane-cursor"
        style:left={`${hsl.saturation}%`}
        style:top={`${100 - hsl.lightness}%`}
        aria-hidden="true"
      ></span>
    </button>

    <div class="channel-editor">
      <div class="model-switch" role="tablist" aria-label="Color model">
        <button
          id={`${id}-hsl-tab`}
          type="button"
          role="tab"
          aria-selected={model === "hsl"}
          aria-controls={`${id}-hsl-panel`}
          tabindex={model === "hsl" ? 0 : -1}
          onclick={() => (model = "hsl")}
          onkeydown={handleModelKeydown}
          data-od-id={`${id}-model-hsl`}>HSL</button
        >
        <button
          id={`${id}-rgb-tab`}
          type="button"
          role="tab"
          aria-selected={model === "rgb"}
          aria-controls={`${id}-rgb-panel`}
          tabindex={model === "rgb" ? 0 : -1}
          onclick={() => (model = "rgb")}
          onkeydown={handleModelKeydown}
          data-od-id={`${id}-model-rgb`}>RGB</button
        >
      </div>

      {#if model === "hsl"}
        <div
          class="channel-panel"
          id={`${id}-hsl-panel`}
          role="tabpanel"
          aria-labelledby={`${id}-hsl-tab`}
        >
          <div class="channel-row">
            <label for={`${id}-hue-range`}>H</label>
            <input
              class="hue-range"
              id={`${id}-hue-range`}
              type="range"
              min="0"
              max="359"
              value={hsl.hue}
              aria-label="Hue"
              oninput={(event) =>
                updateHsl("hue", event.currentTarget.valueAsNumber)}
            />
            <input
              class="channel-input"
              type="number"
              min="0"
              max="359"
              value={hsl.hue}
              aria-label="Hue value"
              oninput={(event) =>
                updateHsl("hue", event.currentTarget.valueAsNumber)}
              onblur={(event) => restoreNumber(event, hsl.hue)}
            />
          </div>
          <div class="channel-row">
            <label for={`${id}-saturation-range`}>S</label>
            <input
              class="saturation-range"
              id={`${id}-saturation-range`}
              type="range"
              min="0"
              max="100"
              value={hsl.saturation}
              aria-label="Saturation"
              oninput={(event) =>
                updateHsl("saturation", event.currentTarget.valueAsNumber)}
            />
            <input
              class="channel-input"
              type="number"
              min="0"
              max="100"
              value={hsl.saturation}
              aria-label="Saturation value"
              oninput={(event) =>
                updateHsl("saturation", event.currentTarget.valueAsNumber)}
              onblur={(event) => restoreNumber(event, hsl.saturation)}
            />
          </div>
          <div class="channel-row">
            <label for={`${id}-lightness-range`}>L</label>
            <input
              class="lightness-range"
              id={`${id}-lightness-range`}
              type="range"
              min="0"
              max="100"
              value={hsl.lightness}
              aria-label="Lightness"
              oninput={(event) =>
                updateHsl("lightness", event.currentTarget.valueAsNumber)}
            />
            <input
              class="channel-input"
              type="number"
              min="0"
              max="100"
              value={hsl.lightness}
              aria-label="Lightness value"
              oninput={(event) =>
                updateHsl("lightness", event.currentTarget.valueAsNumber)}
              onblur={(event) => restoreNumber(event, hsl.lightness)}
            />
          </div>
        </div>
      {:else}
        <div
          class="channel-panel"
          id={`${id}-rgb-panel`}
          role="tabpanel"
          aria-labelledby={`${id}-rgb-tab`}
        >
          <div class="channel-row">
            <label for={`${id}-red-range`}>R</label>
            <input
              class="red-range"
              id={`${id}-red-range`}
              type="range"
              min="0"
              max="255"
              value={rgb.red}
              aria-label="Red"
              oninput={(event) =>
                updateRgb("red", event.currentTarget.valueAsNumber)}
            />
            <input
              class="channel-input"
              type="number"
              min="0"
              max="255"
              value={rgb.red}
              aria-label="Red value"
              oninput={(event) =>
                updateRgb("red", event.currentTarget.valueAsNumber)}
              onblur={(event) => restoreNumber(event, rgb.red)}
            />
          </div>
          <div class="channel-row">
            <label for={`${id}-green-range`}>G</label>
            <input
              class="green-range"
              id={`${id}-green-range`}
              type="range"
              min="0"
              max="255"
              value={rgb.green}
              aria-label="Green"
              oninput={(event) =>
                updateRgb("green", event.currentTarget.valueAsNumber)}
            />
            <input
              class="channel-input"
              type="number"
              min="0"
              max="255"
              value={rgb.green}
              aria-label="Green value"
              oninput={(event) =>
                updateRgb("green", event.currentTarget.valueAsNumber)}
              onblur={(event) => restoreNumber(event, rgb.green)}
            />
          </div>
          <div class="channel-row">
            <label for={`${id}-blue-range`}>B</label>
            <input
              class="blue-range"
              id={`${id}-blue-range`}
              type="range"
              min="0"
              max="255"
              value={rgb.blue}
              aria-label="Blue"
              oninput={(event) =>
                updateRgb("blue", event.currentTarget.valueAsNumber)}
            />
            <input
              class="channel-input"
              type="number"
              min="0"
              max="255"
              value={rgb.blue}
              aria-label="Blue value"
              oninput={(event) =>
                updateRgb("blue", event.currentTarget.valueAsNumber)}
              onblur={(event) => restoreNumber(event, rgb.blue)}
            />
          </div>
        </div>
      {/if}
    </div>
  </div>

  <div class="color-presets" role="group" aria-label="Color presets">
    {#each presets as preset (preset.value)}
      <button
        class={["color-preset", value === preset.value && "is-selected"]}
        type="button"
        style:--preset-color={preset.value}
        aria-label={preset.label}
        aria-pressed={value === preset.value}
        title={preset.label}
        onclick={() => applyColor(preset.value)}
        data-od-id={`${id}-preset-${preset.label.toLowerCase().replaceAll(" ", "-")}`}
        ><span aria-hidden="true"></span></button
      >
    {/each}
  </div>

  <div class="hex-row">
    <label for={`${id}-hex`}>HEX</label>
    <input
      id={`${id}-hex`}
      class={[hexError && "invalid"]}
      type="text"
      {@attach captureHexInput}
      bind:value={hexDraft}
      maxlength="7"
      spellcheck="false"
      autocomplete="off"
      aria-invalid={Boolean(hexError)}
      aria-describedby={`${id}-help`}
      oninput={updateHex}
      data-od-id={`${id}-hex-input`}
    />
    <button type="button" onclick={copyColor} data-od-id={`${id}-copy`}>
      {copyLabel}
    </button>
  </div>
  <small id={`${id}-help`} class={[hexError && "invalid"]} aria-live="polite">
    {hexError || helpText}
  </small>
</fieldset>

<style>
  .pandan-color-picker {
    container-type: inline-size;
    min-width: 0;
    display: grid;
    gap: 12px;
    margin: 0;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 88%, var(--bg));
    padding: 12px;
    color: var(--fg);
  }

  legend {
    padding: 0 6px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 620;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  button,
  input {
    font: inherit;
  }

  button {
    cursor: pointer;
  }

  .color-summary {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 9px;
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .color-preview {
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    border: 1px solid color-mix(in oklch, var(--selected-color) 70%, var(--fg));
    background: var(--selected-color);
    box-shadow: inset 0 0 0 3px var(--surface);
  }

  .color-summary strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 550;
    letter-spacing: 0.03em;
  }

  .color-editor {
    min-width: 0;
    display: grid;
    gap: 12px;
  }

  .color-plane {
    position: relative;
    width: 100%;
    min-height: 160px;
    overflow: hidden;
    border: 1px solid color-mix(in oklch, var(--fg) 28%, var(--border));
    background:
      linear-gradient(
        to bottom,
        oklch(100% 0 0),
        transparent 50%,
        oklch(0% 0 0)
      ),
      linear-gradient(
        to right,
        hsl(var(--picker-hue) 0% 50%),
        hsl(var(--picker-hue) 100% 50%)
      );
    touch-action: none;
  }

  .color-plane::after {
    content: "S →  /  L ↑";
    position: absolute;
    right: 8px;
    bottom: 6px;
    color: color-mix(in oklch, var(--fg) 72%, transparent);
    font-family: var(--font-mono);
    font-size: 8px;
    letter-spacing: 0.08em;
    pointer-events: none;
  }

  .plane-cursor {
    position: absolute;
    width: 16px;
    height: 16px;
    border: 2px solid var(--fg);
    background: transparent;
    box-shadow: 0 0 0 1px var(--bg);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }

  .channel-editor {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .model-switch {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .model-switch button {
    min-height: 44px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 620;
    letter-spacing: 0.08em;
  }

  .model-switch button + button {
    border-left: 0;
  }

  .model-switch button:hover {
    background: var(--fg-soft);
    color: var(--fg);
  }

  .model-switch button[aria-selected="true"] {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--bg);
  }

  .channel-panel {
    display: grid;
    gap: 4px;
  }

  .channel-row {
    min-width: 0;
    min-height: 44px;
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr) 60px;
    align-items: center;
    gap: 7px;
  }

  .channel-row label {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 620;
    text-align: center;
  }

  .channel-input {
    width: 60px;
    min-height: 44px;
    border: 1px solid var(--border);
    border-radius: 0;
    appearance: textfield;
    background: var(--bg);
    color: var(--fg);
    padding: 0 7px;
    text-align: right;
    font-family: var(--font-mono);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .channel-input::-webkit-inner-spin-button,
  .channel-input::-webkit-outer-spin-button {
    margin: 0;
    appearance: none;
  }

  input[type="range"] {
    width: 100%;
    height: 44px;
    margin: 0;
    border: 0;
    appearance: none;
    background: transparent;
    padding: 0;
  }

  input[type="range"]::-webkit-slider-runnable-track {
    height: 8px;
    border: 1px solid color-mix(in oklch, var(--fg) 28%, var(--border));
    background: var(--channel-background);
  }

  input[type="range"]::-moz-range-track {
    height: 8px;
    border: 1px solid color-mix(in oklch, var(--fg) 28%, var(--border));
    background: var(--channel-background);
  }

  input[type="range"]::-webkit-slider-thumb {
    width: 13px;
    height: 22px;
    margin-top: -8px;
    appearance: none;
    border: 2px solid var(--surface);
    border-radius: 0;
    background: var(--fg);
    box-shadow: 0 0 0 1px var(--fg);
  }

  input[type="range"]::-moz-range-thumb {
    width: 11px;
    height: 20px;
    border: 2px solid var(--surface);
    border-radius: 0;
    background: var(--fg);
    box-shadow: 0 0 0 1px var(--fg);
  }

  .hue-range {
    --channel-background: linear-gradient(
      90deg,
      hsl(0 75% 55%),
      hsl(60 75% 55%),
      hsl(120 75% 45%),
      hsl(180 75% 45%),
      hsl(240 75% 60%),
      hsl(300 75% 55%),
      hsl(360 75% 55%)
    );
  }

  .saturation-range {
    --channel-background: linear-gradient(
      90deg,
      hsl(var(--picker-hue) 0% var(--picker-lightness)),
      hsl(var(--picker-hue) 100% var(--picker-lightness))
    );
  }

  .lightness-range {
    --channel-background: linear-gradient(
      90deg,
      hsl(var(--picker-hue) var(--picker-saturation) 0%),
      hsl(var(--picker-hue) var(--picker-saturation) 50%),
      hsl(var(--picker-hue) var(--picker-saturation) 100%)
    );
  }

  .red-range {
    --channel-background: linear-gradient(
      90deg,
      rgb(0 var(--picker-green) var(--picker-blue)),
      rgb(255 var(--picker-green) var(--picker-blue))
    );
  }

  .green-range {
    --channel-background: linear-gradient(
      90deg,
      rgb(var(--picker-red) 0 var(--picker-blue)),
      rgb(var(--picker-red) 255 var(--picker-blue))
    );
  }

  .blue-range {
    --channel-background: linear-gradient(
      90deg,
      rgb(var(--picker-red) var(--picker-green) 0),
      rgb(var(--picker-red) var(--picker-green) 255)
    );
  }

  .color-presets {
    display: grid;
    grid-template-columns: repeat(4, minmax(44px, 1fr));
    gap: 4px;
  }

  .color-preset {
    position: relative;
    min-width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid transparent;
    background: transparent;
    padding: 0;
  }

  .color-preset:hover,
  .color-preset.is-selected {
    border-color: var(--fg);
    background: var(--fg-soft);
  }

  .color-preset span {
    width: 26px;
    height: 26px;
    border: 1px solid color-mix(in oklch, var(--preset-color) 72%, var(--fg));
    border-radius: 50%;
    background: var(--preset-color);
  }

  .color-preset.is-selected::after {
    content: "";
    position: absolute;
    right: 5px;
    bottom: 5px;
    width: 7px;
    height: 7px;
    border: 2px solid var(--surface);
    border-radius: 50%;
    background: var(--fg);
  }

  .hex-row {
    min-width: 0;
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) 78px;
    align-items: center;
    gap: 8px;
  }

  .hex-row label {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
  }

  .hex-row input,
  .hex-row button {
    min-width: 0;
    min-height: 44px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--bg);
    color: var(--fg);
    padding: 0 9px;
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .hex-row input {
    text-transform: uppercase;
  }

  .hex-row input.invalid {
    border-color: var(--danger);
  }

  .hex-row button:hover {
    border-color: var(--fg);
    background: var(--fg-soft);
  }

  small {
    min-height: 14px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    line-height: 1.5;
  }

  small.invalid {
    color: var(--danger);
  }

  :is(button, input):focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  fieldset:disabled :is(button, input) {
    cursor: not-allowed;
    opacity: 0.55;
  }

  @container (min-width: 430px) {
    .color-editor {
      grid-template-columns: minmax(0, 1fr) minmax(190px, 0.9fr);
      gap: 14px;
    }

    .color-plane {
      min-height: 220px;
    }

    .color-presets {
      grid-template-columns: repeat(8, minmax(44px, 1fr));
    }
  }
</style>

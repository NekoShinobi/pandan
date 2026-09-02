<script lang="ts">
  import AudioLines from "lucide-svelte/icons/audio-lines";
  import ImageIcon from "lucide-svelte/icons/image";
  import AudioVisualization from "$lib/AudioVisualization.svelte";
  import {
    AUDIO_VISUALIZATION_STYLES,
    isAudioVisualizationMode,
    type AudioVisualizationMode,
  } from "$lib/audioVisualizationCatalog";
  import {
    dashboardWidgetImageUrl,
    type DashboardWidget,
  } from "$lib/api";
  import { podcastPlayer } from "$lib/podcastPlayer.svelte";

  let { widget }: { widget: DashboardWidget } = $props();

  let imageUrl = $derived(dashboardWidgetImageUrl(widget.id, widget.updated_at));
  let failedImageUrl = $state("");
  let hasImage = $derived(widget.config.has_image === true);
  let imageAvailable = $derived(hasImage && failedImageUrl !== imageUrl);
  let fit = $derived(widget.config.fit === "cover" ? "cover" : "contain");
  let caption = $derived(
    typeof widget.config.caption === "string" ? widget.config.caption.trim() : "",
  );
  let altText = $derived(
    typeof widget.config.alt_text === "string" && widget.config.alt_text.trim()
      ? widget.config.alt_text.trim()
      : "Dashboard image",
  );
  let visualizationMode = $derived.by<AudioVisualizationMode>(() => {
    const configured = widget.config.mode;
    return isAudioVisualizationMode(configured) && configured !== "off"
      ? configured
      : (AUDIO_VISUALIZATION_STYLES[0]?.id ?? "spectrum");
  });
  let visualizationBackground = $derived(
    typeof widget.config.background === "string"
      ? widget.config.background
      : "transparent",
  );
</script>

{#if widget.kind === "image-frame"}
  <figure
    class={["dashboard-image-frame", `fit-${fit}`]}
    data-od-id={`dashboard-image-frame-${widget.id}`}
  >
    {#if imageAvailable}
      <img
        src={imageUrl}
        alt={altText}
        onerror={() => (failedImageUrl = imageUrl)}
      />
      {#if caption}<figcaption>{caption}</figcaption>{/if}
    {:else}
      <div class="dashboard-media-empty">
        <ImageIcon size={28} strokeWidth={1.5} aria-hidden="true" />
        <strong>Add an image</strong>
        <span>Right-click this widget and choose Edit.</span>
      </div>
    {/if}
  </figure>
{:else}
  <section
    class={[
      "dashboard-music-visualizer",
      `background-${visualizationBackground}`,
    ]}
    data-od-id={`dashboard-music-visualizer-${widget.id}`}
  >
    {#if visualizationBackground === "artwork" && podcastPlayer.artworkUrl}
      <img
        class="music-visualizer-background"
        src={podcastPlayer.artworkUrl}
        alt=""
      />
    {:else if visualizationBackground === "custom" && imageAvailable}
      <img
        class="music-visualizer-background"
        src={imageUrl}
        alt=""
        onerror={() => (failedImageUrl = imageUrl)}
      />
    {/if}
    <AudioVisualization mode={visualizationMode} embedded />
    <div class="music-visualizer-status">
      <AudioLines size={16} strokeWidth={1.8} aria-hidden="true" />
      <span>
        <strong>{podcastPlayer.title || "Nothing playing"}</strong>
        <small>{podcastPlayer.subtitle || "Start audio to animate this widget"}</small>
      </span>
    </div>
  </section>
{/if}

<style>
  .dashboard-image-frame,
  .dashboard-music-visualizer {
    position: relative;
    min-width: 0;
    min-height: 100%;
    height: 100%;
    overflow: hidden;
  }

  .dashboard-image-frame {
    display: grid;
    margin: 0;
    background: color-mix(in oklch, var(--fg) 4%, transparent);
  }

  .dashboard-image-frame img {
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .dashboard-image-frame.fit-contain img {
    object-fit: contain;
  }

  .dashboard-image-frame.fit-cover img {
    object-fit: cover;
  }

  .dashboard-image-frame figcaption {
    position: absolute;
    right: 10px;
    bottom: 10px;
    left: 10px;
    z-index: 2;
    width: fit-content;
    max-width: calc(100% - 20px);
    padding: 7px 9px;
    background: color-mix(in oklch, var(--surface) 88%, transparent);
    box-shadow: var(--shadow);
    color: var(--fg);
    font-size: 12px;
    line-height: 1.45;
    text-wrap: pretty;
  }

  .dashboard-media-empty {
    min-height: 100%;
    display: grid;
    align-content: center;
    justify-items: center;
    gap: 7px;
    padding: 24px;
    color: var(--muted);
    text-align: center;
  }

  .dashboard-media-empty strong {
    color: var(--fg);
    font-size: 14px;
  }

  .dashboard-media-empty span {
    max-width: 34ch;
    font-size: 12px;
    line-height: 1.5;
    text-wrap: pretty;
  }

  .dashboard-music-visualizer {
    isolation: isolate;
    background: transparent;
  }

  .dashboard-music-visualizer.background-surface,
  .dashboard-music-visualizer.background-custom,
  .dashboard-music-visualizer.background-artwork {
    background: var(--page-surface, var(--surface));
  }

  .music-visualizer-background {
    position: absolute;
    inset: 0;
    z-index: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: brightness(0.4) saturate(0.82);
  }

  .music-visualizer-status {
    position: absolute;
    right: 12px;
    bottom: 12px;
    left: 12px;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
    padding: 8px 10px;
    background: color-mix(in oklch, var(--surface) 84%, transparent);
    box-shadow: var(--shadow);
    color: var(--fg);
  }

  .music-visualizer-status > span {
    min-width: 0;
    display: grid;
    gap: 2px;
  }

  .music-visualizer-status strong,
  .music-visualizer-status small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .music-visualizer-status strong {
    font-size: 12px;
    font-weight: 620;
  }

  .music-visualizer-status small {
    color: var(--muted);
    font-size: 10px;
  }
</style>

import {
  jellyfinMusicAudioUrl,
  jellyfinMusicImageUrl,
  podcastAudioUrl,
  savePodcastProgress,
  startJellyfinPlayback,
  stopJellyfinPlayback,
  updateJellyfinPlayback,
  youtubeDownloadPreviewUrl,
  type JellyfinMusicItem,
  type PodcastEpisode,
  type YoutubeDownloadJob,
} from "$lib/api";
import {
  isAudioVisualizationMode,
  type AudioVisualizationMode,
} from "$lib/audioVisualizationCatalog";

export type { AudioVisualizationMode } from "$lib/audioVisualizationCatalog";

/**
 * Playback state for the Podcasts page.
 *
 * This lives at module scope, and the single `<audio>` element it drives is rendered by
 * the application shell rather than by `PodcastsPage.svelte`. Pandan swaps pages inside an
 * `{#if activeSection}` chain, so a component-owned audio element would be destroyed the
 * moment someone navigated to Tasks and playback would stop mid-sentence.
 *
 * Progress is written on a timer rather than on `timeupdate`, which fires roughly four
 * times a second.
 */

/** How often a resume position is written while playing. */
const PROGRESS_INTERVAL_MS = 15_000;
/** Treat an episode as finished within this many seconds of the end. */
const COMPLETION_TAIL_SECONDS = 15;
/** Past this point, "previous" restarts the episode rather than stepping back a track. */
const RESTART_THRESHOLD_SECONDS = 5;
/** Volume is a device preference rather than account state, so it lives in storage. */
const VOLUME_STORAGE_KEY = "pandan:podcast-volume";
/** The ambient visualizer is also a device preference and defaults to no motion. */
const VISUALIZATION_STORAGE_KEY = "pandan:audio-visualization";
const DEFAULT_VISUALIZATION_VISIBILITY = 0.34;
const DEFAULT_VISUALIZATION_INTENSITY = 1;
const DEFAULT_VISUALIZATION_BRIGHTNESS = 1;
const DEFAULT_VISUALIZATION_CONTRAST = 1;
const DEFAULT_VISUALIZATION_HUE = 145;
const DEFAULT_VISUALIZATION_PALETTE = "mono";
const DEFAULT_VISUALIZATION_RESPONSE = "balanced";
const VISUALIZATION_FFT_SIZE = 512;
/** Where the slider starts, leaving headroom above unity for a quiet recording. */
const DEFAULT_VOLUME = 0.8;
/** The ceiling, as a multiple of the source level. Above 1 the boost needs a gain node. */
const MAX_VOLUME = 2;

/** How far the rewind control moves. Exported so its label cannot drift from its behavior. */
export const SKIP_BACK_SECONDS = 15;
/** How far the forward control moves. Exported for the same reason. */
export const SKIP_FORWARD_SECONDS = 30;

/** The volume ceiling, exported so the slider cannot drift from what the player accepts. */
export const MAX_PLAYBACK_VOLUME = MAX_VOLUME;
export const MIN_VISUALIZATION_VISIBILITY = 0.1;
export const MAX_VISUALIZATION_VISIBILITY = 0.9;
export const MIN_VISUALIZATION_INTENSITY = 0.5;
export const MAX_VISUALIZATION_INTENSITY = 2.5;
export const MIN_VISUALIZATION_BRIGHTNESS = 0.5;
export const MAX_VISUALIZATION_BRIGHTNESS = 2;
export const MIN_VISUALIZATION_CONTRAST = 0.5;
export const MAX_VISUALIZATION_CONTRAST = 2;

export type AudioVisualizationPalette = "mono" | "pandan" | "signal" | "prism";
export type AudioVisualizationResponse = "calm" | "balanced" | "reactive";

type AudioVisualizationSettings = {
  mode: AudioVisualizationMode;
  visibility: number;
  intensity: number;
  brightness: number;
  contrast: number;
  hue: number;
  palette: AudioVisualizationPalette;
  response: AudioVisualizationResponse;
};

class PodcastPlayer {
  episode = $state<PodcastEpisode | null>(null);
  track = $state<JellyfinMusicItem | null>(null);
  downloadedAudio = $state<YoutubeDownloadJob | null>(null);
  playing = $state(false);
  currentTime = $state(0);
  duration = $state(0);
  playbackRate = $state(1);
  volume = $state(DEFAULT_VOLUME);
  muted = $state(false);
  buffering = $state(false);
  error = $state("");
  visualizationMode = $state<AudioVisualizationMode>("off");
  visualizationVisibility = $state(DEFAULT_VISUALIZATION_VISIBILITY);
  visualizationIntensity = $state(DEFAULT_VISUALIZATION_INTENSITY);
  visualizationBrightness = $state(DEFAULT_VISUALIZATION_BRIGHTNESS);
  visualizationContrast = $state(DEFAULT_VISUALIZATION_CONTRAST);
  visualizationHue = $state(DEFAULT_VISUALIZATION_HUE);
  visualizationPalette = $state<AudioVisualizationPalette>(
    DEFAULT_VISUALIZATION_PALETTE,
  );
  visualizationResponse = $state<AudioVisualizationResponse>(
    DEFAULT_VISUALIZATION_RESPONSE,
  );
  /** Episodes to advance through when the current one ends. */
  upNext = $state<PodcastEpisode[]>([]);
  /** Episodes already stepped past in this run, oldest first, so playback can go back. */
  played = $state<PodcastEpisode[]>([]);
  /** Jellyfin tracks queued behind the current track. */
  musicUpNext = $state<JellyfinMusicItem[]>([]);
  /** Jellyfin tracks already heard in this run, oldest first. */
  musicPlayed = $state<JellyfinMusicItem[]>([]);

  #element: HTMLAudioElement | null = null;
  #lastWrittenAt = 0;
  #lastWrittenPosition = -1;
  /** Set while seeking to a resume point, so the seek is not written straight back. */
  #restoring = false;
  /** The position to seek to once metadata lands. */
  #resumeTo = 0;
  /**
   * Which episode the element's `src` currently points at.
   *
   * Tracked apart from `episode` because the two legitimately disagree. Playing an episode
   * that is not cached yet answers 409: `episode` stays set while the source is unusable,
   * so the retry after the download lands has to reload the element rather than resume it,
   * and has to adopt the reloaded record that finally carries a duration.
   */
  #loadedEpisodeId = "";
  #loadedMusicKey = "";
  #loadedDownloadId = "";
  #musicPlaySessionId = "";
  /**
   * One shared Web Audio graph for amplification and the optional visualizer.
   *
   * A media element can only be wrapped in one `MediaElementAudioSourceNode`, so the
   * analyser sits in the same graph as the existing 200% gain control. The context is
   * still created only from a play, volume, or visualizer interaction — never at mount.
   */
  #audioContext: AudioContext | null = null;
  #mediaSource: MediaElementAudioSourceNode | null = null;
  #analyser: AnalyserNode | null = null;
  #splitter: ChannelSplitterNode | null = null;
  #leftAnalyser: AnalyserNode | null = null;
  #rightAnalyser: AnalyserNode | null = null;
  #gain: GainNode | null = null;

  /** Binds the shell's audio element. Called once, when the shell mounts. */
  attach(element: HTMLAudioElement) {
    this.#element = element;
    element.playbackRate = this.playbackRate;
    this.volume = readStoredVolume();
    const visualization = readStoredVisualizationSettings();
    this.visualizationMode = visualization.mode;
    this.visualizationVisibility = visualization.visibility;
    this.visualizationIntensity = visualization.intensity;
    this.visualizationBrightness = visualization.brightness;
    this.visualizationContrast = visualization.contrast;
    this.visualizationHue = visualization.hue;
    this.visualizationPalette = visualization.palette;
    this.visualizationResponse = visualization.response;
    // Mount is not a user gesture. A stored boost waits for the first play rather than
    // opening an AudioContext here, which would start suspended and play silently.
    this.#applyVolume(false);
  }

  get element(): HTMLAudioElement | null {
    return this.#element;
  }

  get isReady(): boolean {
    return (
      this.track !== null ||
      this.downloadedAudio?.status === "complete" ||
      this.episode?.download_status === "ready"
    );
  }

  /** True when the element's source matches the current episode, so `toggle()` resumes it. */
  get isLoaded(): boolean {
    if (this.track) return this.#loadedMusicKey === musicKey(this.track);
    if (this.downloadedAudio) {
      return this.#loadedDownloadId === this.downloadedAudio.id;
    }
    return this.episode !== null && this.#loadedEpisodeId === this.episode.id;
  }

  get hasNext(): boolean {
    if (this.downloadedAudio) return false;
    return this.track ? this.musicUpNext.length > 0 : this.upNext.length > 0;
  }

  get hasPrevious(): boolean {
    if (this.downloadedAudio) return false;
    return this.track ? this.musicPlayed.length > 0 : this.played.length > 0;
  }

  get source(): "podcast" | "jellyfin" | "download" | null {
    if (this.track) return "jellyfin";
    if (this.downloadedAudio) return "download";
    if (this.episode) return "podcast";
    return null;
  }

  get title(): string {
    return (
      this.track?.name ??
      this.downloadedAudio?.title ??
      this.episode?.title ??
      ""
    );
  }

  get subtitle(): string {
    if (this.track) {
      return [this.track.artist, this.track.album].filter(Boolean).join(" · ");
    }
    if (this.downloadedAudio) {
      return [
        this.downloadedAudio.channel_name || "YouTube",
        this.downloadedAudio.output_format.toUpperCase(),
      ].join(" · ");
    }
    return this.episode?.podcast_title ?? "";
  }

  get artworkUrl(): string {
    if (this.track?.image_item_id) {
      return jellyfinMusicImageUrl(
        this.track.image_item_id,
        this.track.library_id,
        this.track.image_tag,
      );
    }
    return this.episode
      ? `/api/podcasts/${this.episode.podcast_id}/artwork`
      : "";
  }

  /** The level the listener actually hears, for the slider and the volume icon. */
  get effectiveVolume(): number {
    return this.muted ? 0 : this.volume;
  }

  /** The analyser's stable sample count, even before its graph is created. */
  get visualizationBinCount(): number {
    return VISUALIZATION_FFT_SIZE / 2;
  }

  get visualizationSampleRate(): number {
    return this.#audioContext?.sampleRate ?? 48_000;
  }

  /**
   * Selects the visual treatment and builds the graph from the user's control gesture.
   * Returns false when the browser cannot expose Web Audio, leaving the visualizer off.
   */
  setVisualizationMode(mode: AudioVisualizationMode): boolean {
    if (mode !== "off" && !this.#ensureAudioGraph()) {
      this.visualizationMode = "off";
      this.#storeVisualizationSettings();
      return false;
    }
    this.visualizationMode = mode;
    // Creating the shared analyser graph changes the element's audio route. Reapply the
    // listener's level immediately so enabling it never leaves element and gain out of sync.
    this.#applyVolume();
    this.#storeVisualizationSettings();
    if (mode !== "off") {
      void this.#audioContext?.resume().catch(() => undefined);
    }
    return true;
  }

  setVisualizationVisibility(value: number) {
    this.visualizationVisibility = clamp(
      value,
      MIN_VISUALIZATION_VISIBILITY,
      MAX_VISUALIZATION_VISIBILITY,
    );
    this.#storeVisualizationSettings();
  }

  setVisualizationIntensity(value: number) {
    this.visualizationIntensity = clamp(
      value,
      MIN_VISUALIZATION_INTENSITY,
      MAX_VISUALIZATION_INTENSITY,
    );
    this.#storeVisualizationSettings();
  }

  setVisualizationBrightness(value: number) {
    this.visualizationBrightness = clamp(
      value,
      MIN_VISUALIZATION_BRIGHTNESS,
      MAX_VISUALIZATION_BRIGHTNESS,
    );
    this.#storeVisualizationSettings();
  }

  setVisualizationContrast(value: number) {
    this.visualizationContrast = clamp(
      value,
      MIN_VISUALIZATION_CONTRAST,
      MAX_VISUALIZATION_CONTRAST,
    );
    this.#storeVisualizationSettings();
  }

  setVisualizationHue(value: number) {
    this.visualizationHue = clamp(value, 0, 360);
    this.#storeVisualizationSettings();
  }

  setVisualizationPalette(palette: AudioVisualizationPalette) {
    this.visualizationPalette = palette;
    this.#storeVisualizationSettings();
  }

  setVisualizationResponse(response: AudioVisualizationResponse) {
    this.visualizationResponse = response;
    for (const analyser of [
      this.#analyser,
      this.#leftAnalyser,
      this.#rightAnalyser,
    ]) {
      if (analyser) {
        analyser.smoothingTimeConstant = visualizationSmoothing(response);
      }
    }
    this.#storeVisualizationSettings();
  }

  resetVisualizationSettings() {
    this.visualizationMode = "off";
    this.visualizationVisibility = DEFAULT_VISUALIZATION_VISIBILITY;
    this.visualizationIntensity = DEFAULT_VISUALIZATION_INTENSITY;
    this.visualizationBrightness = DEFAULT_VISUALIZATION_BRIGHTNESS;
    this.visualizationContrast = DEFAULT_VISUALIZATION_CONTRAST;
    this.visualizationHue = DEFAULT_VISUALIZATION_HUE;
    this.visualizationPalette = DEFAULT_VISUALIZATION_PALETTE;
    this.visualizationResponse = DEFAULT_VISUALIZATION_RESPONSE;
    for (const analyser of [
      this.#analyser,
      this.#leftAnalyser,
      this.#rightAnalyser,
    ]) {
      if (analyser) {
        analyser.smoothingTimeConstant = visualizationSmoothing(
          DEFAULT_VISUALIZATION_RESPONSE,
        );
      }
    }
    this.#storeVisualizationSettings();
  }

  /** Copies the current frequency bins for the shell canvas without exposing the node. */
  readVisualizationFrequency(target: Uint8Array<ArrayBuffer>): boolean {
    const analyser = this.#analyser;
    if (!analyser || target.length !== analyser.frequencyBinCount) return false;
    analyser.getByteFrequencyData(target);
    return true;
  }

  /** Copies the current waveform samples for the shell canvas. */
  readVisualizationWaveform(target: Uint8Array<ArrayBuffer>): boolean {
    const analyser = this.#analyser;
    if (!analyser || target.length !== analyser.frequencyBinCount) return false;
    analyser.getByteTimeDomainData(target);
    return true;
  }

  readVisualizationStereoFrequency(
    left: Uint8Array<ArrayBuffer>,
    right: Uint8Array<ArrayBuffer>,
  ): boolean {
    if (!this.#ensureStereoAnalysers()) return false;
    const leftAnalyser = this.#leftAnalyser;
    const rightAnalyser = this.#rightAnalyser;
    if (
      !leftAnalyser ||
      !rightAnalyser ||
      left.length !== leftAnalyser.frequencyBinCount ||
      right.length !== rightAnalyser.frequencyBinCount
    ) {
      return false;
    }
    leftAnalyser.getByteFrequencyData(left);
    rightAnalyser.getByteFrequencyData(right);
    if (!hasAnalyserSignal(right)) right.set(left);
    return true;
  }

  readVisualizationStereoWaveform(
    left: Uint8Array<ArrayBuffer>,
    right: Uint8Array<ArrayBuffer>,
  ): boolean {
    if (!this.#ensureStereoAnalysers()) return false;
    const leftAnalyser = this.#leftAnalyser;
    const rightAnalyser = this.#rightAnalyser;
    if (
      !leftAnalyser ||
      !rightAnalyser ||
      left.length !== leftAnalyser.frequencyBinCount ||
      right.length !== rightAnalyser.frequencyBinCount
    ) {
      return false;
    }
    leftAnalyser.getByteTimeDomainData(left);
    rightAnalyser.getByteTimeDomainData(right);
    if (!hasWaveformSignal(right)) right.set(left);
    return true;
  }

  /**
   * Starts an episode, resuming from its stored position.
   *
   * Only downloaded episodes can play: audio is served from the instance's own disk, and
   * the endpoint answers 409 for anything not yet cached.
   */
  async play(
    episode: PodcastEpisode,
    upNext: PodcastEpisode[] = [],
    played: PodcastEpisode[] = [],
  ) {
    const element = this.#element;
    if (!element) return;
    this.error = "";
    this.upNext = upNext.filter((item) => item.id !== episode.id);
    this.played = played.filter((item) => item.id !== episode.id);

    const switching =
      this.episode?.id !== episode.id ||
      this.track !== null ||
      this.downloadedAudio !== null;
    if (switching) {
      await this.#flush(true);
      await this.#stopMusic();
    }

    // Adopt the caller's record even for the episode already on screen: a row re-read
    // after its download finished carries the real duration and a `ready` status that the
    // copy captured before the transfer does not.
    this.episode = episode;
    this.track = null;
    this.downloadedAudio = null;
    this.musicUpNext = [];
    this.musicPlayed = [];
    this.#loadedMusicKey = "";
    this.#loadedDownloadId = "";

    if (switching || this.#loadedEpisodeId !== episode.id) {
      // Reloading the episode already on screen keeps whatever is known live. An error
      // mid-playback clears the source, and the retry must not drop the listener back to
      // the last position written to the server, or discard a duration already measured.
      const resumeFrom = switching
        ? episode.position_seconds
        : Math.max(episode.position_seconds, Math.floor(this.currentTime));
      this.currentTime = resumeFrom;
      this.duration = switching
        ? (episode.duration_seconds ?? 0)
        : this.duration || (episode.duration_seconds ?? 0);
      this.#lastWrittenPosition = resumeFrom;
      this.#resumeTo = resumeFrom;
      this.#restoring = resumeFrom > 0;
      this.#loadedEpisodeId = episode.id;
      element.src = podcastAudioUrl(episode.id);
      element.load();
    } else if (this.duration <= 0 && episode.duration_seconds) {
      this.duration = episode.duration_seconds;
    }

    try {
      this.buffering = true;
      // Playing is a user gesture, which is where a boost stored from a previous visit
      // gets its audio graph.
      this.#applyVolume();
      await element.play();
      this.#setMediaSession();
    } catch {
      this.error = "This episode could not be played.";
      this.playing = false;
    } finally {
      this.buffering = false;
    }
  }

  /** Starts a Jellyfin audio item through Pandan's authenticated live proxy. */
  async playMusic(
    track: JellyfinMusicItem,
    upNext: JellyfinMusicItem[] = [],
    played: JellyfinMusicItem[] = [],
  ) {
    const element = this.#element;
    if (!element) return;
    const key = musicKey(track);
    const switching =
      this.#loadedMusicKey !== key ||
      this.episode !== null ||
      this.downloadedAudio !== null;
    this.error = "";
    this.musicUpNext = upNext.filter((item) => musicKey(item) !== key);
    this.musicPlayed = played.filter((item) => musicKey(item) !== key);

    if (switching) {
      await this.#flush(true);
      await this.#stopMusic();
    }

    this.track = track;
    this.episode = null;
    this.downloadedAudio = null;
    this.upNext = [];
    this.played = [];
    this.playbackRate = 1;
    element.playbackRate = 1;

    if (switching) {
      this.currentTime = 0;
      this.duration = track.duration_seconds ?? 0;
      this.#lastWrittenPosition = -1;
      this.#resumeTo = 0;
      this.#restoring = false;
      this.#loadedEpisodeId = "";
      this.#loadedDownloadId = "";
      this.#loadedMusicKey = key;
      this.#musicPlaySessionId = "";
      element.src = jellyfinMusicAudioUrl(track.id, track.library_id);
      element.load();
    }

    try {
      this.buffering = true;
      this.#applyVolume();
      await element.play();
      this.#setMediaSession();
      if (!this.#musicPlaySessionId) void this.#startMusicReport(track, key);
    } catch {
      this.error = "This track could not be played.";
      this.playing = false;
    } finally {
      this.buffering = false;
    }
  }

  /** Plays a completed audio file from the private Downloads library. */
  async playDownload(job: YoutubeDownloadJob) {
    const element = this.#element;
    if (!element) return;
    if (job.status !== "complete" || job.media_kind !== "audio") {
      this.error = "Only completed audio downloads can be played.";
      return;
    }

    const switching =
      this.#loadedDownloadId !== job.id ||
      this.episode !== null ||
      this.track !== null;
    this.error = "";
    if (switching) {
      await this.#flush(true);
      await this.#stopMusic();
    }

    this.downloadedAudio = job;
    this.episode = null;
    this.track = null;
    this.upNext = [];
    this.played = [];
    this.musicUpNext = [];
    this.musicPlayed = [];
    this.playbackRate = 1;
    element.playbackRate = 1;

    if (switching) {
      this.currentTime = 0;
      this.duration = job.duration_seconds ?? 0;
      this.#lastWrittenPosition = -1;
      this.#resumeTo = 0;
      this.#restoring = false;
      this.#loadedEpisodeId = "";
      this.#loadedMusicKey = "";
      this.#loadedDownloadId = job.id;
      this.#musicPlaySessionId = "";
      element.src = youtubeDownloadPreviewUrl(job.id);
      element.load();
    }

    try {
      this.buffering = true;
      this.#applyVolume();
      await element.play();
      this.#setMediaSession();
    } catch {
      this.error = "This downloaded audio could not be played.";
      this.playing = false;
    } finally {
      this.buffering = false;
    }
  }

  isMusicQueued(track: JellyfinMusicItem): boolean {
    const key = musicKey(track);
    return this.musicUpNext.some((item) => musicKey(item) === key);
  }

  queueMusic(track: JellyfinMusicItem): boolean {
    if (this.isMusicQueued(track)) return false;
    this.musicUpNext = [...this.musicUpNext, track];
    return true;
  }

  removeQueuedMusic(track: JellyfinMusicItem): boolean {
    const key = musicKey(track);
    const next = this.musicUpNext.filter((item) => musicKey(item) !== key);
    if (next.length === this.musicUpNext.length) return false;
    this.musicUpNext = next;
    return true;
  }

  moveQueuedMusic(track: JellyfinMusicItem, offset: -1 | 1): boolean {
    const index = this.musicUpNext.findIndex(
      (item) => musicKey(item) === musicKey(track),
    );
    const target = index + offset;
    if (index < 0 || target < 0 || target >= this.musicUpNext.length) {
      return false;
    }
    const next = [...this.musicUpNext];
    [next[index], next[target]] = [next[target], next[index]];
    this.musicUpNext = next;
    return true;
  }

  clearMusicQueue() {
    this.musicUpNext = [];
  }

  async playQueuedMusic(track: JellyfinMusicItem) {
    const index = this.musicUpNext.findIndex(
      (item) => musicKey(item) === musicKey(track),
    );
    if (index < 0) return;
    const next = this.musicUpNext[index];
    if (!next) return;
    const remaining = this.musicUpNext.filter(
      (_, itemIndex) => itemIndex !== index,
    );
    const current = this.track;
    await this.playMusic(
      next,
      remaining,
      current ? [...this.musicPlayed, current] : this.musicPlayed,
    );
  }

  async toggle() {
    const element = this.#element;
    if (!element || (!this.episode && !this.track && !this.downloadedAudio)) {
      return;
    }
    if (element.paused) {
      try {
        await element.play();
        if (this.track) void this.#writeMusic("progress", true);
      } catch {
        this.error = this.track
          ? "This track could not be played."
          : this.downloadedAudio
            ? "This downloaded audio could not be played."
            : "This episode could not be played.";
      }
    } else {
      element.pause();
      await this.#flush(true);
    }
  }

  /** Advances to the next queued episode, keeping the current one reachable by going back. */
  async playNext() {
    if (this.downloadedAudio) return;
    if (this.track) {
      const [next, ...rest] = this.musicUpNext;
      if (!next) return;
      const current = this.track;
      await this.playMusic(next, rest, [...this.musicPlayed, current]);
      return;
    }
    const [next, ...rest] = this.upNext;
    if (!next) return;
    const current = this.episode;
    await this.play(
      next,
      rest,
      current ? [...this.played, current] : this.played,
    );
  }

  /**
   * Steps back one episode, or restarts the current one.
   *
   * Restarting first past a few seconds in is the convention every podcast client shares,
   * and it keeps a mis-tapped control from losing the listener's place.
   */
  async playPrevious() {
    if (this.downloadedAudio) {
      this.seek(0);
      return;
    }
    if (this.currentTime > RESTART_THRESHOLD_SECONDS) {
      this.seek(0);
      return;
    }
    if (this.track) {
      const played = [...this.musicPlayed];
      const prior = played.pop();
      if (!prior) {
        this.seek(0);
        return;
      }
      const current = this.track;
      await this.playMusic(prior, [current, ...this.musicUpNext], played);
      return;
    }
    const played = [...this.played];
    const prior = played.pop();
    if (!prior) {
      this.seek(0);
      return;
    }
    const current = this.episode;
    await this.play(
      prior,
      current ? [current, ...this.upNext] : this.upNext,
      played,
    );
  }

  seek(seconds: number) {
    const element = this.#element;
    if (!element || (!this.episode && !this.track && !this.downloadedAudio)) {
      return;
    }
    const bounded = Math.min(Math.max(seconds, 0), this.duration || seconds);
    element.currentTime = bounded;
    this.currentTime = bounded;
    void this.#flush(true);
  }

  skip(seconds: number) {
    this.seek(this.currentTime + seconds);
  }

  setPlaybackRate(rate: number) {
    this.playbackRate = rate;
    if (this.#element) this.#element.playbackRate = rate;
  }

  setVolume(value: number) {
    const bounded = Math.min(Math.max(value, 0), MAX_VOLUME);
    this.volume = bounded;
    this.muted = bounded === 0;
    this.#applyVolume();
    storeVolume(bounded);
  }

  toggleMuted() {
    // Unmuting a slider that was dragged to zero would still be silent, so give it a level.
    if (this.muted && this.volume === 0) this.volume = DEFAULT_VOLUME;
    this.muted = !this.muted;
    this.#applyVolume();
    if (!this.muted) storeVolume(this.volume);
  }

  /** Stops playback and releases the element without writing a completion. */
  async close() {
    const element = this.#element;
    await this.#flush(true);
    await this.#stopMusic();
    if (element) {
      element.pause();
      element.removeAttribute("src");
      element.load();
    }
    this.episode = null;
    this.track = null;
    this.downloadedAudio = null;
    this.playing = false;
    this.currentTime = 0;
    this.duration = 0;
    this.upNext = [];
    this.played = [];
    this.musicUpNext = [];
    this.musicPlayed = [];
    this.#loadedEpisodeId = "";
    this.#loadedMusicKey = "";
    this.#loadedDownloadId = "";
    if ("mediaSession" in navigator) navigator.mediaSession.metadata = null;
  }

  // --- element event handlers, wired up by the shell -------------------------

  handlePlay() {
    this.playing = true;
    this.error = "";
    if (this.track) void this.#writeMusic("progress", true);
  }

  handlePause() {
    this.playing = false;
    if (this.track) void this.#writeMusic("progress", true);
  }

  handleLoadedMetadata() {
    const element = this.#element;
    if (!element) return;
    this.handleDurationChange();
    element.playbackRate = this.playbackRate;
    // A live streamed source can publish fresh media state after `load()`. Keep the
    // element/gain pair aligned for Jellyfin as soon as its metadata becomes available.
    this.#applyVolume(false);
    // Restore the resume point only once metadata is known, or the seek is ignored.
    if (this.#restoring) {
      element.currentTime = Math.min(
        this.#resumeTo,
        Math.max(this.duration - 1, 0),
      );
      this.#restoring = false;
    }
  }

  /**
   * Takes the length the element reports.
   *
   * Bound to `durationchange` as well as `loadedmetadata`: browsers publish an estimate
   * for a streamed MP3 and correct it later, and a feed that declared no duration at all
   * would otherwise sit at `0:00` until the page was reloaded.
   */
  handleDurationChange() {
    const element = this.#element;
    if (!element) return;
    if (Number.isFinite(element.duration) && element.duration > 0) {
      this.duration = element.duration;
    }
  }

  handleTimeUpdate() {
    const element = this.#element;
    if (!element) return;
    this.currentTime = element.currentTime;
    if (Date.now() - this.#lastWrittenAt >= PROGRESS_INTERVAL_MS) {
      void this.#flush();
    }
  }

  handleWaiting() {
    this.buffering = true;
  }

  handlePlaying() {
    this.buffering = false;
  }

  handleError() {
    this.playing = false;
    this.buffering = false;
    // The source is unusable, so a retry has to reload rather than resume it.
    if (this.track) {
      this.#loadedMusicKey = "";
      this.error = "This track could not be streamed from Jellyfin.";
    } else if (this.downloadedAudio) {
      this.#loadedDownloadId = "";
      this.error = "This downloaded audio is no longer available.";
    } else {
      this.#loadedEpisodeId = "";
      this.error =
        "This episode is not available on this instance yet. Download it first.";
    }
  }

  /** Marks the episode finished and advances to whatever is queued behind it. */
  async handleEnded() {
    if (this.downloadedAudio) {
      this.playing = false;
      return;
    }
    if (this.track) {
      this.playing = false;
      await this.#stopMusic();
      await this.playNext();
      return;
    }
    const finished = this.episode;
    this.playing = false;
    if (finished) {
      await this.#write(
        finished,
        this.duration || finished.position_seconds,
        true,
      );
    }
    await this.playNext();
  }

  /**
   * Writes the current position immediately.
   *
   * Called on pause, seek, episode change, and page hide, so a listener never loses more
   * than the interval's worth of progress.
   */
  async flushNow() {
    await this.#flush(true);
  }

  #applyVolume(allowGraph = true) {
    const element = this.#element;
    if (!element) return;
    const level = this.muted ? 0 : this.volume;
    // Build the graph for amplification or visualization, then keep using it: the element
    // sits at 1 and the gain node carries the whole level, so the two never multiply.
    const needsGraph = level > 1 || this.visualizationMode !== "off";
    const gain =
      needsGraph && allowGraph
        ? this.#ensureAudioGraph()
          ? this.#gain
          : null
        : this.#gain;
    element.muted = this.muted;
    if (gain) {
      element.volume = 1;
      gain.gain.value = level;
      void this.#audioContext?.resume().catch(() => undefined);
      return;
    }
    element.volume = Math.min(level, 1);
  }

  /** Routes the element through analyser and gain nodes exactly once. */
  #ensureAudioGraph(): boolean {
    if (this.#gain && this.#analyser && this.#mediaSource) return true;
    const element = this.#element;
    if (!element || typeof AudioContext !== "function") return false;
    let context: AudioContext | null = null;
    try {
      context = new AudioContext();
      const source = context.createMediaElementSource(element);
      const analyser = context.createAnalyser();
      const gain = context.createGain();
      analyser.fftSize = VISUALIZATION_FFT_SIZE;
      analyser.minDecibels = -90;
      analyser.maxDecibels = -18;
      analyser.smoothingTimeConstant = visualizationSmoothing(
        this.visualizationResponse,
      );
      source.connect(analyser);
      analyser.connect(gain);
      gain.connect(context.destination);
      // Once a MediaElementSource exists the graph owns the complete level. Initialize
      // both sides together so the previous element volume cannot attenuate it twice.
      element.volume = 1;
      element.muted = this.muted;
      gain.gain.value = this.muted ? 0 : this.volume;
      this.#audioContext = context;
      this.#mediaSource = source;
      this.#analyser = analyser;
      this.#gain = gain;
      return true;
    } catch {
      void context?.close().catch(() => undefined);
      // Without the graph the element still plays; it just cannot boost or visualize.
      return false;
    }
  }

  /** Adds analysis-only stereo branches to the existing media source. */
  #ensureStereoAnalysers(): boolean {
    if (this.#splitter && this.#leftAnalyser && this.#rightAnalyser)
      return true;
    if (!this.#ensureAudioGraph()) return false;
    const context = this.#audioContext;
    const source = this.#mediaSource;
    if (!context || !source) return false;
    try {
      const splitter = context.createChannelSplitter(2);
      const leftAnalyser = context.createAnalyser();
      const rightAnalyser = context.createAnalyser();
      for (const analyser of [leftAnalyser, rightAnalyser]) {
        analyser.fftSize = VISUALIZATION_FFT_SIZE;
        analyser.minDecibels = -90;
        analyser.maxDecibels = -18;
        analyser.smoothingTimeConstant = visualizationSmoothing(
          this.visualizationResponse,
        );
      }
      source.connect(splitter);
      splitter.connect(leftAnalyser, 0);
      splitter.connect(rightAnalyser, 1);
      this.#splitter = splitter;
      this.#leftAnalyser = leftAnalyser;
      this.#rightAnalyser = rightAnalyser;
      return true;
    } catch {
      return false;
    }
  }

  #storeVisualizationSettings() {
    storeVisualizationSettings({
      mode: this.visualizationMode,
      visibility: this.visualizationVisibility,
      intensity: this.visualizationIntensity,
      brightness: this.visualizationBrightness,
      contrast: this.visualizationContrast,
      hue: this.visualizationHue,
      palette: this.visualizationPalette,
      response: this.visualizationResponse,
    });
  }

  async #flush(force = false) {
    if (this.downloadedAudio) return;
    if (this.track) {
      await this.#writeMusic("progress", force);
      return;
    }
    const episode = this.episode;
    if (!episode) return;
    const position = Math.round(this.currentTime);
    if (position === this.#lastWrittenPosition) return;
    const completed =
      this.duration > 0 && position >= this.duration - COMPLETION_TAIL_SECONDS;
    await this.#write(episode, position, completed);
  }

  async #write(episode: PodcastEpisode, position: number, completed: boolean) {
    this.#lastWrittenAt = Date.now();
    this.#lastWrittenPosition = Math.round(position);
    try {
      await savePodcastProgress(episode.id, position, completed);
    } catch {
      // A dropped progress write is not worth interrupting playback for; the next
      // interval will carry the position forward.
    }
  }

  async #startMusicReport(track: JellyfinMusicItem, key: string) {
    try {
      const response = await startJellyfinPlayback({
        library_id: track.library_id,
        item_id: track.id,
        position_seconds: this.currentTime,
        is_paused: false,
      });
      if (this.track && musicKey(this.track) === key) {
        this.#musicPlaySessionId = response.play_session_id;
        this.#lastWrittenAt = Date.now();
        this.#lastWrittenPosition = Math.round(this.currentTime);
      }
    } catch {
      // Reporting is best effort; it never interrupts audio.
    }
  }

  async #writeMusic(mode: "progress" | "stop", force = false) {
    const track = this.track;
    const playSessionId = this.#musicPlaySessionId;
    if (!track || !playSessionId) return;
    const position = Math.round(this.currentTime);
    if (!force && position === this.#lastWrittenPosition) return;
    this.#lastWrittenAt = Date.now();
    this.#lastWrittenPosition = position;
    const update = {
      library_id: track.library_id,
      item_id: track.id,
      position_seconds: this.currentTime,
      is_paused: !this.playing,
      play_session_id: playSessionId,
    };
    try {
      if (mode === "stop") await stopJellyfinPlayback(update);
      else await updateJellyfinPlayback(update);
    } catch {
      // Reporting is best effort; the next interval can recover.
    }
  }

  async #stopMusic() {
    if (!this.track || !this.#musicPlaySessionId) return;
    await this.#writeMusic("stop", true);
    this.#musicPlaySessionId = "";
  }

  #setMediaSession() {
    if (!("mediaSession" in navigator) || typeof MediaMetadata !== "function") {
      return;
    }
    const artwork = this.artworkUrl;
    navigator.mediaSession.metadata = new MediaMetadata({
      title: this.title,
      artist: this.subtitle,
      album:
        this.track?.album ??
        this.episode?.podcast_title ??
        this.downloadedAudio?.channel_name ??
        "",
      artwork: artwork ? [{ src: artwork }] : [],
    });
    const handlers: Array<[MediaSessionAction, MediaSessionActionHandler]> = [
      ["play", () => void this.toggle()],
      ["pause", () => void this.toggle()],
      ["previoustrack", () => void this.playPrevious()],
      ["nexttrack", () => void this.playNext()],
      [
        "seekbackward",
        (details) => this.skip(-(details.seekOffset ?? SKIP_BACK_SECONDS)),
      ],
      [
        "seekforward",
        (details) => this.skip(details.seekOffset ?? SKIP_FORWARD_SECONDS),
      ],
      [
        "seekto",
        (details) => {
          if (details.seekTime !== undefined) this.seek(details.seekTime);
        },
      ],
    ];
    for (const [action, handler] of handlers) {
      try {
        navigator.mediaSession.setActionHandler(action, handler);
      } catch {
        // Browsers expose different subsets of the Media Session actions.
      }
    }
  }
}

function musicKey(track: JellyfinMusicItem): string {
  return `${track.library_id}:${track.id}`;
}

function readStoredVolume(): number {
  try {
    const stored = localStorage.getItem(VOLUME_STORAGE_KEY);
    if (stored === null) return DEFAULT_VOLUME;
    const value = Number(stored);
    return Number.isFinite(value) && value >= 0 && value <= MAX_VOLUME
      ? value
      : DEFAULT_VOLUME;
  } catch {
    return DEFAULT_VOLUME;
  }
}

function storeVolume(value: number) {
  try {
    localStorage.setItem(VOLUME_STORAGE_KEY, String(value));
  } catch {
    // Blocked storage is not worth interrupting playback for; the level still applies.
  }
}

function readStoredVisualizationSettings(): AudioVisualizationSettings {
  const defaults: AudioVisualizationSettings = {
    mode: "off",
    visibility: DEFAULT_VISUALIZATION_VISIBILITY,
    intensity: DEFAULT_VISUALIZATION_INTENSITY,
    brightness: DEFAULT_VISUALIZATION_BRIGHTNESS,
    contrast: DEFAULT_VISUALIZATION_CONTRAST,
    hue: DEFAULT_VISUALIZATION_HUE,
    palette: DEFAULT_VISUALIZATION_PALETTE,
    response: DEFAULT_VISUALIZATION_RESPONSE,
  };
  try {
    const stored = localStorage.getItem(VISUALIZATION_STORAGE_KEY);
    if (isAudioVisualizationMode(stored)) return { ...defaults, mode: stored };
    if (!stored) return defaults;
    const parsed: unknown = JSON.parse(stored);
    if (!isRecord(parsed)) return defaults;
    return {
      mode: isAudioVisualizationMode(parsed.mode) ? parsed.mode : defaults.mode,
      visibility: storedNumber(
        parsed.visibility,
        MIN_VISUALIZATION_VISIBILITY,
        MAX_VISUALIZATION_VISIBILITY,
        defaults.visibility,
      ),
      intensity: storedNumber(
        parsed.intensity,
        MIN_VISUALIZATION_INTENSITY,
        MAX_VISUALIZATION_INTENSITY,
        defaults.intensity,
      ),
      brightness: storedNumber(
        parsed.brightness,
        MIN_VISUALIZATION_BRIGHTNESS,
        MAX_VISUALIZATION_BRIGHTNESS,
        defaults.brightness,
      ),
      contrast: storedNumber(
        parsed.contrast,
        MIN_VISUALIZATION_CONTRAST,
        MAX_VISUALIZATION_CONTRAST,
        defaults.contrast,
      ),
      hue: storedNumber(parsed.hue, 0, 360, defaults.hue),
      palette: isVisualizationPalette(parsed.palette)
        ? parsed.palette
        : defaults.palette,
      response: isVisualizationResponse(parsed.response)
        ? parsed.response
        : defaults.response,
    };
  } catch {
    return defaults;
  }
}

function storeVisualizationSettings(settings: AudioVisualizationSettings) {
  try {
    localStorage.setItem(VISUALIZATION_STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // The selected settings still apply for this session when storage is blocked.
  }
}

function isVisualizationPalette(
  value: unknown,
): value is AudioVisualizationPalette {
  return (
    value === "mono" ||
    value === "pandan" ||
    value === "signal" ||
    value === "prism"
  );
}

function isVisualizationResponse(
  value: unknown,
): value is AudioVisualizationResponse {
  return value === "calm" || value === "balanced" || value === "reactive";
}

function visualizationSmoothing(response: AudioVisualizationResponse): number {
  if (response === "calm") return 0.9;
  if (response === "reactive") return 0.56;
  return 0.78;
}

function hasAnalyserSignal(values: Uint8Array<ArrayBuffer>): boolean {
  for (let index = 0; index < values.length; index += 8) {
    if ((values[index] ?? 0) > 0) return true;
  }
  return false;
}

function hasWaveformSignal(values: Uint8Array<ArrayBuffer>): boolean {
  for (let index = 0; index < values.length; index += 8) {
    if (Math.abs((values[index] ?? 128) - 128) > 1) return true;
  }
  return false;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function storedNumber(
  value: unknown,
  minimum: number,
  maximum: number,
  fallback: number,
): number {
  return typeof value === "number" && Number.isFinite(value)
    ? clamp(value, minimum, maximum)
    : fallback;
}

function clamp(value: number, minimum: number, maximum: number): number {
  if (!Number.isFinite(value)) return minimum;
  return Math.min(Math.max(value, minimum), maximum);
}

function rotatedHue(hue: number, offset: number): number {
  return (Math.round(hue) + offset + 360) % 360;
}

/** Produces canvas-ready OKLch colors from the listener's base hue. */
export function audioVisualizationPaletteColors(
  palette: AudioVisualizationPalette,
  hue: number,
): string[] {
  const base = clamp(hue, 0, 360);
  if (palette === "mono") {
    return [`oklch(79% 0.16 ${rotatedHue(base, 0)})`];
  }
  if (palette === "pandan") {
    return [
      `oklch(66% 0.12 ${rotatedHue(base, 0)})`,
      `oklch(79% 0.16 ${rotatedHue(base, 0)})`,
      `oklch(88% 0.1 ${rotatedHue(base, 0)})`,
    ];
  }
  if (palette === "signal") {
    return [
      `oklch(79% 0.16 ${rotatedHue(base, 0)})`,
      `oklch(82% 0.15 ${rotatedHue(base, 72)})`,
      `oklch(72% 0.18 ${rotatedHue(base, 232)})`,
    ];
  }
  return [0, 60, 120, 180, 240, 300].map(
    (offset) => `oklch(78% 0.17 ${rotatedHue(base, offset)})`,
  );
}

export const podcastPlayer = new PodcastPlayer();

/** Renders a duration as `H:MM:SS` or `M:SS`. */
export function formatPlaybackTime(totalSeconds: number): string {
  if (!Number.isFinite(totalSeconds) || totalSeconds < 0) return "0:00";
  const seconds = Math.floor(totalSeconds % 60);
  const minutes = Math.floor((totalSeconds / 60) % 60);
  const hours = Math.floor(totalSeconds / 3600);
  const paddedSeconds = String(seconds).padStart(2, "0");
  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, "0")}:${paddedSeconds}`;
  }
  return `${minutes}:${paddedSeconds}`;
}

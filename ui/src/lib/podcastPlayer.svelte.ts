import {
  podcastAudioUrl,
  savePodcastProgress,
  type PodcastEpisode,
} from "$lib/api";

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

class PodcastPlayer {
  episode = $state<PodcastEpisode | null>(null);
  playing = $state(false);
  currentTime = $state(0);
  duration = $state(0);
  playbackRate = $state(1);
  volume = $state(DEFAULT_VOLUME);
  muted = $state(false);
  buffering = $state(false);
  error = $state("");
  /** Episodes to advance through when the current one ends. */
  upNext = $state<PodcastEpisode[]>([]);
  /** Episodes already stepped past in this run, oldest first, so playback can go back. */
  played = $state<PodcastEpisode[]>([]);

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
  /**
   * The boost graph, built only once a level above 100% is asked for.
   *
   * `HTMLMediaElement.volume` is capped at 1, so amplification has to run through Web
   * Audio. The context is created from the slider interaction itself, which is a user
   * gesture, so it starts running rather than suspended.
   */
  #audioContext: AudioContext | null = null;
  #gain: GainNode | null = null;

  /** Binds the shell's audio element. Called once, when the shell mounts. */
  attach(element: HTMLAudioElement) {
    this.#element = element;
    element.playbackRate = this.playbackRate;
    this.volume = readStoredVolume();
    // Mount is not a user gesture. A stored boost waits for the first play rather than
    // opening an AudioContext here, which would start suspended and play silently.
    this.#applyVolume(false);
  }

  get element(): HTMLAudioElement | null {
    return this.#element;
  }

  get isReady(): boolean {
    return this.episode?.download_status === "ready";
  }

  /** True when the element's source matches the current episode, so `toggle()` resumes it. */
  get isLoaded(): boolean {
    return this.episode !== null && this.#loadedEpisodeId === this.episode.id;
  }

  get hasNext(): boolean {
    return this.upNext.length > 0;
  }

  get hasPrevious(): boolean {
    return this.played.length > 0;
  }

  /** The level the listener actually hears, for the slider and the volume icon. */
  get effectiveVolume(): number {
    return this.muted ? 0 : this.volume;
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

    const switching = this.episode?.id !== episode.id;
    if (switching) await this.#flush();

    // Adopt the caller's record even for the episode already on screen: a row re-read
    // after its download finished carries the real duration and a `ready` status that the
    // copy captured before the transfer does not.
    this.episode = episode;

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
    } catch {
      this.error = "This episode could not be played.";
      this.playing = false;
    } finally {
      this.buffering = false;
    }
  }

  async toggle() {
    const element = this.#element;
    if (!element || !this.episode) return;
    if (element.paused) {
      try {
        await element.play();
      } catch {
        this.error = "This episode could not be played.";
      }
    } else {
      element.pause();
      await this.#flush();
    }
  }

  /** Advances to the next queued episode, keeping the current one reachable by going back. */
  async playNext() {
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
    if (this.currentTime > RESTART_THRESHOLD_SECONDS) {
      this.seek(0);
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
    if (!element || !this.episode) return;
    const bounded = Math.min(Math.max(seconds, 0), this.duration || seconds);
    element.currentTime = bounded;
    this.currentTime = bounded;
    void this.#flush();
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
    await this.#flush();
    if (element) {
      element.pause();
      element.removeAttribute("src");
      element.load();
    }
    this.episode = null;
    this.playing = false;
    this.currentTime = 0;
    this.duration = 0;
    this.upNext = [];
    this.played = [];
    this.#loadedEpisodeId = "";
  }

  // --- element event handlers, wired up by the shell -------------------------

  handlePlay() {
    this.playing = true;
    this.error = "";
  }

  handlePause() {
    this.playing = false;
  }

  handleLoadedMetadata() {
    const element = this.#element;
    if (!element) return;
    this.handleDurationChange();
    element.playbackRate = this.playbackRate;
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
    this.#loadedEpisodeId = "";
    this.error =
      "This episode is not available on this instance yet. Download it first.";
  }

  /** Marks the episode finished and advances to whatever is queued behind it. */
  async handleEnded() {
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
    await this.#flush();
  }

  #applyVolume(allowGraph = true) {
    const element = this.#element;
    if (!element) return;
    const level = this.muted ? 0 : this.volume;
    // Build the graph the first time the level goes past unity, then keep using it: the
    // element sits at 1 and the gain node carries the whole level, so the two never
    // multiply together.
    const gain = level > 1 && allowGraph ? this.#ensureGain() : this.#gain;
    element.muted = this.muted;
    if (gain) {
      element.volume = 1;
      gain.gain.value = level;
      void this.#audioContext?.resume().catch(() => undefined);
      return;
    }
    element.volume = Math.min(level, 1);
  }

  /** Routes the element through a gain node, or returns null where Web Audio is absent. */
  #ensureGain(): GainNode | null {
    if (this.#gain) return this.#gain;
    const element = this.#element;
    if (!element || typeof AudioContext !== "function") return null;
    try {
      const context = new AudioContext();
      const gain = context.createGain();
      context.createMediaElementSource(element).connect(gain);
      gain.connect(context.destination);
      this.#audioContext = context;
      this.#gain = gain;
      return gain;
    } catch {
      // Without the graph the element still plays; it just cannot go above 100%.
      return null;
    }
  }

  async #flush() {
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

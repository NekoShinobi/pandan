<script lang="ts">
  import Bookmark from "lucide-svelte/icons/bookmark";
  import Check from "lucide-svelte/icons/check";
  import CircleAlert from "lucide-svelte/icons/circle-alert";
  import Download from "lucide-svelte/icons/download";
  import HardDrive from "lucide-svelte/icons/hard-drive";
  import HardDriveDownload from "lucide-svelte/icons/hard-drive-download";
  import Inbox from "lucide-svelte/icons/inbox";
  import ListPlus from "lucide-svelte/icons/list-plus";
  import ListX from "lucide-svelte/icons/list-x";
  import Pause from "lucide-svelte/icons/pause";
  import Play from "lucide-svelte/icons/play";
  import Plus from "lucide-svelte/icons/plus";
  import Podcast from "lucide-svelte/icons/podcast";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import {
    ApiError,
    addPodcast,
    appendToPodcastQueue,
    approvePodcastRequest,
    deletePodcast,
    downloadAllPodcastEpisodes,
    fetchPodcastEpisodes,
    fetchPodcastRequests,
    fetchPodcastSettings,
    fetchPodcasts,
    podcastArtworkUrl,
    rejectPodcastRequest,
    removeFromPodcastQueue,
    removePodcastDownload,
    requestPodcastDownload,
    setPodcastEpisodeSaved,
    submitPodcastRequest,
    subscribeToPodcast,
    unsubscribeFromPodcast,
    updatePodcastSettings,
    withdrawPodcastRequest,
    type PodcastAdminSettings,
    type PodcastEpisode,
    type PodcastOverview,
    type PodcastRequest,
    type PodcastSummary,
  } from "$lib/api";
  import { formatPlaybackTime, podcastPlayer } from "$lib/podcastPlayer.svelte";

  let { viewerRole }: { viewerRole: string } = $props();

  type PodcastView = "listen" | "library" | "saved" | "requests";

  const emptyOverview: PodcastOverview = {
    podcasts: [],
    queue: [],
    saved: [],
    recent: [],
    in_progress: [],
    requests: [],
    policy: {
      requests_enabled: true,
      member_downloads_enabled: true,
      max_pending_requests_per_user: 5,
    },
  };

  let overview = $state.raw<PodcastOverview>(emptyOverview);
  let loading = $state(true);
  let pageError = $state("");
  let activeView = $state<PodcastView>("listen");
  let reloadToken = $state(0);
  /** Newest reload whose response has been applied. Not reactive on purpose. */
  let loadedToken = -1;
  let busyEpisode = $state("");

  let feedUrl = $state("");
  let requestNote = $state("");
  let submitting = $state(false);
  let requestFeedback = $state("");
  let requestError = $state("");

  let openPodcast = $state.raw<PodcastSummary | null>(null);
  let openEpisodes = $state.raw<PodcastEpisode[]>([]);
  let episodesLoading = $state(false);
  let showDialog = $state<HTMLDialogElement>();
  let downloadingShow = $state(false);
  let showFeedback = $state("");

  let adminSettings = $state.raw<PodcastAdminSettings | null>(null);
  /** Instance-wide pending queue. `overview.requests` is scoped to the caller. */
  let reviewQueue = $state.raw<PodcastRequest[]>([]);
  let savingSettings = $state(false);
  let adminError = $state("");
  let decisionNote = $state("");
  let decidingId = $state("");

  const isAdministrator = $derived(viewerRole === "administrator");
  const subscribed = $derived(
    overview.podcasts.filter((podcast) => podcast.subscribed),
  );
  const catalogue = $derived(
    overview.podcasts.filter((podcast) => !podcast.subscribed),
  );
  const ownPendingRequests = $derived(
    overview.requests.filter((request) => request.status === "pending"),
  );
  const canDownload = $derived(
    isAdministrator || overview.policy.member_downloads_enabled,
  );
  /**
   * Anything still moving is worth polling for; a settled page is not.
   *
   * The open show's episodes count too: a bulk download is watched from inside that
   * dialog, and its transfers may not appear anywhere on the page behind it.
   */
  const hasActiveTransfers = $derived(
    [
      ...overview.recent,
      ...overview.queue,
      ...overview.saved,
      ...openEpisodes,
    ].some(
      (episode) =>
        episode.download_status === "queued" ||
        episode.download_status === "downloading",
    ),
  );

  // `onMount` never fires for a component mounted after the initial page render, and this
  // page lives inside the shell's `{#if activeSection}` chain.
  $effect(() => {
    // Reading the token here is what re-runs this effect; carrying it into the
    // response also discards a reply the polling timer has already superseded.
    const token = reloadToken;
    let cancelled = false;
    (async () => {
      try {
        const data = await fetchPodcasts();
        if (cancelled || token < loadedToken) return;
        loadedToken = token;
        overview = data;
        pageError = "";
      } catch (error) {
        if (cancelled) return;
        pageError =
          error instanceof ApiError ? error.message : "Podcasts could not load.";
      } finally {
        if (!cancelled) loading = false;
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  // Downloads land in the background, so the page follows them until they settle.
  $effect(() => {
    if (!hasActiveTransfers) return;
    const timer = setInterval(() => {
      reloadToken += 1;
    }, 5000);
    return () => clearInterval(timer);
  });

  // The open show's episodes follow the same reload token as the page behind it, so a
  // bulk download queued from the dialog reports its progress in place.
  $effect(() => {
    const token = reloadToken;
    const podcast = openPodcast;
    // Episode reads resolve through an active subscription, so an unsubscribed show has
    // nothing to poll for.
    if (!podcast?.subscribed) return;
    void token;
    let cancelled = false;
    (async () => {
      try {
        const episodes = await fetchPodcastEpisodes(podcast.id, { limit: 100 });
        if (!cancelled) openEpisodes = episodes;
      } catch (error) {
        if (!cancelled) {
          pageError = describeError(error, "Episodes could not load.");
        }
      } finally {
        if (!cancelled) episodesLoading = false;
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    if (!isAdministrator || activeView !== "requests") return;
    let cancelled = false;
    (async () => {
      try {
        const settings = await fetchPodcastSettings();
        if (!cancelled) adminSettings = settings;
      } catch (error) {
        if (!cancelled) {
          adminError =
            error instanceof ApiError
              ? error.message
              : "Podcast settings could not load.";
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  // The overview only carries the caller's own requests, so the review queue is
  // its own read. Following the reload token refreshes it after every decision.
  $effect(() => {
    const token = reloadToken;
    if (!isAdministrator || activeView !== "requests") return;
    void token;
    let cancelled = false;
    (async () => {
      try {
        const queue = await fetchPodcastRequests("pending");
        if (!cancelled) reviewQueue = queue;
      } catch (error) {
        if (!cancelled) {
          adminError =
            error instanceof ApiError
              ? error.message
              : "The review queue could not load.";
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  function reload() {
    reloadToken += 1;
  }

  function describeError(error: unknown, fallback: string): string {
    return error instanceof ApiError ? error.message : fallback;
  }

  function formatDate(value: string): string {
    const parsed = new Date(value);
    return Number.isNaN(parsed.getTime())
      ? ""
      : parsed.toLocaleDateString(undefined, {
          year: "numeric",
          month: "short",
          day: "numeric",
        });
  }

  function formatBytes(bytes: number): string {
    if (bytes <= 0) return "0 MB";
    const gigabytes = bytes / 1024 ** 3;
    if (gigabytes >= 1) return `${gigabytes.toFixed(1)} GB`;
    return `${Math.round(bytes / 1024 ** 2)} MB`;
  }

  /**
   * How much of an episode has been listened to, as a fraction.
   *
   * The episode the player currently holds is read from live playback state rather than
   * from the stored position: the server copy only moves when a write lands and the page
   * reloads, so a scrub would otherwise leave the listened line behind until then.
   */
  function progressRatio(episode: PodcastEpisode): number {
    const isCurrent = podcastPlayer.episode?.id === episode.id;
    const total = isCurrent && podcastPlayer.duration > 0
      ? podcastPlayer.duration
      : (episode.duration_seconds ?? 0);
    const position = isCurrent
      ? podcastPlayer.currentTime
      : episode.position_seconds;
    if (total <= 0 || position <= 0) return 0;
    return Math.min(1, position / total);
  }

  async function togglePlay(
    episode: PodcastEpisode,
    upNext: PodcastEpisode[],
    played: PodcastEpisode[],
  ) {
    // `isLoaded` is the guard that matters after a download: the player can still be
    // holding this episode from a play that failed before the file was cached, and that
    // source has to be reloaded rather than resumed.
    if (podcastPlayer.episode?.id === episode.id && podcastPlayer.isLoaded) {
      await podcastPlayer.toggle();
      return;
    }
    await podcastPlayer.play(episode, upNext, played);
  }

  async function startDownload(episode: PodcastEpisode) {
    busyEpisode = episode.id;
    try {
      await requestPodcastDownload(episode.id);
      reload();
    } catch (error) {
      pageError = describeError(error, "The download could not be queued.");
    } finally {
      busyEpisode = "";
    }
  }

  async function toggleSaved(episode: PodcastEpisode) {
    busyEpisode = episode.id;
    try {
      await setPodcastEpisodeSaved(episode.id, episode.saved_at === null);
      reload();
    } catch (error) {
      pageError = describeError(error, "The episode could not be saved.");
    } finally {
      busyEpisode = "";
    }
  }

  async function toggleQueued(episode: PodcastEpisode) {
    busyEpisode = episode.id;
    try {
      if (episode.queue_position === null) {
        await appendToPodcastQueue(episode.id);
      } else {
        await removeFromPodcastQueue(episode.id);
      }
      reload();
    } catch (error) {
      pageError = describeError(error, "The play queue could not be updated.");
    } finally {
      busyEpisode = "";
    }
  }

  async function submitRequest(event: SubmitEvent) {
    event.preventDefault();
    if (submitting) return;
    submitting = true;
    requestError = "";
    requestFeedback = "";
    try {
      const outcome = await submitPodcastRequest(feedUrl.trim(), requestNote.trim());
      feedUrl = "";
      requestNote = "";
      requestFeedback =
        outcome.outcome === "subscribed"
          ? "That show is already on this instance, so you have been subscribed to it."
          : "Your request has been sent for review.";
      reload();
    } catch (error) {
      requestError = describeError(error, "The request could not be sent.");
    } finally {
      submitting = false;
    }
  }

  async function toggleSubscription(podcast: PodcastSummary) {
    try {
      if (podcast.subscribed) {
        await unsubscribeFromPodcast(podcast.id);
      } else {
        await subscribeToPodcast(podcast.id);
      }
      reload();
    } catch (error) {
      pageError = describeError(error, "The subscription could not change.");
    }
  }

  function openShow(podcast: PodcastSummary) {
    openPodcast = podcast;
    openEpisodes = [];
    showFeedback = "";
    episodesLoading = podcast.subscribed;
    showDialog?.showModal();
  }

  function closeShow() {
    showDialog?.close();
    openPodcast = null;
    openEpisodes = [];
    showFeedback = "";
  }

  /**
   * Queues the show's whole back catalogue.
   *
   * Administrator-only, and confirmed first: it commits shared disk for every episode the
   * instance has not cached yet. The server decides what is actually outstanding, so this
   * is safe to press twice.
   */
  async function downloadEntireShow() {
    const podcast = openPodcast;
    if (!podcast || downloadingShow) return;
    if (
      !window.confirm(
        `Download every uncached episode of ${podcast.title} to this server?`,
      )
    ) {
      return;
    }
    downloadingShow = true;
    showFeedback = "";
    try {
      const { queued } = await downloadAllPodcastEpisodes(podcast.id);
      showFeedback =
        queued === 0
          ? "Every episode of this show is already cached or in progress."
          : `${queued} ${queued === 1 ? "episode" : "episodes"} queued for download.`;
      reload();
    } catch (error) {
      pageError = describeError(
        error,
        "The show could not be queued for download.",
      );
    } finally {
      downloadingShow = false;
    }
  }

  async function withdraw(requestId: string) {
    try {
      await withdrawPodcastRequest(requestId);
      reload();
    } catch (error) {
      pageError = describeError(error, "The request could not be withdrawn.");
    }
  }

  async function decide(requestId: string, approved: boolean) {
    decidingId = requestId;
    adminError = "";
    try {
      if (approved) {
        await approvePodcastRequest(requestId, decisionNote.trim());
      } else {
        await rejectPodcastRequest(requestId, decisionNote.trim());
      }
      decisionNote = "";
      reload();
    } catch (error) {
      adminError = describeError(error, "The decision could not be recorded.");
    } finally {
      decidingId = "";
    }
  }

  async function addDirectly(event: SubmitEvent) {
    event.preventDefault();
    if (submitting) return;
    submitting = true;
    adminError = "";
    try {
      await addPodcast(feedUrl.trim());
      feedUrl = "";
      requestFeedback = "The show was added to the catalogue.";
      reload();
    } catch (error) {
      adminError = describeError(error, "The show could not be added.");
    } finally {
      submitting = false;
    }
  }

  async function removeShow(podcast: PodcastSummary) {
    try {
      await deletePodcast(podcast.id);
      reload();
    } catch (error) {
      pageError = describeError(error, "The show could not be removed.");
    }
  }

  async function evict(episode: PodcastEpisode) {
    try {
      await removePodcastDownload(episode.id);
      reload();
    } catch (error) {
      pageError = describeError(error, "The file could not be removed.");
    }
  }

  async function saveSettings(event: SubmitEvent) {
    event.preventDefault();
    const settings = adminSettings;
    if (!settings || savingSettings) return;
    savingSettings = true;
    adminError = "";
    try {
      adminSettings = await updatePodcastSettings({
        requests_enabled: settings.requests_enabled,
        member_downloads_enabled: settings.member_downloads_enabled,
        max_pending_requests_per_user: settings.max_pending_requests_per_user,
        storage_budget_bytes: settings.storage_budget_bytes,
        max_episode_bytes: settings.max_episode_bytes,
        default_auto_download_count: settings.default_auto_download_count,
      });
      reload();
    } catch (error) {
      adminError = describeError(error, "The settings could not be saved.");
    } finally {
      savingSettings = false;
    }
  }

  function toggleAdminSwitch(key: "requests_enabled" | "member_downloads_enabled") {
    const settings = adminSettings;
    if (!settings) return;
    adminSettings = { ...settings, [key]: !settings[key] };
  }
</script>

{#snippet artwork(podcastId: string, hasArtwork: boolean)}
  {#if hasArtwork}
    <img
      class="podcast-artwork"
      src={podcastArtworkUrl(podcastId)}
      alt=""
      loading="lazy"
    />
  {:else}
    <span class="podcast-artwork podcast-artwork--empty" aria-hidden="true">
      <Podcast size={18} strokeWidth={1.6} />
    </span>
  {/if}
{/snippet}

{#snippet episodeRow(
  episode: PodcastEpisode,
  upNext: PodcastEpisode[],
  played: PodcastEpisode[],
)}
  {@const isCurrent = podcastPlayer.episode?.id === episode.id}
  {@const ratio = progressRatio(episode)}
  {@const queued = episode.queue_position !== null}
  <li class="podcast-episode" class:podcast-episode--current={isCurrent}>
    <div class="podcast-episode-main">
      <div class="podcast-episode-heading">
        <h4>{episode.title}</h4>
        <p>
          <span>{episode.podcast_title}</span>
          <span aria-hidden="true">·</span>
          <span>{formatDate(episode.published_at)}</span>
          {#if episode.duration_seconds}
            <span aria-hidden="true">·</span>
            <span>{formatPlaybackTime(episode.duration_seconds)}</span>
          {/if}
          {#if episode.completed_at}
            <span class="podcast-chip podcast-chip--done">
              <Check size={11} strokeWidth={2.2} aria-hidden="true" /> Played
            </span>
          {/if}
        </p>
      </div>

      {#if episode.download_status === "downloading"}
        <div
          class="podcast-progress podcast-progress--transfer"
          role="progressbar"
          aria-label="Download progress"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(episode.download_progress * 100)}
        >
          <span style:width={`${Math.round(episode.download_progress * 100)}%`}
          ></span>
        </div>
      {:else if (ratio > 0 || isCurrent) && !episode.completed_at}
        <div
          class={["podcast-progress", isCurrent && "podcast-progress--live"]}
          aria-hidden="true"
        >
          <span style:width={`${Math.min(100, ratio * 100).toFixed(2)}%`}></span>
        </div>
      {/if}

      {#if episode.download_status === "failed"}
        <p class="podcast-episode-error">
          <CircleAlert size={12} strokeWidth={1.9} aria-hidden="true" />
          This episode could not be cached. Try again.
        </p>
      {/if}
    </div>

    <div class="podcast-episode-actions">
      {#if episode.download_status === "ready"}
        <button
          class="ui-button ui-button--primary"
          type="button"
          onclick={() => togglePlay(episode, upNext, played)}
        >
          {#if isCurrent && podcastPlayer.playing}
            <Pause size={14} strokeWidth={2} aria-hidden="true" /> Pause
          {:else}
            <Play size={14} strokeWidth={2} aria-hidden="true" /> Play
          {/if}
        </button>
      {:else if episode.download_status === "downloading"}
        <span class="podcast-status">
          {Math.round(episode.download_progress * 100)}%
        </span>
      {:else if episode.download_status === "queued"}
        <span class="podcast-status">Queued</span>
      {:else}
        <button
          class="ui-button ui-button--secondary"
          type="button"
          disabled={busyEpisode === episode.id || !canDownload}
          title={canDownload
            ? undefined
            : "An administrator has turned off member downloads."}
          onclick={() => startDownload(episode)}
        >
          <Download size={14} strokeWidth={1.9} aria-hidden="true" />
          {episode.download_status === "failed" ? "Retry" : "Download"}
        </button>
      {/if}

      <button
        class="ui-button ui-button--ghost ui-button--icon podcast-toggle"
        type="button"
        aria-pressed={episode.saved_at !== null}
        aria-label={episode.saved_at === null
          ? `Save ${episode.title}`
          : `Unsave ${episode.title}`}
        title={episode.saved_at === null ? "Save episode" : "Remove from saved"}
        disabled={busyEpisode === episode.id}
        onclick={() => toggleSaved(episode)}
      >
        <Bookmark
          size={15}
          strokeWidth={1.9}
          fill={episode.saved_at === null ? "none" : "currentColor"}
        />
      </button>
      <!--
        Queued is a state the listener has to be able to read at a glance and undo without
        guessing, so the control swaps its icon and its label rather than relying on
        `aria-pressed` and a tint alone.
      -->
      <button
        class="ui-button ui-button--ghost ui-button--icon podcast-toggle"
        type="button"
        aria-pressed={queued}
        aria-label={queued
          ? `Take ${episode.title} off the play queue`
          : `Add ${episode.title} to the play queue`}
        title={queued ? "Remove from queue" : "Add to queue"}
        disabled={busyEpisode === episode.id}
        onclick={() => toggleQueued(episode)}
      >
        {#if queued}
          <ListX size={15} strokeWidth={1.9} />
        {:else}
          <ListPlus size={15} strokeWidth={1.9} />
        {/if}
      </button>
      {#if isAdministrator && episode.download_status === "ready"}
        <button
          class="ui-button ui-button--danger ui-button--icon"
          type="button"
          aria-label={`Remove the cached file for ${episode.title}`}
          onclick={() => evict(episode)}
        >
          <Trash2 size={15} strokeWidth={1.9} />
        </button>
      {/if}
    </div>
  </li>
{/snippet}

{#snippet episodeList(episodes: PodcastEpisode[], emptyCopy: string)}
  {#if episodes.length === 0}
    <p class="podcast-empty">{emptyCopy}</p>
  {:else}
    <ul class="podcast-episode-list">
      {#each episodes as episode, index (episode.id)}
        {@render episodeRow(
          episode,
          episodes.slice(index + 1),
          episodes.slice(0, index),
        )}
      {/each}
    </ul>
  {/if}
{/snippet}

<section
  class="podcasts-page product-page"
  class:has-player={podcastPlayer.episode !== null}
  data-od-id="podcasts-page"
>
  <header class="podcasts-header page-header" data-od-id="podcasts-header">
    <div>
      <h2 data-od-id="podcasts-heading">$ podcasts --{activeView}</h2>
      <p>
        Shows are approved for the whole instance and downloaded once, then played
        from this server.
      </p>
    </div>
    <div class="header-actions">
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={reload}
        disabled={loading}
        data-od-id="refresh-podcasts"
      >
        <RefreshCw size={15} strokeWidth={1.8} aria-hidden="true" />
        Refresh
      </button>
    </div>
  </header>

  {#if pageError}
    <p class="podcast-page-error" role="alert">{pageError}</p>
  {/if}

  <nav class="podcast-view-tabs" aria-label="Podcast views">
    {#each [["listen", "Listen"], ["library", "Library"], ["saved", "Saved"], ["requests", "Requests"]] as [view, label] (view)}
      <button
        class={activeView === view ? "active" : undefined}
        type="button"
        aria-pressed={activeView === view}
        onclick={() => (activeView = view as PodcastView)}
        data-od-id={`podcasts-${view}-view`}
      >
        {label}
        {#if view === "listen"}<span>{overview.queue.length}</span>{/if}
        {#if view === "saved"}<span>{overview.saved.length}</span>{/if}
        {#if view === "requests"}
          <span>
            {isAdministrator ? reviewQueue.length : ownPendingRequests.length}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

  {#if loading}
    <p class="podcast-empty">Loading podcasts…</p>
  {:else if activeView === "listen"}
    <div class="podcast-sections">
      <section aria-labelledby="podcast-up-next">
        <span>[ QUEUE ]</span>
        <h3 id="podcast-up-next">Up next</h3>
        {@render episodeList(
          overview.queue,
          "Nothing queued. Add an episode from any show to line it up.",
        )}
      </section>
      {#if overview.in_progress.length > 0}
        <section aria-labelledby="podcast-continue">
          <span>[ IN.PROGRESS ]</span>
          <h3 id="podcast-continue">Continue listening</h3>
          {@render episodeList(overview.in_progress, "")}
        </section>
      {/if}
      <section aria-labelledby="podcast-latest">
        <span>[ LATEST ]</span>
        <h3 id="podcast-latest">Latest</h3>
        {@render episodeList(
          overview.recent,
          "Subscribe to a show in Library to see new episodes here.",
        )}
      </section>
    </div>
  {:else if activeView === "saved"}
    <div class="podcast-sections">
      <section aria-labelledby="podcast-saved" data-od-id="podcast-saved-episodes">
        <span>[ SAVED ]</span>
        <h3 id="podcast-saved">Saved episodes</h3>
        {@render episodeList(
          overview.saved,
          "No saved episodes yet. Use the bookmark control on any episode.",
        )}
      </section>
    </div>
  {:else if activeView === "library"}
    <div class="podcast-sections">
      <section aria-labelledby="podcast-subscribed">
        <span>[ SUBSCRIBED ]</span>
        <h3 id="podcast-subscribed">Your shows</h3>
        {#if subscribed.length === 0}
          <p class="podcast-empty">
            You are not subscribed to anything yet. Everything approved for this
            instance is listed below.
          </p>
        {:else}
          <ul class="podcast-grid">
            {#each subscribed as podcast (podcast.id)}
              <li class="podcast-card">
                <button
                  class="podcast-card-open"
                  type="button"
                  onclick={() => openShow(podcast)}
                >
                  {@render artwork(podcast.id, podcast.has_artwork)}
                  <span class="podcast-card-text">
                    <strong>{podcast.title}</strong>
                    <small>{podcast.author || "Unknown author"}</small>
                    <small>
                      {podcast.episode_count} episodes ·
                      {podcast.downloaded_count} downloaded
                    </small>
                  </span>
                </button>
                <div class="podcast-card-actions">
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    onclick={() => toggleSubscription(podcast)}
                  >
                    Unsubscribe
                  </button>
                  {#if isAdministrator}
                    <button
                      class="ui-button ui-button--danger ui-button--icon"
                      type="button"
                      aria-label={`Remove ${podcast.title} from the instance`}
                      onclick={() => removeShow(podcast)}
                    >
                      <Trash2 size={15} strokeWidth={1.9} />
                    </button>
                  {/if}
                </div>
                {#if podcast.last_error}
                  <p class="podcast-episode-error">
                    <CircleAlert size={12} strokeWidth={1.9} aria-hidden="true" />
                    Last refresh failed.
                  </p>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <section aria-labelledby="podcast-catalogue">
        <span>[ CATALOGUE ]</span>
        <h3 id="podcast-catalogue">Available on this instance</h3>
        {#if catalogue.length === 0}
          <p class="podcast-empty">
            Every approved show is already in your library.
          </p>
        {:else}
          <ul class="podcast-grid">
            {#each catalogue as podcast (podcast.id)}
              <li class="podcast-card">
                <button
                  class="podcast-card-open"
                  type="button"
                  onclick={() => openShow(podcast)}
                >
                  {@render artwork(podcast.id, podcast.has_artwork)}
                  <span class="podcast-card-text">
                    <strong>{podcast.title}</strong>
                    <small>{podcast.author || "Unknown author"}</small>
                    <small>{podcast.episode_count} episodes</small>
                  </span>
                </button>
                <div class="podcast-card-actions">
                  <button
                    class="ui-button ui-button--primary"
                    type="button"
                    onclick={() => toggleSubscription(podcast)}
                  >
                    <Plus size={14} strokeWidth={2} aria-hidden="true" /> Subscribe
                  </button>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      <section class="podcast-request-panel" aria-labelledby="podcast-ask">
        <span>{isAdministrator ? "[ CATALOGUE.ADD ]" : "[ SHOW.REQUEST ]"}</span>
        <h3 id="podcast-ask">
          {isAdministrator ? "Add a show" : "Ask for a show"}
        </h3>
        <p>
          {#if isAdministrator}
            Adding a show publishes it for everyone on this instance and starts
            downloading its newest episodes.
          {:else if overview.policy.requests_enabled}
            Paste a podcast's RSS address. An administrator reviews every request
            before anything is downloaded to this server.
          {:else}
            Requests are closed. Ask an administrator to add a show for you.
          {/if}
        </p>
        <form
          class="podcast-request-form"
          onsubmit={isAdministrator ? addDirectly : submitRequest}
        >
          <label>
            Feed address
            <input
              type="url"
              required
              maxlength="2048"
              placeholder="https://example.com/feed.xml"
              bind:value={feedUrl}
              disabled={!isAdministrator && !overview.policy.requests_enabled}
            />
          </label>
          {#if !isAdministrator}
            <label>
              Why this show <small>optional</small>
              <textarea
                rows="2"
                maxlength="500"
                bind:value={requestNote}
                disabled={!overview.policy.requests_enabled}
              ></textarea>
            </label>
          {/if}
          <button
            class="ui-button ui-button--primary"
            type="submit"
            disabled={submitting ||
              (!isAdministrator && !overview.policy.requests_enabled)}
          >
            {#if submitting}
              Sending…
            {:else if isAdministrator}
              Add to catalogue
            {:else}
              Send request
            {/if}
          </button>
        </form>
        {#if requestFeedback}
          <p class="podcast-feedback" role="status">{requestFeedback}</p>
        {/if}
        {#if requestError}
          <p class="podcast-episode-error" role="alert">{requestError}</p>
        {/if}
        {#if adminError}
          <p class="podcast-episode-error" role="alert">{adminError}</p>
        {/if}
      </section>
    </div>
  {:else}
    <div class="podcast-sections">
      <section aria-labelledby="podcast-my-requests">
        <span>[ REQUESTS ]</span>
        <h3 id="podcast-my-requests">Your requests</h3>
        {#if overview.requests.length === 0}
          <p class="podcast-empty">
            You have not asked for any shows yet.
          </p>
        {:else}
          <ul class="podcast-request-list">
            {#each overview.requests as request (request.id)}
              <li class="podcast-request">
                <div>
                  <strong>{request.resolved_title || request.feed_url}</strong>
                  <small>{request.resolved_author}</small>
                  {#if request.decision_note}
                    <small class="podcast-decision">
                      “{request.decision_note}”
                    </small>
                  {/if}
                </div>
                <span class={`podcast-chip podcast-chip--${request.status}`}>
                  {request.status}
                </span>
                {#if request.status === "pending"}
                  <button
                    class="ui-button ui-button--ghost"
                    type="button"
                    onclick={() => withdraw(request.id)}
                  >
                    Withdraw
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </section>

      {#if isAdministrator}
        <section aria-labelledby="podcast-review">
          <span>[ REVIEW.QUEUE ]</span>
          <h3 id="podcast-review">
            <Inbox size={15} strokeWidth={1.8} aria-hidden="true" />
            Review queue
          </h3>
          {#if adminError}
            <p class="podcast-episode-error" role="alert">{adminError}</p>
          {/if}
          {#if reviewQueue.length === 0}
            <p class="podcast-empty">Nothing is waiting for a decision.</p>
          {:else}
            <label class="podcast-decision-note">
              Decision note <small>shown to the requester</small>
              <input maxlength="500" bind:value={decisionNote} />
            </label>
            <ul class="podcast-request-list">
              {#each reviewQueue as request (request.id)}
                <li class="podcast-request">
                  <div>
                    <strong>{request.resolved_title || request.feed_url}</strong>
                    <small>{request.resolved_author}</small>
                    <small>Asked by {request.requester_name}</small>
                    {#if request.note}
                      <small class="podcast-decision">“{request.note}”</small>
                    {/if}
                  </div>
                  <div class="podcast-card-actions">
                    <button
                      class="ui-button ui-button--primary"
                      type="button"
                      disabled={decidingId === request.id}
                      onclick={() => decide(request.id, true)}
                    >
                      Approve
                    </button>
                    <button
                      class="ui-button ui-button--danger"
                      type="button"
                      disabled={decidingId === request.id}
                      onclick={() => decide(request.id, false)}
                    >
                      Reject
                    </button>
                  </div>
                </li>
              {/each}
            </ul>
          {/if}
        </section>

        {#if adminSettings}
          <section class="podcast-admin-panel" aria-labelledby="podcast-policy">
            <span>[ STORAGE.POLICY ]</span>
            <h3 id="podcast-policy">
              <HardDrive size={15} strokeWidth={1.8} aria-hidden="true" />
              Storage and policy
            </h3>
            <p class="podcast-usage">
              {formatBytes(adminSettings.storage_used_bytes)} of
              {formatBytes(adminSettings.storage_budget_bytes)} used
            </p>
            <form class="podcast-settings-form" onsubmit={saveSettings}>
              <button
                class="ui-toggle-button"
                type="button"
                aria-pressed={adminSettings.requests_enabled}
                onclick={() => toggleAdminSwitch("requests_enabled")}
              >
                <span class="ui-toggle-indicator" aria-hidden="true"></span>
                Members may request shows
              </button>
              <button
                class="ui-toggle-button"
                type="button"
                aria-pressed={adminSettings.member_downloads_enabled}
                onclick={() => toggleAdminSwitch("member_downloads_enabled")}
              >
                <span class="ui-toggle-indicator" aria-hidden="true"></span>
                Members may download episodes
              </button>
              <label>
                Open requests per member
                <input
                  type="number"
                  min="0"
                  max="100"
                  bind:value={adminSettings.max_pending_requests_per_user}
                />
              </label>
              <label>
                Storage budget (GB)
                <input
                  type="number"
                  min="1"
                  max="1024"
                  value={Math.round(
                    adminSettings.storage_budget_bytes / 1024 ** 3,
                  )}
                  oninput={(event) => {
                    if (!adminSettings) return;
                    adminSettings = {
                      ...adminSettings,
                      storage_budget_bytes:
                        Number(event.currentTarget.value) * 1024 ** 3,
                    };
                  }}
                />
              </label>
              <label>
                Largest episode (MB)
                <input
                  type="number"
                  min="1"
                  max="5120"
                  value={Math.round(adminSettings.max_episode_bytes / 1024 ** 2)}
                  oninput={(event) => {
                    if (!adminSettings) return;
                    adminSettings = {
                      ...adminSettings,
                      max_episode_bytes:
                        Number(event.currentTarget.value) * 1024 ** 2,
                    };
                  }}
                />
              </label>
              <label>
                Episodes cached automatically per show
                <input
                  type="number"
                  min="0"
                  max="25"
                  bind:value={adminSettings.default_auto_download_count}
                />
              </label>
              <button
                class="ui-button ui-button--primary"
                type="submit"
                disabled={savingSettings}
              >
                {savingSettings ? "Saving…" : "Save policy"}
              </button>
            </form>
          </section>
        {/if}
      {/if}
    </div>
  {/if}
</section>

<dialog
  class="ui-dialog podcast-dialog"
  bind:this={showDialog}
  onclose={closeShow}
  data-od-id="podcast-show-dialog"
>
  {#if openPodcast}
    <header class="podcast-dialog-header">
      <div>
        <span class="podcast-kicker">[ SHOW ]</span>
        <h3>{openPodcast.title}</h3>
        <p>{openPodcast.author}</p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close show"
        onclick={closeShow}
      >
        <X size={18} strokeWidth={1.9} />
      </button>
    </header>
    <div class="podcast-dialog-body">
      {#if !openPodcast.subscribed}
        <p class="podcast-empty">
          Subscribe to this show to download and play its episodes.
        </p>
      {:else if episodesLoading}
        <p class="podcast-empty">Loading episodes…</p>
      {:else}
        {#if isAdministrator}
          <!--
            Caching a whole back catalogue commits shared disk for the whole instance,
            so it stays an administrator control alongside the per-episode download.
          -->
          <div class="podcast-show-bulk">
            <p>
              <span
                >{openPodcast.downloaded_count} of {openPodcast.episode_count} episodes
                cached on this server</span
              >
              {#if showFeedback}
                <span class="podcast-feedback">{showFeedback}</span>
              {/if}
            </p>
            <button
              class="ui-button ui-button--secondary"
              type="button"
              disabled={downloadingShow ||
                openPodcast.downloaded_count >= openPodcast.episode_count}
              onclick={downloadEntireShow}
            >
              <HardDriveDownload
                size={14}
                strokeWidth={1.9}
                aria-hidden="true"
              />
              {downloadingShow ? "Queueing…" : "Download all"}
            </button>
          </div>
        {/if}
        {@render episodeList(openEpisodes, "This show has no episodes yet.")}
      {/if}
    </div>
  {/if}
</dialog>

<style>
  .podcasts-page {
    display: grid;
    gap: 18px;
    min-width: 0;
  }
  .podcasts-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  .podcasts-header h2 {
    margin: 8px 0 0;
    font-family: var(--font-mono);
    font-size: clamp(26px, 3vw, 42px);
    font-weight: 540;
    line-height: 1.05;
    letter-spacing: -0.04em;
  }
  .podcasts-header p {
    max-width: 68ch;
    margin: 8px 0 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.6;
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .podcast-view-tabs {
    display: flex;
    gap: 6px;
    overflow-x: auto;
  }
  .podcast-view-tabs button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 44px;
    padding: 0 13px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--page-surface, var(--surface));
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.02em;
  }
  .podcast-view-tabs button:hover,
  .podcast-view-tabs button.active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .podcast-view-tabs span {
    color: inherit;
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }

  .podcast-sections {
    display: grid;
    gap: 18px;
  }
  .podcast-sections > section {
    min-width: 0;
    border: 1px solid var(--border);
    background: color-mix(
      in oklch,
      var(--page-surface, var(--surface)) 92%,
      transparent
    );
  }
  .podcast-sections > section > span {
    display: block;
    padding: 12px 14px 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }
  .podcast-sections > section > h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 42px;
    margin: 2px 0 0;
    padding: 0 14px 11px;
    border-bottom: 1px solid var(--border);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 15px;
    font-weight: 550;
    letter-spacing: -0.01em;
  }
  .podcast-empty {
    margin: 0;
    padding: 14px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.6;
  }
  .podcasts-page > .podcast-empty,
  .podcast-sections > section > .podcast-empty {
    min-height: 180px;
    display: grid;
    place-items: center;
    align-content: center;
    border: 0;
    text-align: center;
  }
  .podcast-page-error {
    padding: 10px 12px;
    border: 1px solid color-mix(in oklch, var(--danger) 55%, var(--border));
    background: color-mix(in oklch, var(--danger) 14%, transparent);
  }
  .podcast-page-error,
  .podcast-episode-error {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    color: var(--danger);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .podcast-feedback {
    margin: 0;
    color: var(--accent);
    font-size: 12px;
  }

  .podcast-episode-list,
  .podcast-request-list,
  .podcast-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .podcast-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 12px;
  }
  .podcast-sections > section > .podcast-grid {
    padding: 14px;
  }
  .podcast-sections > section > .podcast-episode-list,
  .podcast-sections > section > .podcast-request-list {
    gap: 0;
  }

  .podcast-episode {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
    padding: 12px 14px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 72%, transparent);
    transition: background-color 120ms var(--ease-out);
  }
  .podcast-sections > section > .podcast-episode-list .podcast-episode {
    border-width: 0 0 1px;
    background: transparent;
  }
  .podcast-sections > section > .podcast-episode-list .podcast-episode:last-child {
    border-bottom: 0;
  }
  .podcast-episode:hover,
  .podcast-episode:focus-within {
    background: var(--fg-soft);
  }
  .podcast-episode--current {
    border-color: var(--accent);
  }
  .podcast-episode-main {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 220px;
    flex: 1 1 320px;
  }
  .podcast-episode-heading h4 {
    margin: 0 0 3px;
    font-family: var(--font-display);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .podcast-episode-heading p {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    margin: 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .podcast-episode-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .podcast-status {
    min-width: 66px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }

  /*
   * A toggle that is on has to read as on before it is hovered, so the pressed state
   * carries the accent on the border and the glyph rather than colour alone on hover.
   */
  .podcast-toggle[aria-pressed="true"] {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
  }
  .podcast-toggle[aria-pressed="true"]:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--button-on-accent, var(--bg));
  }

  .podcast-progress {
    height: 3px;
    background: color-mix(in oklch, var(--fg) 12%, transparent);
    overflow: hidden;
  }
  .podcast-progress span {
    display: block;
    height: 100%;
    background: color-mix(in oklch, var(--accent) 55%, transparent);
    /*
     * Playback moves this roughly four times a second while the scrubber moves it in one
     * jump, so a short ease keeps the seek legible without lagging behind the audio.
     */
    transition: width 120ms var(--ease-out);
  }
  /* A bar that is moving right now earns the full accent; a stored position does not. */
  .podcast-progress--transfer span,
  .podcast-progress--live span {
    background: var(--accent);
  }

  @media (prefers-reduced-motion: reduce) {
    .podcast-progress span {
      transition: none;
    }
  }

  .podcast-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 7px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-family: var(--font-mono);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .podcast-chip--done,
  .podcast-chip--approved {
    border-color: var(--accent);
    color: var(--accent);
  }
  .podcast-chip--rejected {
    border-color: var(--danger);
    color: var(--danger);
  }
  .podcast-chip--pending {
    color: var(--muted);
  }

  .podcast-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--border);
    background: var(--bg);
    transition: border-color 120ms var(--ease-out), background-color 120ms var(--ease-out);
  }
  .podcast-card:hover,
  .podcast-card:focus-within {
    border-color: var(--fg);
    background: var(--fg-soft);
  }
  .podcast-card-open {
    display: flex;
    align-items: center;
    gap: 11px;
    min-height: 44px;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .podcast-card-text {
    display: flex;
    flex-direction: column;
    flex: 1;
    gap: 2px;
    min-width: 0;
  }
  .podcast-card-text strong {
    overflow: hidden;
    font-family: var(--font-display);
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.01em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .podcast-card-text small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .podcast-card-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .podcast-artwork {
    flex: 0 0 auto;
    width: 52px;
    height: 52px;
    border: 1px solid var(--border);
    object-fit: cover;
  }
  .podcast-artwork--empty {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--muted);
  }

  .podcast-request {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
    padding: 12px 14px;
    border: 0;
    border-bottom: 1px solid var(--border);
    transition: background-color 120ms var(--ease-out);
  }
  .podcast-request:last-child {
    border-bottom: 0;
  }
  .podcast-request:hover,
  .podcast-request:focus-within {
    background: var(--fg-soft);
  }
  .podcast-request div {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .podcast-request small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .podcast-decision {
    font-style: italic;
  }

  .podcast-request-panel,
  .podcast-admin-panel {
    padding: 0;
  }
  .podcast-request-panel > p:not(.podcast-feedback):not(.podcast-episode-error),
  .podcast-usage {
    margin: 0;
    padding: 14px 14px 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
  }
  .podcast-request-form,
  .podcast-settings-form,
  .podcast-decision-note {
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .podcast-decision-note {
    margin: 14px;
  }
  .podcast-request-form,
  .podcast-settings-form {
    padding: 14px;
  }
  .podcast-settings-form {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }
  .podcast-settings-form > .ui-toggle-button,
  .podcast-settings-form > .ui-button {
    grid-column: 1 / -1;
    width: fit-content;
  }
  .podcast-request-form label,
  .podcast-settings-form label,
  .podcast-decision-note {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-family: var(--font-mono);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--muted);
  }
  .podcast-request-form small,
  .podcast-decision-note small {
    text-transform: none;
    letter-spacing: 0;
    opacity: 0.75;
  }
  .podcast-request-form input,
  .podcast-request-form textarea,
  .podcast-settings-form input,
  .podcast-decision-note input {
    width: 100%;
    min-height: 44px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--bg);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .podcast-request-form textarea {
    min-height: 82px;
    padding-block: 10px;
    resize: vertical;
  }
  .podcast-request-form input:focus-visible,
  .podcast-request-form textarea:focus-visible,
  .podcast-settings-form input:focus-visible,
  .podcast-decision-note input:focus-visible,
  .podcast-card-open:focus-visible,
  .podcast-view-tabs button:focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }
  .podcast-request-form > .ui-button {
    width: fit-content;
  }
  .podcast-request-panel > .podcast-feedback,
  .podcast-request-panel > .podcast-episode-error,
  .podcast-sections > section > .podcast-episode-error {
    margin: 0 14px 14px;
    padding: 0;
  }

  .podcast-dialog {
    width: min(760px, calc(100vw - 24px));
  }
  .podcast-dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 76px;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }
  .podcast-dialog-header h3 {
    margin: 4px 0 2px;
    font-family: var(--font-mono);
    font-size: 18px;
    font-weight: 550;
  }
  .podcast-dialog-header p {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .podcast-kicker {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    color: var(--muted);
  }
  .podcast-dialog-body {
    padding: 18px;
  }

  .podcast-show-bulk {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    flex-wrap: wrap;
    margin-bottom: 14px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    background: color-mix(
      in oklch,
      var(--page-surface, var(--surface)) 72%,
      transparent
    );
  }
  .podcast-show-bulk p {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  @media (max-width: 720px) {
    .podcasts-header {
      align-items: stretch;
      flex-direction: column;
    }
    .header-actions,
    .header-actions .ui-button {
      width: 100%;
    }
    .podcast-settings-form {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 640px) {
    .podcast-episode,
    .podcast-request {
      align-items: flex-start;
    }
    .podcast-grid {
      grid-template-columns: 1fr;
    }
  }
</style>

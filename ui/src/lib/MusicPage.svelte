<script lang="ts">
  import { onDestroy } from "svelte";
  import ArrowLeft from "lucide-svelte/icons/arrow-left";
  import Disc3 from "lucide-svelte/icons/disc-3";
  import Download from "lucide-svelte/icons/download";
  import ListMusic from "lucide-svelte/icons/list-music";
  import ListPlus from "lucide-svelte/icons/list-plus";
  import Music2 from "lucide-svelte/icons/music-2";
  import Play from "lucide-svelte/icons/play";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import {
    fetchJellyfinMusicHome,
    fetchJellyfinMusicItems,
    fetchJellyfinStatus,
    jellyfinMusicDownloadUrl,
    jellyfinMusicImageUrl,
    type JellyfinMusicHome,
    type JellyfinMusicItem,
    type JellyfinMusicKind,
    type JellyfinStatus,
  } from "$lib/api";
  import { motionSurfaceEnter } from "$lib/motion.svelte";
  import MusicQueueDialog from "$lib/MusicQueueDialog.svelte";
  import { formatPlaybackTime, podcastPlayer } from "$lib/podcastPlayer.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import { createViewSwap } from "$lib/viewSwap.svelte";

  let {
    onOpenAccountSettings,
    onOpenAdminSettings,
  }: {
    onOpenAccountSettings: () => void;
    onOpenAdminSettings: () => void;
  } = $props();

  type MusicView = "home" | "collection" | "detail";

  let status = $state<JellyfinStatus | null>(null);
  let home = $state.raw<JellyfinMusicHome | null>(null);
  let view = $state<MusicView>("home");
  let activeLibraryId = $state("");
  let collectionKind = $state<JellyfinMusicKind>("albums");
  let collectionTitle = $state("Albums");
  let collectionItems = $state.raw<JellyfinMusicItem[]>([]);
  let collectionTotal = $state(0);
  let detailItem = $state.raw<JellyfinMusicItem | null>(null);
  let detailTracks = $state.raw<JellyfinMusicItem[]>([]);
  let searchQuery = $state("");
  let loading = $state(true);
  let refreshing = $state(false);
  let loadingCollection = $state(false);
  let queueOpen = $state(false);
  let error = $state("");
  let reloadToken = $state(0);
  let hasLoaded = false;
  let collectionRequest = 0;
  let detailReturnView: Exclude<MusicView, "detail"> = "home";

  const viewSwap = createViewSwap();
  const loadedSurfaceEnter = motionSurfaceEnter({ y: 8, duration: 0.22 });
  const skeletonRows = [...Array(6).keys()];
  const skeletonHomeCards = [...Array(6).keys()];
  const skeletonCollectionCards = [...Array(10).keys()];

  onDestroy(() => viewSwap.cancel());

  const activeLibrary = $derived(
    home?.libraries.find((library) => library.id === activeLibraryId) ?? null,
  );
  const homeRecent = $derived(
    home?.recent.filter((item) => item.library_id === activeLibraryId) ?? [],
  );
  const homeGroups = $derived.by<
    Array<{
      kind: JellyfinMusicKind;
      title: string;
      items: JellyfinMusicItem[];
    }>
  >(() =>
    home
      ? [
          {
            kind: "albums",
            title: "Albums",
            items: home.albums.filter(
              (item) => item.library_id === activeLibraryId,
            ),
          },
          {
            kind: "artists",
            title: "Artists",
            items: home.artists.filter(
              (item) => item.library_id === activeLibraryId,
            ),
          },
          {
            kind: "playlists",
            title: "Playlists",
            items: home.playlists.filter(
              (item) => item.library_id === activeLibraryId,
            ),
          },
        ]
      : [],
  );

  $effect(() => {
    void reloadToken;
    let cancelled = false;
    void (async () => {
      if (hasLoaded) refreshing = true;
      else loading = true;
      error = "";
      try {
        const nextStatus = await fetchJellyfinStatus();
        if (cancelled) return;
        status = nextStatus;
        if (nextStatus.configured && nextStatus.connected) {
          const nextHome = await fetchJellyfinMusicHome();
          if (cancelled) return;
          home = nextHome;
          if (
            !activeLibraryId ||
            !nextHome.libraries.some(
              (library) => library.id === activeLibraryId,
            )
          ) {
            activeLibraryId = nextHome.libraries[0]?.id ?? "";
          }
        } else {
          home = null;
          view = "home";
          viewSwap.cancel();
        }
        error = "";
      } catch (reason: unknown) {
        if (!cancelled) {
          error =
            reason instanceof Error
              ? reason.message
              : "Unable to load Jellyfin music";
        }
      } finally {
        if (!cancelled) {
          hasLoaded = true;
          loading = false;
          refreshing = false;
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  function refresh() {
    reloadToken += 1;
  }

  async function openCollection(
    kind: JellyfinMusicKind,
    title: string,
    query = "",
  ) {
    if (!activeLibraryId) return;
    const request = ++collectionRequest;
    const libraryId = activeLibraryId;
    let nextItems: JellyfinMusicItem[] = [];
    let nextTotal = 0;
    let nextError = "";
    let settled = false;
    const pending = fetchJellyfinMusicItems({
      libraryId,
      kind,
      query: query || undefined,
      limit: 100,
      sort: query ? "name" : kind === "tracks" ? "newest" : "name",
    })
      .then((response) => {
        nextItems = response.items;
        nextTotal = response.total;
      })
      .catch((reason: unknown) => {
        nextError =
          reason instanceof Error
            ? reason.message
            : "Unable to load collection";
      })
      .finally(() => {
        settled = true;
      });

    await viewSwap.run({
      forward: true,
      pending,
      commit: () => {
        if (request !== collectionRequest) return;
        view = "collection";
        collectionKind = kind;
        collectionTitle = title;
        collectionItems = settled ? nextItems : [];
        collectionTotal = settled ? nextTotal : 0;
        detailItem = null;
        loadingCollection = !settled;
        error = settled ? nextError : "";
      },
    });
    await pending;
    if (request !== collectionRequest || view !== "collection") return;
    collectionItems = nextItems;
    collectionTotal = nextTotal;
    loadingCollection = false;
    error = nextError;
  }

  async function openDetail(item: JellyfinMusicItem) {
    const request = ++collectionRequest;
    const returnView = view === "collection" ? "collection" : "home";
    let nextTracks: JellyfinMusicItem[] = [];
    let nextError = "";
    let settled = false;
    const pending = fetchJellyfinMusicItems({
      libraryId: item.library_id,
      kind: "tracks",
      parentId: item.id,
      limit: 200,
      sort: item.kind === "MusicAlbum" ? "track" : "name",
    })
      .then((response) => {
        nextTracks = response.items;
      })
      .catch((reason: unknown) => {
        nextError =
          reason instanceof Error
            ? reason.message
            : "Unable to load this collection";
      })
      .finally(() => {
        settled = true;
      });

    await viewSwap.run({
      forward: true,
      pending,
      commit: () => {
        if (request !== collectionRequest) return;
        view = "detail";
        detailReturnView = returnView;
        detailItem = item;
        detailTracks = settled ? nextTracks : [];
        loadingCollection = !settled;
        error = settled ? nextError : "";
      },
    });
    await pending;
    if (request !== collectionRequest || view !== "detail") return;
    detailTracks = nextTracks;
    loadingCollection = false;
    error = nextError;
  }

  function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    const query = searchQuery.trim();
    if (!query) return;
    void openCollection("tracks", 'Search: "' + query + '"', query);
  }

  function selectLibrary(event: Event) {
    const nextLibraryId = (event.currentTarget as HTMLSelectElement).value;
    if (nextLibraryId === activeLibraryId) return;
    collectionRequest += 1;
    void viewSwap.run({
      forward: false,
      commit: () => {
        activeLibraryId = nextLibraryId;
        view = "home";
        detailItem = null;
        loadingCollection = false;
        error = "";
      },
    });
  }

  function playTrack(items: JellyfinMusicItem[], index: number) {
    const track = items[index];
    if (!track) return;
    void podcastPlayer.playMusic(
      track,
      items.slice(index + 1),
      items.slice(0, index),
    );
  }

  function playAll() {
    if (detailTracks.length > 0) playTrack(detailTracks, 0);
  }

  function artwork(item: JellyfinMusicItem) {
    if (!item.image_item_id) return "";
    return jellyfinMusicImageUrl(
      item.image_item_id,
      item.library_id,
      item.image_tag,
    );
  }

  function cardSubtitle(item: JellyfinMusicItem) {
    return (
      item.artist ??
      item.album ??
      (item.production_year ? String(item.production_year) : item.kind)
    );
  }

  function back() {
    collectionRequest += 1;
    void viewSwap.run({
      forward: false,
      commit: () => {
        if (view === "detail") view = detailReturnView;
        else view = "home";
        loadingCollection = false;
        error = "";
      },
    });
  }
</script>

{#snippet trackSkeleton()}
  <div class="music-track-list music-skeleton-list" aria-hidden="true">
    {#each skeletonRows as index (index)}
      <div class="music-skeleton-track">
        <span class="music-skeleton music-skeleton-square"></span>
        <span class="music-skeleton-copy">
          <span class="music-skeleton music-skeleton-line"></span>
          <span
            class="music-skeleton music-skeleton-line music-skeleton-line--short"
          ></span>
        </span>
        <span class="music-skeleton music-skeleton-time"></span>
        <span class="music-skeleton-actions">
          <span class="music-skeleton music-skeleton-square"></span>
          <span class="music-skeleton music-skeleton-square"></span>
        </span>
      </div>
    {/each}
  </div>
{/snippet}

{#snippet cardSkeleton(collection = true)}
  <div
    class="music-card-grid"
    class:music-card-grid--collection={collection}
    aria-hidden="true"
  >
    {#each collection ? skeletonCollectionCards : skeletonHomeCards as index (index)}
      <div class="music-card music-skeleton-card">
        <span class="music-skeleton music-skeleton-art"></span>
        <span class="music-skeleton music-skeleton-line"></span>
        <span
          class="music-skeleton music-skeleton-line music-skeleton-line--short"
        ></span>
      </div>
    {/each}
  </div>
{/snippet}

{#snippet pageSkeleton()}
  <div
    class="music-page-skeleton"
    role="status"
    aria-label="Loading Jellyfin music"
  >
    <span class="sr-only">Loading Jellyfin music</span>
    <div class="music-skeleton-toolbar" aria-hidden="true">
      <span class="music-skeleton music-skeleton-label"></span>
      <span class="music-skeleton music-skeleton-search"></span>
    </div>
    <div class="music-home" aria-hidden="true">
      <section class="music-section">
        <div class="music-skeleton-heading" aria-hidden="true">
          <span class="music-skeleton music-skeleton-kicker"></span>
          <span class="music-skeleton music-skeleton-title"></span>
        </div>
        {@render trackSkeleton()}
      </section>
      <section class="music-section">
        <div class="music-skeleton-heading" aria-hidden="true">
          <span class="music-skeleton music-skeleton-kicker"></span>
          <span class="music-skeleton music-skeleton-title"></span>
        </div>
        {@render cardSkeleton(false)}
      </section>
    </div>
  </div>
{/snippet}

<section
  class="music-page product-page"
  class:has-player={podcastPlayer.source !== null}
  data-od-id="music-page"
>
  <header class="music-header page-header" data-od-id="music-header">
    <div>
      <TypedHeading text={"$ music --" + view} odId="music-heading" />
      <p>
        Your Jellyfin music libraries, played through Pandan without exposing
        the server token to this browser.
      </p>
    </div>
    <div class="header-actions">
      {#if status?.connected}
        <button
          class="ui-button ui-button--secondary music-queue-trigger"
          type="button"
          aria-haspopup="dialog"
          aria-controls="music-queue-dialog"
          aria-label={`Open music queue, ${podcastPlayer.musicUpNext.length} ${podcastPlayer.musicUpNext.length === 1 ? "track" : "tracks"} waiting`}
          onclick={() => (queueOpen = true)}
          data-od-id="open-music-queue"
        >
          <ListMusic size={16} strokeWidth={1.8} aria-hidden="true" />
          Queue
          <span aria-live="polite" aria-atomic="true"
            >{podcastPlayer.musicUpNext.length}</span
          >
        </button>
      {/if}
      {#if home && home.libraries.length > 1}
        <label class="music-library-select">
          <span class="sr-only">Music library</span>
          <select
            class="select-input"
            value={activeLibraryId}
            onchange={selectLibrary}
            data-od-id="music-library-select"
          >
            {#each home.libraries as library (library.id)}
              <option value={library.id}>{library.name}</option>
            {/each}
          </select>
        </label>
      {/if}
      <button
        class="ui-button ui-button--ghost ui-button--icon music-refresh"
        type="button"
        disabled={loading || refreshing}
        onclick={refresh}
        aria-label={refreshing ? "Refreshing music" : "Refresh music"}
        title="Refresh music"
        data-od-id="refresh-jellyfin-music"
      >
        <RefreshCw
          class={refreshing ? "spinning" : undefined}
          size={17}
          strokeWidth={1.8}
          aria-hidden="true"
        />
      </button>
    </div>
  </header>

  {#if loading}
    {@render pageSkeleton()}
  {:else}
    <div class="music-loaded" {@attach loadedSurfaceEnter}>
      {#if error}
        <p class="podcast-page-error" role="alert">{error}</p>
      {/if}

      {#if !status?.configured}
        <div
          class="music-connection-state"
          data-od-id="music-instance-not-configured"
        >
          <Disc3
            class="music-state-icon"
            size={34}
            strokeWidth={1.5}
            aria-hidden="true"
          />
          <div>
            <h3>Jellyfin is not connected</h3>
            <p>
              An administrator needs to add the instance media server first.
            </p>
          </div>
          <button
            class="ui-button ui-button--secondary"
            type="button"
            onclick={onOpenAdminSettings}
            data-od-id="open-jellyfin-admin-settings"
          >
            Open settings
          </button>
        </div>
      {:else if !status.connected}
        <div
          class="music-connection-state"
          data-od-id="music-account-not-linked"
        >
          <Music2
            class="music-state-icon"
            size={34}
            strokeWidth={1.5}
            aria-hidden="true"
          />
          <div>
            <h3>Link your music account</h3>
            <p>
              Connect your Jellyfin identity with Quick Connect or your Jellyfin
              credentials. Only your music libraries will be available here.
            </p>
          </div>
          <button
            class="ui-button ui-button--primary"
            type="button"
            onclick={onOpenAccountSettings}
            data-od-id="open-jellyfin-account-settings"
          >
            Link Jellyfin
          </button>
        </div>
      {:else if !home}
        <div class="music-connection-state" data-od-id="music-load-failed">
          <Disc3
            class="music-state-icon"
            size={34}
            strokeWidth={1.5}
            aria-hidden="true"
          />
          <div>
            <h3>Music could not load</h3>
            <p>
              Pandan kept your Jellyfin link. Retry the library request without
              reconnecting your account.
            </p>
          </div>
          <button
            class="ui-button ui-button--secondary"
            type="button"
            onclick={refresh}
            data-od-id="retry-jellyfin-music"
          >
            Retry
          </button>
        </div>
      {:else if home.libraries.length === 0}
        <p class="music-empty">
          This Jellyfin account has no visible music libraries. Movie, TV, book,
          music-video, and live-TV libraries are intentionally excluded.
        </p>
      {:else}
        <div class="music-toolbar" data-od-id="music-toolbar">
          {#if view !== "home"}
            <button
              class="ui-button ui-button--ghost"
              type="button"
              onclick={back}
              data-od-id="music-back"
            >
              <ArrowLeft size={16} strokeWidth={1.8} aria-hidden="true" />
              Back
            </button>
          {:else}
            <span class="music-library-label">{activeLibrary?.name}</span>
          {/if}
          <form class="music-search" role="search" onsubmit={submitSearch}>
            <Search
              class="music-search-icon"
              size={16}
              strokeWidth={1.8}
              aria-hidden="true"
            />
            <input
              type="search"
              bind:value={searchQuery}
              placeholder="Search tracks"
              maxlength="160"
              aria-label="Search Jellyfin tracks"
              data-od-id="jellyfin-music-search"
            />
            <button
              class="ui-button ui-button--secondary"
              type="submit"
              disabled={!searchQuery.trim()}
              data-od-id="search-jellyfin-music"
            >
              Search
            </button>
          </form>
        </div>

        <div
          class="music-view view-swap"
          data-view-phase={viewSwap.phase}
          data-view-direction={viewSwap.direction}
          aria-busy={loadingCollection}
          {@attach viewSwap.attach}
        >
          {#if view === "home"}
            <div class="music-home" data-od-id="music-home">
              <section
                class="music-section"
                aria-labelledby="music-recent-title"
              >
                <div class="music-section-heading">
                  <div>
                    <span>[ RECENT ]</span>
                    <h3 id="music-recent-title">Recently added</h3>
                  </div>
                  <button
                    class="ui-button ui-button--ghost"
                    type="button"
                    onclick={() =>
                      void openCollection("tracks", "Recently added")}
                  >
                    See all
                  </button>
                </div>
                {#if homeRecent.length === 0}
                  <p class="music-empty">
                    No audio items found in this library.
                  </p>
                {:else}
                  <div class="music-track-list">
                    {#each homeRecent as track, index (track.id)}
                      {@const queued = podcastPlayer.isMusicQueued(track)}
                      <article
                        class:active={podcastPlayer.track?.id === track.id}
                        data-od-id={"music-recent-" + track.id}
                      >
                        <button
                          class="music-track-play"
                          type="button"
                          aria-label={"Play " + track.name}
                          onclick={() => playTrack(homeRecent, index)}
                        >
                          <Play
                            size={16}
                            fill="currentColor"
                            aria-hidden="true"
                          />
                        </button>
                        <div class="music-track-copy">
                          <strong>{track.name}</strong>
                          <small
                            >{track.artist ??
                              track.album ??
                              "Unknown artist"}</small
                          >
                        </div>
                        <span
                          >{formatPlaybackTime(
                            track.duration_seconds ?? 0,
                          )}</span
                        >
                        <div class="music-track-actions">
                          <!-- eslint-disable svelte/no-navigation-without-resolve -- authenticated API attachment -->
                          <a
                            class="ui-button ui-button--ghost ui-button--icon"
                            href={jellyfinMusicDownloadUrl(
                              track.id,
                              track.library_id,
                            )}
                            download
                            aria-label={`Download ${track.name}`}
                            title="Download track"
                            data-od-id={`music-download-${track.id}`}
                          >
                            <Download
                              size={17}
                              strokeWidth={1.8}
                              aria-hidden="true"
                            />
                          </a>
                          <!-- eslint-enable svelte/no-navigation-without-resolve -->
                          <button
                            class="ui-button ui-button--ghost ui-button--icon"
                            type="button"
                            disabled={queued}
                            aria-label={queued
                              ? track.name + " is already queued"
                              : "Add " + track.name + " to queue"}
                            title={queued ? "Already queued" : "Add to queue"}
                            onclick={() => podcastPlayer.queueMusic(track)}
                          >
                            <ListPlus
                              size={17}
                              strokeWidth={1.8}
                              aria-hidden="true"
                            />
                          </button>
                        </div>
                      </article>
                    {/each}
                  </div>
                {/if}
              </section>

              {#each homeGroups as group (group.kind)}
                <section
                  class="music-section"
                  aria-labelledby={"music-" + group.kind + "-title"}
                >
                  <div class="music-section-heading">
                    <div>
                      <span>[ {group.kind.toUpperCase()} ]</span>
                      <h3 id={"music-" + group.kind + "-title"}>
                        {group.title}
                      </h3>
                    </div>
                    <button
                      class="ui-button ui-button--ghost"
                      type="button"
                      onclick={() =>
                        void openCollection(group.kind, group.title)}
                    >
                      See all
                    </button>
                  </div>
                  <div class="music-card-grid">
                    {#each group.items as item (item.id)}
                      <button
                        class="music-card"
                        type="button"
                        onclick={() => void openDetail(item)}
                        data-od-id={"music-card-" + item.id}
                      >
                        <span class="music-card-art">
                          {#if artwork(item)}
                            <img src={artwork(item)} alt="" loading="lazy" />
                          {:else}
                            <Disc3
                              size={30}
                              strokeWidth={1.4}
                              aria-hidden="true"
                            />
                          {/if}
                        </span>
                        <strong>{item.name}</strong>
                        <small>{cardSubtitle(item)}</small>
                      </button>
                    {/each}
                  </div>
                </section>
              {/each}
            </div>
          {:else if view === "collection"}
            <section class="music-section" data-od-id="music-collection">
              <div class="music-section-heading">
                <div>
                  <span>[ {collectionTotal} ITEMS ]</span>
                  <h3>{collectionTitle}</h3>
                </div>
              </div>
              {#if loadingCollection}
                <span class="sr-only" role="status">Loading collection</span>
                {#if collectionKind === "tracks"}
                  {@render trackSkeleton()}
                {:else}
                  {@render cardSkeleton()}
                {/if}
              {:else if collectionItems.length === 0}
                <p class="music-empty">No matching music items.</p>
              {:else if collectionKind === "tracks"}
                <div class="music-track-list">
                  {#each collectionItems as track, index (track.id)}
                    {@const queued = podcastPlayer.isMusicQueued(track)}
                    <article
                      class:active={podcastPlayer.track?.id === track.id}
                    >
                      <button
                        class="music-track-play"
                        type="button"
                        aria-label={"Play " + track.name}
                        onclick={() => playTrack(collectionItems, index)}
                      >
                        <Play
                          size={16}
                          fill="currentColor"
                          aria-hidden="true"
                        />
                      </button>
                      <div class="music-track-copy">
                        <strong>{track.name}</strong>
                        <small
                          >{track.artist ??
                            track.album ??
                            "Unknown artist"}</small
                        >
                      </div>
                      <span
                        >{formatPlaybackTime(track.duration_seconds ?? 0)}</span
                      >
                      <div class="music-track-actions">
                        <!-- eslint-disable svelte/no-navigation-without-resolve -- authenticated API attachment -->
                        <a
                          class="ui-button ui-button--ghost ui-button--icon"
                          href={jellyfinMusicDownloadUrl(
                            track.id,
                            track.library_id,
                          )}
                          download
                          aria-label={`Download ${track.name}`}
                          title="Download track"
                          data-od-id={`music-download-${track.id}`}
                        >
                          <Download
                            size={17}
                            strokeWidth={1.8}
                            aria-hidden="true"
                          />
                        </a>
                        <!-- eslint-enable svelte/no-navigation-without-resolve -->
                        <button
                          class="ui-button ui-button--ghost ui-button--icon"
                          type="button"
                          disabled={queued}
                          aria-label={queued
                            ? track.name + " is already queued"
                            : "Add " + track.name + " to queue"}
                          title={queued ? "Already queued" : "Add to queue"}
                          onclick={() => podcastPlayer.queueMusic(track)}
                        >
                          <ListPlus
                            size={17}
                            strokeWidth={1.8}
                            aria-hidden="true"
                          />
                        </button>
                      </div>
                    </article>
                  {/each}
                </div>
              {:else}
                <div class="music-card-grid music-card-grid--collection">
                  {#each collectionItems as item (item.id)}
                    <button
                      class="music-card"
                      type="button"
                      onclick={() => void openDetail(item)}
                    >
                      <span class="music-card-art">
                        {#if artwork(item)}
                          <img src={artwork(item)} alt="" loading="lazy" />
                        {:else}
                          <Disc3
                            size={30}
                            strokeWidth={1.4}
                            aria-hidden="true"
                          />
                        {/if}
                      </span>
                      <strong>{item.name}</strong>
                      <small>{cardSubtitle(item)}</small>
                    </button>
                  {/each}
                </div>
              {/if}
            </section>
          {:else if detailItem}
            <section
              class="music-detail"
              data-od-id={"music-detail-" + detailItem.id}
            >
              <div class="music-detail-heading">
                <span class="music-detail-art">
                  {#if artwork(detailItem)}
                    <img src={artwork(detailItem)} alt="" />
                  {:else}
                    <Disc3 size={46} strokeWidth={1.3} aria-hidden="true" />
                  {/if}
                </span>
                <div>
                  <span>[ {detailItem.kind.toUpperCase()} ]</span>
                  <h3>{detailItem.name}</h3>
                  <p>{cardSubtitle(detailItem)}</p>
                  <button
                    class="ui-button ui-button--primary"
                    type="button"
                    disabled={detailTracks.length === 0}
                    onclick={playAll}
                    data-od-id="play-jellyfin-collection"
                  >
                    <Play size={16} fill="currentColor" aria-hidden="true" />
                    Play
                  </button>
                </div>
              </div>
              {#if loadingCollection}
                <span class="sr-only" role="status">Loading tracks</span>
                {@render trackSkeleton()}
              {:else if detailTracks.length === 0}
                <p class="music-empty">
                  This collection has no tracks from the selected music library.
                </p>
              {:else}
                <div class="music-track-list">
                  {#each detailTracks as track, index (track.id)}
                    {@const queued = podcastPlayer.isMusicQueued(track)}
                    <article
                      class:active={podcastPlayer.track?.id === track.id}
                    >
                      <button
                        class="music-track-play"
                        type="button"
                        aria-label={"Play " + track.name}
                        onclick={() => playTrack(detailTracks, index)}
                      >
                        <Play
                          size={16}
                          fill="currentColor"
                          aria-hidden="true"
                        />
                      </button>
                      <div class="music-track-copy">
                        <strong>{track.name}</strong>
                        <small>{track.artist ?? detailItem.name}</small>
                      </div>
                      <span
                        >{formatPlaybackTime(track.duration_seconds ?? 0)}</span
                      >
                      <div class="music-track-actions">
                        <!-- eslint-disable svelte/no-navigation-without-resolve -- authenticated API attachment -->
                        <a
                          class="ui-button ui-button--ghost ui-button--icon"
                          href={jellyfinMusicDownloadUrl(
                            track.id,
                            track.library_id,
                          )}
                          download
                          aria-label={`Download ${track.name}`}
                          title="Download track"
                          data-od-id={`music-download-${track.id}`}
                        >
                          <Download
                            size={17}
                            strokeWidth={1.8}
                            aria-hidden="true"
                          />
                        </a>
                        <!-- eslint-enable svelte/no-navigation-without-resolve -->
                        <button
                          class="ui-button ui-button--ghost ui-button--icon"
                          type="button"
                          disabled={queued}
                          aria-label={queued
                            ? track.name + " is already queued"
                            : "Add " + track.name + " to queue"}
                          title={queued ? "Already queued" : "Add to queue"}
                          onclick={() => podcastPlayer.queueMusic(track)}
                        >
                          <ListPlus
                            size={17}
                            strokeWidth={1.8}
                            aria-hidden="true"
                          />
                        </button>
                      </div>
                    </article>
                  {/each}
                </div>
              {/if}
            </section>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<MusicQueueDialog open={queueOpen} onClose={() => (queueOpen = false)} />

<style>
  .music-page {
    min-width: 0;
    display: grid;
    align-content: start;
    gap: 22px;
    padding-bottom: 44px;
  }

  .music-page.has-player {
    padding-bottom: 128px;
  }

  .music-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }

  .music-header > div:first-child {
    min-width: 0;
  }

  .music-header p {
    max-width: 68ch;
    margin: 8px 0 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.6;
  }

  .music-header .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .music-library-select {
    min-width: min(220px, 100%);
  }

  .music-refresh {
    flex: 0 0 auto;
  }

  .music-queue-trigger > span {
    min-width: 20px;
    display: inline-grid;
    place-items: center;
    padding-inline: 5px;
    border: 1px solid var(--border);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 9px;
    font-variant-numeric: tabular-nums;
  }

  .music-loaded,
  .music-view,
  .music-page-skeleton {
    min-width: 0;
    display: grid;
    gap: 22px;
  }

  .music-skeleton-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 61px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--border);
  }

  .music-skeleton {
    display: block;
    background: linear-gradient(
      90deg,
      color-mix(in oklch, var(--fg) 7%, transparent) 20%,
      color-mix(in oklch, var(--fg) 15%, transparent) 46%,
      color-mix(in oklch, var(--fg) 7%, transparent) 72%
    );
    background-size: 240% 100%;
    animation: music-skeleton-scan 1.35s cubic-bezier(0.2, 0, 0, 1) infinite;
  }

  .music-skeleton-label {
    width: min(180px, 36vw);
    height: 12px;
  }

  .music-skeleton-search {
    width: min(420px, 52vw);
    height: 44px;
  }

  .music-skeleton-heading {
    display: grid;
    gap: 8px;
  }

  .music-skeleton-kicker {
    width: 74px;
    height: 8px;
  }

  .music-skeleton-title {
    width: min(230px, 64vw);
    height: 31px;
  }

  .music-skeleton-track {
    min-width: 0;
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    min-height: 58px;
    border-bottom: 1px solid var(--border);
  }

  .music-skeleton-copy {
    min-width: 0;
    display: grid;
    gap: 7px;
  }

  .music-skeleton-square {
    width: 28px;
    height: 28px;
    justify-self: center;
  }

  .music-skeleton-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .music-skeleton-line {
    width: min(240px, 72%);
    height: 10px;
  }

  .music-skeleton-line--short {
    width: min(150px, 46%);
    height: 8px;
  }

  .music-skeleton-time {
    width: 34px;
    height: 9px;
  }

  .music-skeleton-card {
    pointer-events: none;
  }

  .music-skeleton-art {
    width: 100%;
    aspect-ratio: 1;
    margin-bottom: 5px;
  }

  @keyframes music-skeleton-scan {
    from {
      background-position: 100% 0;
    }
    to {
      background-position: -140% 0;
    }
  }

  .music-empty {
    margin: 0;
    padding: 30px 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.65;
  }

  .music-connection-state {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 22px;
    min-height: 190px;
    padding: clamp(24px, 4vw, 44px);
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 84%, transparent);
  }

  :global(.music-state-icon) {
    color: var(--muted);
  }

  .music-connection-state h3,
  .music-section h3,
  .music-detail h3 {
    margin: 0;
    color: var(--fg);
    font-family: var(--font-display);
    font-weight: 590;
    letter-spacing: -0.02em;
  }

  .music-connection-state h3 {
    font-size: clamp(24px, 3vw, 34px);
    line-height: 1.12;
  }

  .music-connection-state p {
    max-width: 58ch;
    margin: 8px 0 0;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.6;
  }

  .music-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-width: 0;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--border);
  }

  .music-library-label {
    overflow: hidden;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .music-search {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: min(420px, 100%);
    padding-left: 12px;
    border: 1px solid var(--border);
    background: var(--bg);
  }

  :global(.music-search-icon) {
    flex: 0 0 auto;
    color: var(--muted);
  }

  .music-search input {
    min-width: 0;
    flex: 1;
    height: 44px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font: 12px var(--font-mono);
  }

  .music-search:focus-within {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .music-home {
    display: grid;
    gap: clamp(34px, 6vw, 62px);
  }

  .music-section,
  .music-detail {
    min-width: 0;
    display: grid;
    gap: 16px;
    padding: clamp(18px, 2.2vw, 26px);
    border: 1px solid color-mix(in oklch, var(--border) 76%, var(--fg));
    background: color-mix(in oklch, var(--bg) 88%, transparent);
    box-shadow:
      0 0 0 10px color-mix(in oklch, var(--bg) 24%, transparent),
      0 24px 64px color-mix(in oklch, var(--bg) 72%, transparent);
  }

  .music-section-heading {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 16px;
  }

  .music-section-heading > div {
    min-width: 0;
  }

  .music-section-heading span,
  .music-detail-heading > div > span {
    display: block;
    margin-bottom: 6px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
  }

  .music-section h3 {
    font-size: clamp(24px, 3vw, 32px);
    line-height: 1.16;
  }

  .music-card-grid {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 1px;
    border: 1px solid var(--border);
    background: var(--border);
  }

  .music-card-grid--collection {
    grid-template-columns: repeat(5, minmax(0, 1fr));
  }

  .music-card {
    min-width: 0;
    min-height: 214px;
    display: grid;
    align-content: start;
    gap: 6px;
    padding: 12px;
    border: 0;
    background: var(--surface);
    color: var(--fg);
    cursor: pointer;
    text-align: left;
    transition:
      background 120ms var(--ease-out),
      transform 90ms var(--ease-out);
  }

  .music-card:hover {
    background: color-mix(in oklch, var(--surface) 82%, var(--fg));
  }

  .music-card:active {
    transform: translateY(2px);
  }

  .music-card:focus-visible,
  .music-track-play:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -3px;
  }

  .music-card-art,
  .music-detail-art {
    overflow: hidden;
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--muted);
  }

  .music-card-art {
    margin-bottom: 5px;
  }

  .music-card-art img,
  .music-detail-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .music-card strong,
  .music-track-list strong {
    overflow: hidden;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 550;
    line-height: 1.45;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .music-card small,
  .music-track-list small {
    overflow: hidden;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.5;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .music-track-list {
    display: grid;
    border-top: 1px solid var(--border);
  }

  .music-track-list article {
    min-width: 0;
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 12px;
    min-height: 58px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
  }

  .music-track-list article.active {
    background: color-mix(in oklch, var(--accent) 8%, transparent);
    color: var(--fg);
  }

  .music-track-copy {
    min-width: 0;
    display: grid;
  }

  .music-track-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .music-track-list article > span {
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .music-track-play {
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--fg);
    cursor: pointer;
  }

  .music-track-play:hover {
    background: var(--fg-soft);
  }

  .music-detail {
    gap: 28px;
  }

  .music-detail-heading {
    display: grid;
    grid-template-columns: minmax(180px, 270px) minmax(0, 1fr);
    align-items: end;
    gap: clamp(24px, 5vw, 54px);
  }

  .music-detail-heading > div {
    min-width: 0;
    padding-bottom: 8px;
  }

  .music-detail h3 {
    overflow-wrap: anywhere;
    font-size: clamp(34px, 5vw, 58px);
    line-height: 1.08;
  }

  .music-detail p {
    margin: 10px 0 22px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  @media (max-width: 1100px) {
    .music-card-grid,
    .music-card-grid--collection {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }
  }

  @media (max-width: 760px) {
    .music-header {
      align-items: stretch;
      flex-direction: column;
    }

    .music-header .header-actions {
      width: 100%;
    }

    .music-library-select {
      flex: 1 1 220px;
    }

    .music-library-select .select-input {
      width: 100%;
    }

    .music-connection-state {
      grid-template-columns: 1fr;
      align-items: start;
    }

    .music-toolbar {
      align-items: stretch;
      flex-direction: column;
    }

    .music-skeleton-toolbar {
      align-items: stretch;
      flex-direction: column;
    }

    .music-skeleton-label,
    .music-skeleton-search {
      width: 100%;
    }

    .music-search {
      min-width: 0;
      width: 100%;
    }

    .music-card-grid,
    .music-card-grid--collection {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .music-card {
      min-height: 0;
    }

    .music-detail-heading {
      grid-template-columns: minmax(120px, 190px) minmax(0, 1fr);
      align-items: center;
    }
  }

  @media (max-width: 520px) {
    .music-page.has-player {
      padding-bottom: 178px;
    }

    .music-section,
    .music-detail {
      padding: 14px;
      box-shadow:
        0 0 0 6px color-mix(in oklch, var(--bg) 24%, transparent),
        0 18px 44px color-mix(in oklch, var(--bg) 68%, transparent);
    }

    .music-section-heading {
      align-items: start;
    }

    .music-track-list article {
      grid-template-columns: 44px minmax(0, 1fr) auto;
      gap: 8px;
    }

    .music-skeleton-track {
      grid-template-columns: 44px minmax(0, 1fr) auto;
      gap: 8px;
    }

    .music-track-list article > span {
      display: none;
    }

    .music-skeleton-time {
      display: none;
    }

    .music-detail-heading {
      grid-template-columns: 1fr;
    }

    .music-detail-art {
      width: min(220px, 70vw);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .music-card,
    .music-skeleton {
      transition: none;
      animation: none;
    }
  }
</style>

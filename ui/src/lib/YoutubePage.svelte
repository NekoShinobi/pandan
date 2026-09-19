<script lang="ts">
  import { move } from "@dnd-kit/helpers";
  import {
    DragDropProvider,
    KeyboardSensor,
    PointerSensor,
    type DragDropEventHandlers,
  } from "@dnd-kit/svelte";
  import CirclePlay from "lucide-svelte/icons/circle-play";
  import Download from "lucide-svelte/icons/download";
  import Bookmark from "lucide-svelte/icons/bookmark";
  import Check from "lucide-svelte/icons/check";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import Image from "lucide-svelte/icons/image";
  import Ellipsis from "lucide-svelte/icons/ellipsis";
  import List from "lucide-svelte/icons/list";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Settings2 from "lucide-svelte/icons/settings-2";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import YoutubeGroupSortable from "$lib/YoutubeGroupSortable.svelte";
  import {
    createYoutubeGroup,
    createYoutubeSubscription,
    deleteYoutubeGroup,
    deleteYoutubeSubscription,
    fetchYoutubeReader,
    refreshYoutubeSubscription,
    reorderYoutubeGroups,
    setYoutubeWatchLater,
    updateYoutubeDisplayMode,
    updateYoutubeGroup,
    type YoutubeDisplayMode,
    type YoutubeGroup,
    type YoutubeReaderResponse,
    type YoutubeSubscription,
    type YoutubeVideo,
  } from "$lib/api";

  type YoutubeView = "latest" | "watch-later";

  let { ondownload = () => {} }: { ondownload?: (url: string) => void } =
    $props();
  type YoutubeDragHandlers = DragDropEventHandlers;
  type YoutubeDragStartEvent = Parameters<
    NonNullable<YoutubeDragHandlers["onDragStart"]>
  >[0];
  type YoutubeDragOverEvent = Parameters<
    NonNullable<YoutubeDragHandlers["onDragOver"]>
  >[0];
  type YoutubeDragEndEvent = Parameters<
    NonNullable<YoutubeDragHandlers["onDragEnd"]>
  >[0];

  const youtubeGroupSensors = [
    PointerSensor,
    KeyboardSensor.configure({
      keyboardCodes: {
        start: ["Space"],
        cancel: ["Escape"],
        end: ["Space", "Tab"],
        up: ["ArrowUp"],
        down: ["ArrowDown"],
        left: ["ArrowLeft"],
        right: ["ArrowRight"],
      },
    }),
  ];
  const reducedMotion = new MediaQuery("(prefers-reduced-motion: reduce)");

  let reader = $state.raw<YoutubeReaderResponse>({
    subscriptions: [],
    groups: [],
    videos: [],
    watch_later: [],
    display_mode: "thumbnails",
  });
  let loading = $state(true);
  let refreshing = $state(false);
  let hasLoaded = false;
  let pageError = $state("");
  let query = $state("");
  let activeView = $state<YoutubeView>("latest");
  let activeGroupId = $state("all");
  let busyChannelId = $state("");
  let busyVideoId = $state("");
  let pendingChannelDelete = $state("");

  let sourcesDialog = $state<HTMLDialogElement>();
  let sourceQuery = $state("");
  let sourceNeedle = $derived(sourceQuery.trim().toLowerCase());
  let matchingSourceGroups = $derived(
    reader.groups.filter((group) =>
      group.name.toLowerCase().includes(sourceNeedle),
    ),
  );
  let matchingSources = $derived.by(() => {
    if (!sourceNeedle) return reader.subscriptions;
    const categoryChannelIds = new Set(
      matchingSourceGroups.flatMap((group) => group.channel_ids),
    );
    return reader.subscriptions.filter(
      (subscription) =>
        subscription.title.toLowerCase().includes(sourceNeedle) ||
        subscription.channel_id.toLowerCase().includes(sourceNeedle) ||
        categoryChannelIds.has(subscription.channel_id),
    );
  });
  let videoMenuPosition = $state({ top: 0, left: 0 });

  let subscriptionDialog = $state<HTMLDialogElement>();
  let channelInput = $state<HTMLInputElement>();
  let channelId = $state("");
  let subscriptionGroupIds = $state<string[]>([]);
  let subscriptionError = $state("");
  let savingSubscription = $state(false);

  let groupDialog = $state<HTMLDialogElement>();
  let groupNameInput = $state<HTMLInputElement>();
  let editingGroupId = $state<string | null>(null);
  let groupName = $state("");
  let groupChannelIds = $state<string[]>([]);
  let groupError = $state("");
  let savingGroup = $state(false);
  let confirmingGroupDelete = $state(false);
  let groupDragSnapshot: YoutubeGroup[] | null = null;
  let savingGroupOrder = $state(false);

  let activeGroup = $derived(
    reader.groups.find((group) => group.id === activeGroupId) ?? null,
  );
  let activeChannelIds = $derived(
    activeGroup ? new Set(activeGroup.channel_ids) : null,
  );
  let channelThumbnails = $derived.by(() => {
    const thumbnails: Record<string, string> = {};
    for (const subscription of reader.subscriptions) {
      if (subscription.thumbnail_url) {
        thumbnails[subscription.channel_id] = subscription.thumbnail_url;
      }
    }
    return thumbnails;
  });
  let filteredVideos = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    const videos =
      activeView === "watch-later" ? reader.watch_later : reader.videos;
    return videos.filter((video) => {
      if (
        activeView === "latest" &&
        activeChannelIds &&
        !activeChannelIds.has(video.channel_id)
      )
        return false;
      if (!needle) return true;
      return [video.title, video.channel_title].some((value) =>
        value.toLowerCase().includes(needle),
      );
    });
  });

  onMount(() => {
    void loadReader();
  });

  async function loadReader() {
    if (refreshing) return;
    loading = !hasLoaded;
    refreshing = true;
    pageError = "";
    try {
      reader = await fetchYoutubeReader();
      hasLoaded = true;
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to load YouTube subscriptions");
    } finally {
      loading = false;
      refreshing = false;
    }
  }

  function captureSubscriptionDialog(node: HTMLDialogElement) {
    subscriptionDialog = node;
    return () => (subscriptionDialog = undefined);
  }

  function captureChannelInput(node: HTMLInputElement) {
    channelInput = node;
    return () => (channelInput = undefined);
  }

  function captureGroupDialog(node: HTMLDialogElement) {
    groupDialog = node;
    return () => (groupDialog = undefined);
  }

  function captureGroupNameInput(node: HTMLInputElement) {
    groupNameInput = node;
    return () => (groupNameInput = undefined);
  }

  async function openSubscriptionDialog() {
    channelId = "";
    subscriptionGroupIds = [];
    subscriptionError = "";
    subscriptionDialog?.showModal();
    await tick();
    channelInput?.focus();
  }

  async function subscribe(event: SubmitEvent) {
    event.preventDefault();
    if (savingSubscription) return;
    savingSubscription = true;
    subscriptionError = "";
    try {
      reader = await createYoutubeSubscription(
        channelId.trim(),
        subscriptionGroupIds,
      );
      subscriptionDialog?.close();
    } catch (reason: unknown) {
      subscriptionError = message(
        reason,
        "Unable to subscribe to this channel",
      );
    } finally {
      savingSubscription = false;
    }
  }

  function toggleSubscriptionGroup(groupId: string) {
    subscriptionGroupIds = subscriptionGroupIds.includes(groupId)
      ? subscriptionGroupIds.filter((value) => value !== groupId)
      : [...subscriptionGroupIds, groupId];
  }

  function applyGroupOrder(groups: YoutubeGroup[]) {
    reader = {
      ...reader,
      groups: groups.map((group, position) => ({ ...group, position })),
    };
  }

  function startGroupDrag(event: YoutubeDragStartEvent) {
    if (event.operation.source?.type === "youtube-group") {
      groupDragSnapshot = reader.groups.slice();
    }
  }

  function previewGroupOrder(event: YoutubeDragOverEvent) {
    if (event.operation.source?.type !== "youtube-group") return;
    applyGroupOrder(move(reader.groups, event));
  }

  async function finishGroupDrag(event: YoutubeDragEndEvent) {
    if (event.operation.source?.type !== "youtube-group") return;
    if (event.canceled || !event.operation.target) {
      if (groupDragSnapshot) applyGroupOrder(groupDragSnapshot);
      groupDragSnapshot = null;
      return;
    }

    const previous = groupDragSnapshot;
    const groups = move(reader.groups, event);
    applyGroupOrder(groups);
    groupDragSnapshot = null;
    if (
      !previous ||
      previous.every((group, index) => group.id === groups[index]?.id)
    )
      return;

    savingGroupOrder = true;
    pageError = "";
    try {
      reader = await reorderYoutubeGroups(groups.map((group) => group.id));
    } catch (reason: unknown) {
      applyGroupOrder(previous);
      pageError = message(reason, "Unable to save the category order");
    } finally {
      savingGroupOrder = false;
    }
  }

  async function refreshChannel(subscription: YoutubeSubscription) {
    if (busyChannelId) return;
    busyChannelId = subscription.channel_id;
    pageError = "";
    try {
      reader = await refreshYoutubeSubscription(subscription.channel_id);
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to refresh this channel");
      reader = await fetchYoutubeReader().catch(() => reader);
    } finally {
      busyChannelId = "";
    }
  }

  async function removeChannel(subscription: YoutubeSubscription) {
    if (busyChannelId) return;
    if (pendingChannelDelete !== subscription.channel_id) {
      pendingChannelDelete = subscription.channel_id;
      return;
    }
    busyChannelId = subscription.channel_id;
    pageError = "";
    try {
      await deleteYoutubeSubscription(subscription.channel_id);
      reader = await fetchYoutubeReader();
      pendingChannelDelete = "";
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to remove this channel");
    } finally {
      busyChannelId = "";
    }
  }

  async function setDisplayMode(displayMode: YoutubeDisplayMode) {
    if (reader.display_mode === displayMode) return;
    const previous = reader;
    reader = { ...reader, display_mode: displayMode };
    try {
      reader = await updateYoutubeDisplayMode(displayMode);
    } catch (reason: unknown) {
      reader = previous;
      pageError = message(reason, "Unable to save the display mode");
    }
  }

  function selectView(view: YoutubeView) {
    activeView = view;
    if (view === "watch-later") activeGroupId = "all";
  }

  function selectGroup(groupId: string) {
    activeGroupId = groupId;
  }

  function captureSourcesDialog(node: HTMLDialogElement) {
    sourcesDialog = node;
    return () => (sourcesDialog = undefined);
  }

  function openSources() {
    pendingChannelDelete = "";
    sourceQuery = "";
    sourcesDialog?.querySelectorAll("details[open]").forEach((details) => {
      details.removeAttribute("open");
    });
    sourcesDialog?.showModal();
  }

  async function toggleChannelCategory(
    subscription: YoutubeSubscription,
    group: YoutubeGroup,
  ) {
    if (busyChannelId || savingGroupOrder) return;
    busyChannelId = subscription.channel_id;
    pageError = "";
    const channelIds = group.channel_ids.includes(subscription.channel_id)
      ? group.channel_ids.filter((id) => id !== subscription.channel_id)
      : [...group.channel_ids, subscription.channel_id];
    try {
      reader = await updateYoutubeGroup(group.id, group.name, channelIds);
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to change this channel's categories");
    } finally {
      busyChannelId = "";
    }
  }

  function positionVideoMenu(event: MouseEvent) {
    const button = event.currentTarget as HTMLButtonElement;
    const bounds = button.getBoundingClientRect();
    videoMenuPosition = {
      top: Math.max(8, Math.min(bounds.bottom + 4, window.innerHeight - 112)),
      left: Math.max(8, Math.min(bounds.right - 220, window.innerWidth - 228)),
    };
  }

  function closeVideoMenu() {
    document
      .querySelector<HTMLElement>(".youtube-video-menu:popover-open")
      ?.hidePopover();
  }

  async function toggleWatchLater(video: YoutubeVideo) {
    if (busyVideoId) return;
    busyVideoId = video.id;
    pageError = "";
    try {
      reader = await setYoutubeWatchLater(
        video.id,
        video.watch_later_at === null,
      );
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to update Watch Later");
    } finally {
      busyVideoId = "";
    }
  }

  async function openNewGroup() {
    editingGroupId = null;
    groupName = "";
    groupChannelIds = [];
    groupError = "";
    confirmingGroupDelete = false;
    groupDialog?.showModal();
    await tick();
    groupNameInput?.focus();
  }

  async function openEditGroup(group: YoutubeGroup) {
    editingGroupId = group.id;
    groupName = group.name;
    groupChannelIds = [...group.channel_ids];
    groupError = "";
    confirmingGroupDelete = false;
    groupDialog?.showModal();
    await tick();
    groupNameInput?.focus();
  }

  function toggleGroupChannel(channelIdValue: string) {
    groupChannelIds = groupChannelIds.includes(channelIdValue)
      ? groupChannelIds.filter((value) => value !== channelIdValue)
      : [...groupChannelIds, channelIdValue];
  }

  async function saveGroup(event: SubmitEvent) {
    event.preventDefault();
    if (savingGroup) return;
    savingGroup = true;
    groupError = "";
    try {
      let groupId = editingGroupId;
      if (!groupId) {
        const created = await createYoutubeGroup(groupName.trim());
        groupId =
          created.groups.find(
            (group) =>
              group.name.toLowerCase() === groupName.trim().toLowerCase(),
          )?.id ?? null;
        if (!groupId) throw new Error("The new group could not be found");
      }
      reader = await updateYoutubeGroup(
        groupId,
        groupName.trim(),
        groupChannelIds,
      );
      activeGroupId = groupId;
      groupDialog?.close();
    } catch (reason: unknown) {
      groupError = message(reason, "Unable to save this group");
    } finally {
      savingGroup = false;
    }
  }

  async function removeGroup() {
    if (!editingGroupId || savingGroup) return;
    if (!confirmingGroupDelete) {
      confirmingGroupDelete = true;
      return;
    }
    savingGroup = true;
    groupError = "";
    try {
      await deleteYoutubeGroup(editingGroupId);
      if (activeGroupId === editingGroupId) activeGroupId = "all";
      reader = await fetchYoutubeReader();
      groupDialog?.close();
    } catch (reason: unknown) {
      groupError = message(reason, "Unable to remove this group");
    } finally {
      savingGroup = false;
    }
  }

  function message(reason: unknown, fallback: string) {
    return reason instanceof Error ? reason.message : fallback;
  }

  function dateLabel(value: string | null) {
    if (!value) return "Waiting for first fetch";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "Fetch time unavailable";
    return `Checked ${new Intl.RelativeTimeFormat("en", {
      numeric: "auto",
    }).format(Math.round((date.getTime() - Date.now()) / 3_600_000), "hour")}`;
  }

  function publishedLabel(value: string) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "Date unavailable";
    const ageMilliseconds = Date.now() - date.getTime();
    if (ageMilliseconds >= 0 && ageMilliseconds < 24 * 3_600_000) {
      const ageMinutes = Math.floor(ageMilliseconds / 60_000);
      return ageMinutes < 60
        ? `${Math.max(1, ageMinutes)}m`
        : `${Math.floor(ageMinutes / 60)}h`;
    }
    return new Intl.DateTimeFormat("en", {
      month: "short",
      day: "numeric",
      year:
        date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
    }).format(date);
  }

  function channelInitial(value: string) {
    return Array.from(value.trim())[0]?.toUpperCase() ?? "?";
  }

  const viewCountFormatter = new Intl.NumberFormat("en", {
    notation: "compact",
    maximumFractionDigits: 1,
  });

  function durationLabel(seconds: number | null) {
    if (seconds == null || seconds <= 0) return undefined;
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const remainder = String(seconds % 60).padStart(2, "0");
    return `${hours ? `${hours}:${String(minutes).padStart(2, "0")}` : minutes}:${remainder}`;
  }
</script>

<svelte:window onresize={closeVideoMenu} />

<section class="youtube-page product-page" data-od-id="youtube-page">
  <header class="youtube-header page-header" data-od-id="youtube-heading">
    <div>
      <TypedHeading text={`$ youtube --${activeView}`} odId="youtube-heading" />
      <p>
        {activeView === "latest"
          ? `${reader.subscriptions.length} channels · ${reader.videos.length} stored uploads · refreshes every 2 hours`
          : `${reader.watch_later.length} saved ${reader.watch_later.length === 1 ? "video" : "videos"}`}
      </p>
    </div>
    <div class="youtube-header-actions">
      <button
        class="ui-button ui-button--secondary youtube-secondary-button"
        type="button"
        onclick={loadReader}
        disabled={refreshing || !!busyChannelId}
        aria-busy={refreshing}
        title="Load the latest stored uploads"
        data-od-id="youtube-refresh"
      >
        <RefreshCw
          class={refreshing ? "spinning" : ""}
          size={16}
          strokeWidth={1.8}
          aria-hidden="true"
        />
        Refresh
      </button>
      <button
        class="ui-button ui-button--secondary youtube-secondary-button"
        type="button"
        onclick={openSources}
        data-od-id="youtube-edit-sources"
      >
        <Settings2 size={16} strokeWidth={1.8} aria-hidden="true" />
        Edit Sources
      </button>
    </div>
  </header>

  <nav
    class="youtube-view-tabs"
    aria-label="YouTube reader views"
    data-od-id="youtube-reader-views"
  >
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={activeView === "latest"}
      onclick={() => selectView("latest")}
      data-od-id="youtube-latest-view"
    >
      Latest <span>{reader.videos.length}</span>
    </button>
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={activeView === "watch-later"}
      onclick={() => selectView("watch-later")}
      data-od-id="youtube-watch-later-view"
    >
      <Bookmark size={15} strokeWidth={1.8} aria-hidden="true" />
      Watch later <span>{reader.watch_later.length}</span>
    </button>
  </nav>

  <div class="youtube-toolbar" data-od-id="youtube-view-controls">
    {#if activeView === "latest"}
      <nav aria-label="YouTube categories">
        <button
          type="button"
          aria-pressed={activeGroupId === "all"}
          onclick={() => selectGroup("all")}>All channels</button
        >
        {#each reader.groups as group (group.id)}
          <button
            type="button"
            aria-pressed={activeGroupId === group.id}
            onclick={() => selectGroup(group.id)}>{group.name}</button
          >
        {/each}
      </nav>
    {:else}
      <p class="youtube-watch-later-note">
        Saved videos stay here after you unsubscribe.
      </p>
    {/if}
    <div class="youtube-toolbar-right">
      <label class="youtube-search">
        <Search size={15} strokeWidth={1.8} aria-hidden="true" />
        <span class="sr-only">Search videos and channels</span>
        <input type="search" bind:value={query} placeholder="Filter uploads…" />
      </label>
      <div class="display-switch" role="group" aria-label="Video display mode">
        <button
          type="button"
          aria-label="Show thumbnails"
          aria-pressed={reader.display_mode === "thumbnails"}
          onclick={() => setDisplayMode("thumbnails")}
        >
          <Image size={16} strokeWidth={1.8} aria-hidden="true" />
        </button>
        <button
          type="button"
          aria-label="Hide thumbnails"
          aria-pressed={reader.display_mode === "compact"}
          onclick={() => setDisplayMode("compact")}
        >
          <List size={17} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </div>
    </div>
  </div>

  {#if pageError}
    <div class="youtube-message" role="status">
      <span>{pageError}</span><button
        type="button"
        onclick={() => (pageError = "")}>Dismiss</button
      >
    </div>
  {/if}

  <div class="youtube-layout">
    <main
      class={["youtube-feed", reader.display_mode]}
      onscroll={closeVideoMenu}
      aria-label={activeView === "latest"
        ? "YouTube uploads"
        : "YouTube Watch Later"}
      data-od-id={activeView === "latest"
        ? "youtube-video-feed"
        : "youtube-watch-later-feed"}
    >
      {#if loading}
        <div class="youtube-empty" role="status">
          <RefreshCw
            class="spinning"
            size={28}
            strokeWidth={1.5}
            aria-hidden="true"
          /><strong>Loading channels…</strong>
        </div>
      {:else}
        {#each filteredVideos as video (video.id)}
          {@const duration = durationLabel(video.duration_seconds)}
          <article
            class="youtube-video"
            data-od-id={`youtube-video-${video.id}`}
          >
            <div class="youtube-video-media">
              <!-- eslint-disable svelte/no-navigation-without-resolve -- External YouTube destination. -->
              <a
                class="youtube-thumbnail"
                href={video.url}
                target="_blank"
                rel="noreferrer"
                aria-label={`Watch ${video.title}${duration ? `. Duration: ${duration}` : ""}`}
              >
                {#if video.thumbnail_url}<img
                    src={video.thumbnail_url}
                    alt=""
                    loading="lazy"
                  />{:else}<CirclePlay
                    size={28}
                    strokeWidth={1.4}
                    aria-hidden="true"
                  />{/if}
                {#if duration}
                  <span class="youtube-duration" aria-hidden="true"
                    >{duration}</span
                  >
                {/if}
              </a>
              <!-- eslint-enable svelte/no-navigation-without-resolve -->
              <button
                class="youtube-video-menu-trigger"
                type="button"
                popovertarget={`youtube-video-menu-${video.id}`}
                aria-label={`Actions for ${video.title}`}
                title="Video actions"
                onclick={positionVideoMenu}
                data-od-id={`youtube-video-actions-${video.id}`}
                ><Ellipsis
                  size={20}
                  strokeWidth={2}
                  aria-hidden="true"
                /></button
              >
              <div
                class="youtube-video-menu"
                id={`youtube-video-menu-${video.id}`}
                popover="auto"
                role="group"
                aria-label={`Actions for ${video.title}`}
                style:top={`${videoMenuPosition.top}px`}
                style:left={`${videoMenuPosition.left}px`}
              >
                <button
                  type="button"
                  onclick={() => {
                    closeVideoMenu();
                    ondownload(video.url);
                  }}
                  data-od-id={`youtube-download-${video.id}`}
                  ><Download size={16} aria-hidden="true" /> Download</button
                >
                <button
                  type="button"
                  disabled={busyVideoId !== ""}
                  onclick={() => {
                    closeVideoMenu();
                    void toggleWatchLater(video);
                  }}
                  data-od-id={`youtube-save-later-${video.id}`}
                >
                  <Bookmark
                    size={16}
                    fill={video.watch_later_at ? "currentColor" : "none"}
                    aria-hidden="true"
                  />
                  {video.watch_later_at
                    ? "Remove from Watch Later"
                    : "Save to Watch Later"}
                </button>
              </div>
            </div>
            <div class="youtube-video-copy">
              <div class="youtube-video-title">
                <!-- eslint-disable svelte/no-navigation-without-resolve -- External YouTube destination. -->
                <a
                  href={video.url}
                  target="_blank"
                  rel="noreferrer"
                  title={video.title}><span>{video.title}</span></a
                >
                <!-- eslint-enable svelte/no-navigation-without-resolve -->
              </div>
              <div class="youtube-video-meta">
                <a
                  class="youtube-video-channel"
                  href={`https://www.youtube.com/channel/${video.channel_id}`}
                  target="_blank"
                  rel="noreferrer"
                >
                  {#if channelThumbnails[video.channel_id]}<img
                      class="youtube-channel-avatar"
                      src={channelThumbnails[video.channel_id]}
                      alt=""
                      loading="lazy"
                      referrerpolicy="no-referrer"
                    />{:else}<span
                      class="youtube-channel-mark"
                      aria-hidden="true"
                      >{channelInitial(video.channel_title)}</span
                    >{/if}
                  <strong>{video.channel_title}</strong>
                  <span class="youtube-video-stats">
                    {#if video.view_count != null}
                      <span
                        title={`${video.view_count.toLocaleString("en")} views`}
                        >{viewCountFormatter.format(video.view_count)} views</span
                      >
                      <span aria-hidden="true"> - </span>
                    {/if}
                    <time datetime={video.published_at}
                      >{publishedLabel(video.published_at)}</time
                    >
                  </span>
                </a>
              </div>
            </div>
          </article>
        {:else}
          <div class="youtube-empty">
            <CirclePlay size={32} strokeWidth={1.4} aria-hidden="true" />
            <strong
              >{activeView === "watch-later"
                ? "Nothing in Watch Later"
                : reader.subscriptions.length
                  ? "No uploads match this view"
                  : "Your channel feed is empty"}</strong
            >
            <p>
              {activeView === "watch-later"
                ? "Choose Save to Watch Later in any video’s three-dot menu to build a private viewing queue."
                : reader.subscriptions.length
                  ? "Try another category or clear the text filter."
                  : "Add a Channel ID to start building a quieter YouTube feed."}
            </p>
          </div>
        {/each}
      {/if}
    </main>
  </div>

  <dialog
    class="youtube-dialog youtube-sources-dialog settings-dialog"
    aria-labelledby="youtube-sources-title"
    {@attach captureSourcesDialog}
    onclick={(event) => event.target === sourcesDialog && sourcesDialog.close()}
    data-od-id="youtube-sources-dialog"
  >
    <header>
      <div>
        <span>[ YOUTUBE.SOURCES ]</span>
        <h2 id="youtube-sources-title">Edit Sources</h2>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close Edit Sources"
        onclick={() => sourcesDialog?.close()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </header>
    <div class="youtube-sources-search">
      <label class="youtube-search">
        <Search size={15} strokeWidth={1.8} aria-hidden="true" />
        <span class="sr-only"
          >Search sources by channel name, ID, or category</span
        >
        <input
          type="search"
          bind:value={sourceQuery}
          placeholder="Search channels or categories…"
          data-od-id="youtube-source-search"
        />
      </label>
    </div>
    <div class="youtube-sources-body">
      {#if pageError}
        <p class="youtube-form-error" role="alert">{pageError}</p>
      {/if}
      <div class="youtube-sources-actions">
        <button
          class="ui-button ui-button--primary youtube-primary-button"
          type="button"
          disabled={busyChannelId !== "" || savingGroupOrder}
          onclick={openSubscriptionDialog}
          data-od-id="youtube-add-channel"
          ><Plus size={16} aria-hidden="true" /> Add channel</button
        >
      </div>
      <details class="youtube-source-editor">
        <summary
          >Edit categories <small
            >{sourceNeedle ? `${matchingSourceGroups.length} / ` : ""}{reader
              .groups.length}</small
          ></summary
        >
        <div class="youtube-category-editor-body">
          <button
            class="ui-button ui-button--secondary youtube-secondary-button"
            type="button"
            disabled={busyChannelId !== "" || savingGroupOrder}
            onclick={openNewGroup}
            ><Plus size={16} aria-hidden="true" /> Create category</button
          >
          <h3
            class="youtube-sources-heading"
            id="youtube-source-category-heading"
          >
            {sourceNeedle ? "Matching categories" : "Category order"}
          </h3>
          <p class="dialog-note">
            {sourceNeedle
              ? "Select a category to edit it. Clear the search to reorder categories."
              : "Drag a handle to reorder, or press Space and use the arrow keys. Select a category to edit it."}
          </p>
          <DragDropProvider
            sensors={youtubeGroupSensors}
            onDragStart={startGroupDrag}
            onDragOver={previewGroupOrder}
            onDragEnd={(event) => void finishGroupDrag(event)}
          >
            <div class="youtube-group-list" aria-label="Reorderable categories">
              {#each matchingSourceGroups as group, index (group.id)}
                {#if sourceNeedle}
                  <button
                    class="ui-button ui-button--secondary youtube-secondary-button"
                    type="button"
                    disabled={savingGroupOrder || busyChannelId !== ""}
                    aria-label={`Edit ${group.name} category`}
                    onclick={() => openEditGroup(group)}>{group.name}</button
                  >
                {:else}
                  <YoutubeGroupSortable
                    {group}
                    {index}
                    active={false}
                    disabled={savingGroupOrder || busyChannelId !== ""}
                    reducedMotion={reducedMotion.current}
                    onselect={() => openEditGroup(group)}
                  />
                {/if}
              {:else}
                <p class="dialog-note">
                  {sourceNeedle
                    ? "No categories match your search."
                    : "No categories yet."}
                </p>
              {/each}
            </div>
          </DragDropProvider>
        </div>
      </details>
      <aside class="youtube-directory" data-od-id="youtube-channel-directory">
        <h3 class="youtube-sources-heading">
          Channels <span
            >{sourceNeedle ? `${matchingSources.length} / ` : ""}{reader
              .subscriptions.length}</span
          >
        </h3>
        {#each matchingSources as subscription (subscription.channel_id)}
          <article
            class="youtube-channel"
            data-od-id={`youtube-channel-${subscription.channel_id}`}
          >
            <div class="youtube-channel-identity">
              {#if subscription.thumbnail_url}<img
                  class="youtube-channel-avatar directory-avatar"
                  src={subscription.thumbnail_url}
                  alt=""
                  loading="lazy"
                  referrerpolicy="no-referrer"
                />{:else}<span
                  class="youtube-channel-mark directory-mark"
                  aria-hidden="true">{channelInitial(subscription.title)}</span
                >{/if}
              <span class="youtube-channel-name">
                <strong>{subscription.title}</strong><small
                  >{subscription.channel_id}</small
                >
              </span>
            </div>
            <!-- eslint-disable svelte/no-navigation-without-resolve -- External YouTube destination. -->
            <a
              class="youtube-channel-external"
              href={subscription.channel_url}
              target="_blank"
              rel="noreferrer"
              aria-label={`Open ${subscription.title} on YouTube`}
              data-od-id={`youtube-channel-external-${subscription.channel_id}`}
            >
              <ExternalLink size={16} strokeWidth={1.8} aria-hidden="true" />
            </a>
            <!-- eslint-enable svelte/no-navigation-without-resolve -->
            <p class:error={subscription.last_error !== null}>
              {subscription.last_error ??
                dateLabel(subscription.last_fetched_at)}
            </p>
            <details
              class="youtube-source-editor"
              name="youtube-channel-categories"
            >
              <summary>
                Edit categories
                <small
                  >{reader.groups.filter((group) =>
                    group.channel_ids.includes(subscription.channel_id),
                  ).length} assigned</small
                >
              </summary>
              <fieldset class="youtube-source-categories">
                <legend>Categories</legend>
                {#each reader.groups as group (group.id)}
                  <button
                    class="ui-toggle-button"
                    type="button"
                    aria-label={`${subscription.title}: ${group.name}`}
                    aria-pressed={group.channel_ids.includes(
                      subscription.channel_id,
                    )}
                    disabled={busyChannelId !== "" || savingGroupOrder}
                    onclick={() => toggleChannelCategory(subscription, group)}
                  >
                    <span class="ui-toggle-indicator" aria-hidden="true"></span>
                    <span>{group.name}</span>
                  </button>
                {:else}
                  <p class="dialog-note">
                    Create a category to organize this channel.
                  </p>
                {/each}
              </fieldset>
            </details>
            <div class="youtube-channel-actions">
              <button
                type="button"
                disabled={busyChannelId !== "" || savingGroupOrder}
                onclick={() => refreshChannel(subscription)}
              >
                <RefreshCw
                  class={busyChannelId === subscription.channel_id
                    ? "spinning"
                    : undefined}
                  size={14}
                  strokeWidth={1.8}
                  aria-hidden="true"
                /> Check now
              </button>
              <button
                class="ui-button ui-button--danger"
                class:confirm={pendingChannelDelete === subscription.channel_id}
                type="button"
                disabled={busyChannelId !== "" || savingGroupOrder}
                onclick={() => removeChannel(subscription)}
              >
                <Trash2
                  size={14}
                  strokeWidth={1.8}
                  aria-hidden="true"
                />{pendingChannelDelete === subscription.channel_id
                  ? "Confirm"
                  : "Remove"}
              </button>
            </div>
          </article>
        {:else}
          <p class="youtube-directory-empty" role="status">
            {sourceNeedle
              ? "No channels match your search."
              : "No channels subscribed."}
          </p>
        {/each}
      </aside>
    </div>
  </dialog>

  <dialog
    class="youtube-dialog settings-dialog"
    {@attach captureSubscriptionDialog}
    onclick={(event) =>
      event.target === subscriptionDialog && subscriptionDialog.close()}
    data-od-id="youtube-subscribe-dialog"
  >
    <header>
      <div>
        <span>[ NEW.SUBSCRIPTION ]</span>
        <h2>Add a YouTube channel</h2>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close channel dialog"
        onclick={() => subscriptionDialog?.close()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </header>
    <form onsubmit={subscribe}>
      <label for="youtube-channel-id">Channel ID</label>
      <input
        id="youtube-channel-id"
        bind:value={channelId}
        {@attach captureChannelInput}
        placeholder="UCxxxxxxxxxxxxxxxxxxxxxx"
        minlength="24"
        maxlength="24"
        pattern="UC[A-Za-z0-9_-]{22}"
        required
      />
      <p class="dialog-note">
        On the channel page, open the channel description, choose <strong
          >Share channel</strong
        >, then <strong>Copy channel ID</strong>. Pandan uses YouTube’s public
        feed—no API key is required.
      </p>
      <fieldset>
        <legend>Add to categories <span>(optional)</span></legend>
        {#each reader.groups as group (group.id)}
          {@const selected = subscriptionGroupIds.includes(group.id)}
          <button
            class="ui-toggle-button youtube-channel-toggle"
            type="button"
            aria-pressed={selected}
            onclick={() => toggleSubscriptionGroup(group.id)}
          >
            <span class="ui-toggle-indicator" aria-hidden="true">
              {#if selected}<Check size={13} />{/if}
            </span>
            <span>
              <strong>{group.name}</strong>
              <small
                >{group.channel_ids.length}
                {group.channel_ids.length === 1 ? "channel" : "channels"}</small
              >
            </span>
          </button>
        {:else}
          <p>No categories yet. The channel will stay in All channels.</p>
        {/each}
      </fieldset>
      {#if subscriptionError}<p class="youtube-form-error" role="alert">
          {subscriptionError}
        </p>{/if}
      <footer>
        <button
          class="ui-button ui-button--secondary youtube-secondary-button"
          type="button"
          onclick={() => subscriptionDialog?.close()}>Cancel</button
        ><button
          class="ui-button ui-button--primary youtube-primary-button"
          type="submit"
          disabled={savingSubscription}
          >{savingSubscription ? "Checking channel…" : "Subscribe"}</button
        >
      </footer>
    </form>
  </dialog>

  <dialog
    class="youtube-dialog settings-dialog"
    {@attach captureGroupDialog}
    onclick={(event) => event.target === groupDialog && groupDialog.close()}
    data-od-id="youtube-group-dialog"
  >
    <header>
      <div>
        <span>[ CHANNEL.CATEGORY ]</span>
        <h2>{editingGroupId ? "Manage category" : "Create category"}</h2>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close group dialog"
        onclick={() => groupDialog?.close()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </header>
    <form onsubmit={saveGroup}>
      <label for="youtube-group-name">Category name</label>
      <input
        id="youtube-group-name"
        bind:value={groupName}
        {@attach captureGroupNameInput}
        placeholder="Gaming"
        maxlength="40"
        required
      />
      <fieldset>
        <legend>Channels in this group</legend>
        {#each reader.subscriptions as subscription (subscription.channel_id)}
          {@const selected = groupChannelIds.includes(subscription.channel_id)}
          <button
            class="ui-toggle-button youtube-channel-toggle"
            type="button"
            aria-pressed={selected}
            onclick={() => toggleGroupChannel(subscription.channel_id)}
            ><span class="ui-toggle-indicator" aria-hidden="true"
              >{#if selected}<Check size={13} />{/if}</span
            ><span
              ><strong>{subscription.title}</strong><small
                >{subscription.channel_id}</small
              ></span
            ></button
          >
        {:else}<p>Subscribe to a channel before adding it to a group.</p>{/each}
      </fieldset>
      <p class="dialog-note">A channel can belong to more than one category.</p>
      {#if groupError}<p class="youtube-form-error" role="alert">
          {groupError}
        </p>{/if}
      <footer>
        {#if editingGroupId}<button
            class="ui-button ui-button--danger youtube-danger-button"
            type="button"
            disabled={savingGroup}
            onclick={removeGroup}
            >{confirmingGroupDelete
              ? "Confirm remove"
              : "Remove category"}</button
          >{/if}<button
          class="ui-button ui-button--secondary youtube-secondary-button"
          type="button"
          onclick={() => groupDialog?.close()}>Cancel</button
        ><button
          class="ui-button ui-button--primary youtube-primary-button"
          type="submit"
          disabled={savingGroup}
          >{savingGroup ? "Saving…" : "Save category"}</button
        >
      </footer>
    </form>
  </dialog>
</section>

<style>
  .youtube-page {
    height: var(--product-view-height);
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
    overflow: hidden;
    padding: clamp(24px, 3vw, 42px);
  }
  .youtube-header,
  .youtube-view-tabs,
  .youtube-toolbar,
  .youtube-message {
    flex: 0 0 auto;
  }
  .youtube-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  .youtube-header span,
  .youtube-dialog header span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
  }
  .youtube-header p {
    margin-top: 8px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .youtube-header-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  button,
  input {
    font: inherit;
  }
  button {
    min-height: 44px;
  }
  button:focus-visible,
  input:focus-visible,
  a:focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 3px;
  }
  .youtube-primary-button,
  .youtube-secondary-button,
  .youtube-danger-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    min-height: 44px;
    padding: 0 14px;
    border: 1px solid var(--fg);
    border-radius: 7px;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 560;
    letter-spacing: 0.02em;
  }
  .youtube-primary-button {
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-primary-button:hover {
    background: color-mix(in oklch, var(--fg) 88%, var(--surface));
    color: var(--surface);
  }
  .youtube-secondary-button,
  .youtube-danger-button {
    border-color: var(--border);
    background: var(--surface);
    color: var(--fg);
  }
  .youtube-secondary-button:hover,
  .youtube-danger-button:hover {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-width: 0;
  }
  .youtube-view-tabs {
    display: flex;
    gap: 6px;
    overflow-x: auto;
  }
  .youtube-view-tabs span {
    color: inherit;
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }
  .youtube-watch-later-note {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .youtube-toolbar nav {
    display: flex;
    gap: 4px;
    min-width: 0;
    overflow-x: auto;
    padding-bottom: 2px;
  }
  .youtube-group-list {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }
  .youtube-toolbar nav button {
    flex: 0 0 auto;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--surface);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
    transition:
      border-color 120ms var(--ease-out),
      background-color 120ms var(--ease-out),
      color 120ms var(--ease-out);
  }
  .youtube-toolbar nav button:hover {
    border-color: var(--fg);
    background: var(--surface);
    color: var(--fg);
  }
  .youtube-toolbar nav button[aria-pressed="true"],
  .youtube-toolbar nav button[aria-pressed="true"]:hover {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-toolbar-right {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .youtube-search {
    display: flex;
    align-items: center;
    gap: 8px;
    width: min(230px, 24vw);
    min-height: 44px;
    padding: 0 11px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--muted);
  }
  .youtube-search input {
    width: 100%;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .display-switch {
    display: flex;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--surface);
  }
  .display-switch button {
    width: 44px;
    min-height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid transparent;
    border-radius: 0;
    background: transparent;
    color: var(--muted);
    transition:
      border-color 120ms var(--ease-out),
      background-color 120ms var(--ease-out),
      color 120ms var(--ease-out);
  }
  .display-switch button:hover {
    border-color: var(--fg);
    background: transparent;
    color: var(--fg);
  }
  .display-switch button[aria-pressed="true"],
  .display-switch button[aria-pressed="true"]:hover {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-message {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 11px 13px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .youtube-message button {
    color: var(--fg);
    text-decoration: underline;
  }
  .youtube-layout {
    min-height: 0;
    flex: 1 1 auto;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 18px;
    align-items: stretch;
    overflow: hidden;
  }
  .youtube-feed {
    min-width: 0;
    min-height: 0;
    align-content: start;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    border: 0;
    background: transparent;
  }
  .youtube-directory {
    border: 1px solid var(--border);
    background: var(--surface);
  }
  .youtube-sources-dialog {
    width: min(760px, calc(100vw - 32px));
  }
  .youtube-sources-body {
    display: grid;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    gap: 22px;
    padding: 20px;
  }
  .youtube-sources-search {
    flex: 0 0 auto;
    min-width: 0;
    padding: 14px 20px;
    border-bottom: 1px solid var(--border);
  }
  .youtube-sources-search .youtube-search {
    width: 100%;
  }
  .youtube-sources-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .youtube-sources-heading {
    padding: 12px 0;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
  }
  .youtube-directory > .youtube-sources-heading {
    padding: 16px;
    border-bottom: 1px solid var(--border);
  }
  .youtube-sources-heading span {
    color: var(--muted);
  }
  .youtube-source-editor {
    min-width: 0;
    border: 1px solid var(--border);
  }
  .youtube-channel > .youtube-source-editor {
    margin-top: 12px;
  }
  .youtube-source-editor > summary {
    min-height: 44px;
    padding: 10px 12px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 24px;
    cursor: pointer;
  }
  .youtube-source-editor > summary:hover {
    background: var(--fg-soft);
  }
  .youtube-source-editor > summary:focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: -2px;
  }
  .youtube-source-editor > summary small {
    margin-left: 8px;
    color: var(--muted);
    font-size: 10px;
  }
  .youtube-category-editor-body {
    padding: 12px;
    border-top: 1px solid var(--border);
  }
  .youtube-source-editor > .youtube-source-categories {
    margin: 0 12px 12px;
  }
  .youtube-source-categories {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin: 12px 0 0;
    padding: 0;
    border: 0;
    min-width: 0;
  }
  .youtube-source-categories .ui-toggle-button {
    min-width: 0;
    max-width: 100%;
  }
  .youtube-source-categories .ui-toggle-button > span:last-child {
    overflow-wrap: anywhere;
  }
  .youtube-feed.thumbnails {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 16px 14px;
    background: transparent;
  }
  .youtube-feed.compact {
    display: grid;
    gap: 0;
    border: 0;
    background: transparent;
  }
  .youtube-video {
    min-width: 0;
    margin: 0;
    background: var(--surface);
  }
  .thumbnails .youtube-video {
    display: grid;
    grid-template-rows: auto 1fr;
    align-content: start;
    max-height: 320px;
  }
  .youtube-video-media {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  .thumbnails .youtube-video-copy {
    gap: 4px;
    padding: 10px 2px 4px;
    grid-template-rows: 44px auto;
  }
  .youtube-thumbnail {
    position: relative;
    aspect-ratio: 16 / 9;
    max-height: 210px;
    display: grid;
    line-height: 0;
    place-items: center;
    overflow: hidden;
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-thumbnail img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 180ms var(--ease-out);
  }
  .youtube-duration {
    position: absolute;
    right: 6px;
    bottom: 6px;
    padding: 3px 5px;
    border-radius: 3px;
    background: rgb(0 0 0 / 85%);
    color: #fff;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
    line-height: 1.2;
    pointer-events: none;
  }
  .youtube-video-menu-trigger {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid color-mix(in oklch, var(--fg) 18%, transparent);
    border-radius: 6px;
    background: color-mix(in oklch, var(--surface) 78%, transparent);
    color: var(--fg);
    opacity: 0.85;
  }
  .youtube-video-menu-trigger:hover,
  .youtube-video-menu-trigger:focus-visible {
    opacity: 1;
    background: var(--surface);
  }
  .youtube-video-menu {
    position: fixed;
    inset: auto;
    width: min(220px, calc(100vw - 16px));
    margin: 0;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--fg);
  }
  .youtube-video-menu button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px;
    text-align: left;
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .youtube-video-menu button:hover,
  .youtube-video-menu button:focus-visible {
    background: var(--fg-soft);
  }
  .youtube-video-menu button :global(svg) {
    flex: 0 0 auto;
  }
  .youtube-thumbnail:hover img {
    transform: scale(1.025);
  }
  .youtube-video-copy {
    display: grid;
    gap: 9px;
    padding: 15px;
  }
  .youtube-video-title {
    min-width: 0;
    height: 44px;
    overflow: hidden;
  }
  .youtube-video-title > a {
    min-height: 44px;
    display: block;
    max-height: 44px;
    overflow: hidden;
    overflow-wrap: anywhere;
    color: var(--fg);
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.015em;
    line-height: 1.3;
    text-decoration: none;
  }
  .youtube-video-title > a > span {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    max-height: 2.6em;
  }
  .youtube-video-title > a:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .youtube-video-meta {
    min-width: 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .youtube-video-channel {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr);
    grid-template-rows: auto auto;
    align-items: center;
    column-gap: 8px;
    row-gap: 3px;
    min-width: 0;
    min-height: 44px;
    color: var(--fg);
    text-decoration: none;
  }
  .youtube-video-channel:hover strong {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .youtube-video-channel strong {
    grid-column: 2;
    min-width: 0;
    overflow: hidden;
    font-size: 14px;
    font-weight: 600;
    letter-spacing: -0.01em;
    line-height: 1.2;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .youtube-channel-mark,
  .youtube-channel-avatar {
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    border: 1px solid var(--border);
    border-radius: 50%;
    background: var(--fg-soft);
  }
  .youtube-channel-mark {
    display: grid;
    place-items: center;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
    line-height: 1;
  }
  .youtube-channel-avatar {
    display: block;
    object-fit: cover;
  }
  .youtube-video-channel .youtube-channel-avatar,
  .youtube-video-channel .youtube-channel-mark {
    grid-row: 1 / 3;
    width: 36px;
    height: 36px;
  }
  .youtube-video-stats {
    grid-column: 2;
    min-width: 0;
    color: var(--muted);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }
  .youtube-video-meta time {
    font-size: 10px;
  }
  .youtube-feed.compact .youtube-video {
    display: grid;
    grid-template-columns: 152px minmax(0, 1fr);
    height: 108px;
    max-height: 108px;
    align-items: stretch;
    border: 0;
    border-bottom: 1px solid var(--border);
  }
  .youtube-feed.compact .youtube-video:last-of-type {
    border-bottom: 0;
  }
  .youtube-feed.compact .youtube-video-copy {
    grid-template-columns: minmax(0, 1fr) minmax(0, 220px);
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
  }
  .youtube-feed.compact .youtube-thumbnail {
    height: 100%;
    min-height: 92px;
    margin: 0;
    aspect-ratio: auto;
    border: 0;
  }
  .youtube-feed.compact .youtube-video-title > a {
    min-height: auto;
  }
  .youtube-channel {
    position: relative;
    padding: 14px;
    border-bottom: 1px solid var(--border);
  }
  .youtube-channel:last-child {
    border-bottom: 0;
  }
  .youtube-channel-identity {
    position: relative;
    z-index: 1;
    min-height: 44px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 11px;
    padding-right: 54px;
    color: var(--fg);
    pointer-events: none;
  }
  .youtube-channel-external {
    position: absolute;
    top: 14px;
    right: 14px;
    z-index: 2;
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    color: var(--muted);
    transition:
      border-color 120ms var(--ease-out),
      background-color 120ms var(--ease-out),
      color 120ms var(--ease-out);
  }
  .youtube-channel-external:hover {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-channel-name {
    min-width: 0;
    display: grid;
    gap: 3px;
  }
  .directory-mark,
  .directory-avatar {
    width: 40px;
    height: 40px;
  }
  .directory-mark {
    font-size: 14px;
  }
  .youtube-channel-name strong,
  .youtube-channel-name small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .youtube-channel-name strong {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .youtube-channel-name small,
  .youtube-channel > p {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .youtube-channel > p {
    position: relative;
    z-index: 1;
    margin-top: 8px;
    line-height: 1.45;
    pointer-events: none;
  }
  .youtube-channel > p.error {
    color: var(--fg);
  }
  .youtube-channel-actions {
    position: relative;
    z-index: 2;
    display: flex;
    gap: 6px;
    margin-top: 11px;
  }
  .youtube-channel-actions button {
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 0 10px;
    border: 1px solid var(--border);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .youtube-channel-actions button:hover,
  .youtube-channel-actions button.confirm {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-directory-empty {
    padding: 20px 15px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .youtube-empty {
    grid-column: 1 / -1;
    min-height: 360px;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 8px;
    padding: 30px;
    background: var(--surface);
    color: var(--muted);
    text-align: center;
  }
  .youtube-empty strong {
    color: var(--fg);
    font-family: var(--font-display);
    font-size: 19px;
  }
  .youtube-empty p {
    max-width: 44ch;
    font-size: 12px;
  }
  .youtube-dialog {
    width: min(620px, calc(100vw - 32px));
    max-height: min(780px, calc(100vh - 32px));
    margin: auto;
    padding: 0;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    color: var(--fg);
    box-shadow: 0 24px 80px color-mix(in oklch, var(--bg) 72%, transparent);
  }
  .youtube-dialog::backdrop {
    background: color-mix(in oklch, var(--bg) 72%, transparent);
    backdrop-filter: blur(7px);
  }
  .youtube-dialog header {
    min-height: 76px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }
  .youtube-dialog header h2 {
    margin-top: 5px;
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .youtube-dialog header > button {
    width: 44px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: 7px;
  }
  .youtube-dialog form {
    display: grid;
    gap: 11px;
    padding: 22px;
  }
  .youtube-dialog form > label,
  .youtube-dialog legend {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .youtube-dialog legend span {
    color: var(--muted);
    letter-spacing: 0.02em;
    text-transform: none;
  }
  .youtube-dialog form > input {
    min-height: 46px;
    width: 100%;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .dialog-note {
    color: var(--muted);
    font-size: 11px;
    line-height: 1.6;
  }
  .dialog-note strong {
    color: var(--fg);
    font-weight: 560;
  }
  .youtube-dialog fieldset {
    display: grid;
    gap: 1px;
    margin: 5px 0 0;
    padding: 0;
    border: 1px solid var(--border);
  }
  .youtube-dialog fieldset legend {
    margin: 0 10px;
    padding: 0 5px;
  }
  .youtube-channel-toggle {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 52px;
    padding: 8px 12px;
    border: 0;
    border-bottom: 1px solid var(--border);
    border-radius: 0;
  }
  .youtube-channel-toggle:last-child {
    border-bottom: 0;
  }
  .youtube-channel-toggle > span:last-child {
    min-width: 0;
    display: grid;
  }
  .youtube-channel-toggle strong,
  .youtube-channel-toggle small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .youtube-channel-toggle strong {
    font-size: 12px;
    font-weight: 560;
  }
  .youtube-channel-toggle small,
  .youtube-dialog fieldset > p {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .youtube-dialog fieldset > p {
    padding: 18px;
  }
  .youtube-form-error {
    padding: 10px;
    border: 1px solid var(--border);
    background: var(--fg-soft);
    color: var(--fg);
    font-size: 11px;
  }
  .youtube-dialog footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  .youtube-dialog footer .youtube-danger-button:first-child {
    margin-right: auto;
  }
  @media (max-width: 1100px) {
    .youtube-toolbar {
      align-items: stretch;
      flex-direction: column;
    }
    .youtube-toolbar-right {
      flex-wrap: wrap;
    }
    .youtube-search {
      width: min(320px, 100%);
      flex: 1;
    }
  }
  @media (max-width: 920px) {
    .youtube-page {
      height: auto;
      min-height: var(--product-view-height);
      overflow: visible;
      padding: 20px 16px;
    }
    .youtube-header {
      align-items: start;
      flex-direction: column;
    }
    .youtube-layout {
      min-height: auto;
      flex: 0 0 auto;
      grid-template-columns: 1fr;
      align-items: start;
      overflow: visible;
    }
    .youtube-feed {
      min-height: auto;
      overflow: visible;
      overscroll-behavior: auto;
      scrollbar-gutter: auto;
    }
  }
  @media (max-width: 760px) {
    .youtube-feed.thumbnails {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 600px) {
    .youtube-header-actions {
      width: 100%;
    }
    .youtube-header-actions > button {
      flex: 1;
    }
    .youtube-feed.thumbnails {
      gap: 12px 10px;
    }
    .thumbnails .youtube-video-copy {
      padding-top: 8px;
    }
    .youtube-video-title > a {
      font-size: 14px;
      line-height: 1.4;
    }
    .youtube-video-title > a > span {
      max-height: 2.8em;
    }
    .youtube-video-channel {
      width: 100%;
      grid-template-columns: 30px minmax(0, 1fr);
      column-gap: 5px;
    }
    .youtube-video-channel strong {
      font-size: 11px;
    }
    .youtube-video-channel .youtube-channel-avatar,
    .youtube-video-channel .youtube-channel-mark {
      width: 30px;
      height: 30px;
    }
    .youtube-sources-body {
      padding: 14px;
    }
    .youtube-feed.compact .youtube-video-copy {
      grid-template-columns: 1fr;
      gap: 5px;
      padding: 9px 11px;
    }
    .youtube-feed.compact .youtube-video {
      grid-template-columns: 112px minmax(0, 1fr);
      height: 128px;
      max-height: 128px;
    }
    .youtube-feed.compact .youtube-thumbnail {
      min-height: 84px;
    }
    .youtube-toolbar-right {
      flex-wrap: nowrap;
    }
    .youtube-search {
      flex: 1;
      width: auto;
      min-width: 0;
    }
    .youtube-dialog footer {
      flex-wrap: wrap;
    }
    .youtube-dialog footer button {
      flex: 1;
    }
    .youtube-dialog footer .youtube-danger-button:first-child {
      flex-basis: 100%;
      margin-right: 0;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .youtube-thumbnail img {
      transition: none;
    }
    .youtube-thumbnail:hover img {
      transform: none;
    }
  }
</style>

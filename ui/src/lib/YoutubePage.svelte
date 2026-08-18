<script lang="ts">
  import CirclePlay from "lucide-svelte/icons/circle-play";
  import Bookmark from "lucide-svelte/icons/bookmark";
  import Check from "lucide-svelte/icons/check";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import Image from "lucide-svelte/icons/image";
  import Layers3 from "lucide-svelte/icons/layers-3";
  import List from "lucide-svelte/icons/list";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Settings2 from "lucide-svelte/icons/settings-2";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import {
    createYoutubeGroup,
    createYoutubeSubscription,
    deleteYoutubeGroup,
    deleteYoutubeSubscription,
    fetchYoutubeReader,
    refreshYoutubeSubscription,
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

  let reader = $state.raw<YoutubeReaderResponse>({
    subscriptions: [],
    groups: [],
    videos: [],
    watch_later: [],
    display_mode: "thumbnails",
  });
  let loading = $state(true);
  let pageError = $state("");
  let query = $state("");
  let activeView = $state<YoutubeView>("latest");
  let activeGroupId = $state("all");
  let busyChannelId = $state("");
  let busyVideoId = $state("");
  let pendingChannelDelete = $state("");

  let subscriptionDialog = $state<HTMLDialogElement>();
  let channelInput = $state<HTMLInputElement>();
  let channelId = $state("");
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
    const videos = activeView === "watch-later" ? reader.watch_later : reader.videos;
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
    loading = true;
    pageError = "";
    try {
      reader = await fetchYoutubeReader();
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to load YouTube subscriptions");
    } finally {
      loading = false;
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
      reader = await createYoutubeSubscription(channelId.trim());
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
</script>

<section class="youtube-page product-page" data-od-id="youtube-page">
  <header class="youtube-header page-header" data-od-id="youtube-heading">
    <div>
      <h2>$ youtube --{activeView}</h2>
      <p>
        {activeView === "latest"
          ? `${reader.subscriptions.length} channels · ${reader.videos.length} stored uploads · refreshes every 2 hours`
          : `${reader.watch_later.length} saved ${reader.watch_later.length === 1 ? "video" : "videos"}`}
      </p>
    </div>
    <button
      class="ui-button ui-button--primary youtube-primary-button"
      type="button"
      onclick={openSubscriptionDialog}
      data-od-id="youtube-add-channel"
    >
      <Plus size={16} strokeWidth={2} aria-hidden="true" />
      Add channel
    </button>
  </header>

  <nav class="youtube-view-tabs" aria-label="YouTube reader views" data-od-id="youtube-reader-views">
    <button
      class={activeView === "latest" ? "active" : undefined}
      type="button"
      aria-pressed={activeView === "latest"}
      onclick={() => selectView("latest")}
      data-od-id="youtube-latest-view"
    >
      Latest <span>{reader.videos.length}</span>
    </button>
    <button
      class={activeView === "watch-later" ? "active" : undefined}
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
      <nav aria-label="YouTube groups">
        <button
          class:active={activeGroupId === "all"}
          type="button"
          onclick={() => (activeGroupId = "all")}>All channels</button
        >
        {#each reader.groups as group (group.id)}
          <button
            class:active={activeGroupId === group.id}
            type="button"
            onclick={() => (activeGroupId = group.id)}
            data-od-id={`youtube-group-${group.id}`}>{group.name}</button
          >
        {/each}
        <button
          class="group-add"
          type="button"
          onclick={openNewGroup}
          aria-label="Create channel group"
        >
          <Plus size={15} strokeWidth={1.9} aria-hidden="true" /> Group
        </button>
      </nav>
    {:else}
      <p class="youtube-watch-later-note">Saved videos stay here after you unsubscribe.</p>
    {/if}
    <div class="youtube-toolbar-right">
      {#if activeGroup}
        <button
          class="group-manage"
          type="button"
          onclick={() => openEditGroup(activeGroup)}
        >
          <Settings2 size={15} strokeWidth={1.8} aria-hidden="true" /> Manage {activeGroup.name}
        </button>
      {/if}
      <label class="youtube-search">
        <Search size={15} strokeWidth={1.8} aria-hidden="true" />
        <span class="sr-only">Search videos and channels</span>
        <input type="search" bind:value={query} placeholder="Filter uploads…" />
      </label>
      <div class="display-switch" role="group" aria-label="Video display mode">
        <button
          class:active={reader.display_mode === "thumbnails"}
          type="button"
          aria-label="Show thumbnails"
          aria-pressed={reader.display_mode === "thumbnails"}
          onclick={() => setDisplayMode("thumbnails")}
        >
          <Image size={16} strokeWidth={1.8} aria-hidden="true" />
        </button>
        <button
          class:active={reader.display_mode === "compact"}
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
      aria-label={activeView === "latest" ? "YouTube uploads" : "YouTube Watch Later"}
      data-od-id={activeView === "latest" ? "youtube-video-feed" : "youtube-watch-later-feed"}
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
          <article
            class="youtube-video"
            data-od-id={`youtube-video-${video.id}`}
          >
            <a
              class="youtube-thumbnail"
              href={video.url}
              target="_blank"
              rel="noreferrer"
              aria-label={`Watch ${video.title}`}
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
              <span
                ><CirclePlay
                  size={18}
                  fill="currentColor"
                  strokeWidth={1.5}
                  aria-hidden="true"
                /></span
              >
            </a>
            <div class="youtube-video-copy">
              <div class="youtube-video-title">
                <a href={video.url} target="_blank" rel="noreferrer"
                  >{video.title}<ExternalLink
                    size={13}
                    strokeWidth={1.7}
                    aria-hidden="true"
                  /></a
                >
                <button
                  class={[
                    "youtube-watch-later-button",
                    video.watch_later_at !== null && "active",
                  ]}
                  type="button"
                  disabled={busyVideoId !== ""}
                  aria-label={video.watch_later_at
                    ? `Remove ${video.title} from Watch Later`
                    : `Save ${video.title} to Watch Later`}
                  title={video.watch_later_at ? "Remove from Watch Later" : "Save to Watch Later"}
                  onclick={() => toggleWatchLater(video)}
                  data-od-id={`youtube-save-later-${video.id}`}
                >
                  <Bookmark
                    size={16}
                    strokeWidth={1.8}
                    fill={video.watch_later_at ? "currentColor" : "none"}
                    aria-hidden="true"
                  />
                </button>
              </div>
              <div class="youtube-video-meta">
                <a
                  class="youtube-video-channel"
                  href={`https://www.youtube.com/channel/${video.channel_id}`}
                  target="_blank"
                  rel="noreferrer"
                  aria-label={`Open ${video.channel_title} on YouTube`}
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
                </a>
                <time datetime={video.published_at}
                  >{publishedLabel(video.published_at)}</time
                >
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
                ? "Use the bookmark control on any upload to build a private viewing queue."
                : reader.subscriptions.length
                  ? "Try another group or clear the text filter."
                  : "Add a Channel ID to start building a quieter YouTube feed."}
            </p>
          </div>
        {/each}
      {/if}
    </main>

    <aside class="youtube-directory" data-od-id="youtube-channel-directory">
      <header>
        <div>
          <span>[ SUBSCRIPTIONS ]</span><strong
            >{reader.subscriptions.length}</strong
          >
        </div>
        <Layers3 size={18} strokeWidth={1.6} aria-hidden="true" />
      </header>
      {#each reader.subscriptions as subscription (subscription.channel_id)}
        <article
          class="youtube-channel"
          data-od-id={`youtube-channel-${subscription.channel_id}`}
        >
          <a href={subscription.channel_url} target="_blank" rel="noreferrer">
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
          </a>
          <p class:error={subscription.last_error !== null}>
            {subscription.last_error ?? dateLabel(subscription.last_fetched_at)}
          </p>
          <div>
            <button
              type="button"
              disabled={busyChannelId !== ""}
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
              disabled={busyChannelId !== ""}
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
        <p class="youtube-directory-empty">No channels subscribed.</p>
      {/each}
    </aside>
  </div>

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
        <span>[ CHANNEL.GROUP ]</span>
        <h2>{editingGroupId ? "Manage group" : "Create group"}</h2>
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
      <label for="youtube-group-name">Group name</label>
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
          <button class="ui-toggle-button youtube-channel-toggle" type="button" aria-pressed={selected} onclick={() => toggleGroupChannel(subscription.channel_id)}
            ><span class="ui-toggle-indicator" aria-hidden="true">{#if selected}<Check size={13} />{/if}</span><span
              ><strong>{subscription.title}</strong><small
                >{subscription.channel_id}</small
              ></span
            ></button
          >
        {:else}<p>Subscribe to a channel before adding it to a group.</p>{/each}
      </fieldset>
      <p class="dialog-note">A channel can belong to more than one group.</p>
      {#if groupError}<p class="youtube-form-error" role="alert">
          {groupError}
        </p>{/if}
      <footer>
        {#if editingGroupId}<button
            class="ui-button ui-button--danger youtube-danger-button"
            type="button"
            disabled={savingGroup}
            onclick={removeGroup}
            >{confirmingGroupDelete ? "Confirm remove" : "Remove group"}</button
          >{/if}<button
          class="ui-button ui-button--secondary youtube-secondary-button"
          type="button"
          onclick={() => groupDialog?.close()}>Cancel</button
        ><button
          class="ui-button ui-button--primary youtube-primary-button"
          type="submit"
          disabled={savingGroup}
          >{savingGroup ? "Saving…" : "Save group"}</button
        >
      </footer>
    </form>
  </dialog>
</section>

<style>
  .youtube-page {
    display: grid;
    gap: 18px;
    min-width: 0;
    padding: clamp(24px, 3vw, 42px);
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
  .youtube-dialog header span,
  .youtube-directory header span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
  }
  .youtube-header h2 {
    margin-top: 8px;
    font-family: var(--font-mono);
    font-size: clamp(26px, 3vw, 42px);
    font-weight: 540;
    letter-spacing: -0.04em;
    line-height: 1.05;
  }
  .youtube-header p {
    margin-top: 8px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
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
  .youtube-view-tabs button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 0 13px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .youtube-view-tabs button:hover,
  .youtube-view-tabs button.active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
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
  .youtube-toolbar nav button,
  .group-manage {
    flex: 0 0 auto;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .youtube-toolbar nav button:hover,
  .group-manage:hover {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-toolbar nav button.active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-toolbar .group-add,
  .group-manage {
    display: inline-flex;
    align-items: center;
    gap: 6px;
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
    border-radius: 7px;
    background: var(--surface);
  }
  .display-switch button {
    width: 44px;
    min-height: 44px;
    display: grid;
    place-items: center;
    border-radius: 4px;
    color: var(--muted);
  }
  .display-switch button:hover,
  .display-switch button.active {
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(250px, 310px);
    gap: 18px;
    align-items: start;
  }
  .youtube-feed {
    border: 0;
    background: transparent;
  }
  .youtube-directory {
    border: 1px solid var(--border);
    background: var(--surface);
  }
  .youtube-feed.thumbnails {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
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
    align-content: start;
  }
  .thumbnails .youtube-video-copy {
    gap: 4px;
    padding: 10px 2px 4px;
  }
  .youtube-thumbnail {
    position: relative;
    aspect-ratio: 16 / 9;
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
  .youtube-thumbnail > span {
    position: absolute;
    right: 10px;
    bottom: 10px;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 1px solid color-mix(in oklch, var(--surface) 45%, transparent);
    border-radius: 50%;
    background: color-mix(in oklch, var(--fg) 78%, transparent);
    color: var(--surface);
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: start;
    gap: 6px;
  }
  .youtube-watch-later-button {
    width: 44px;
    min-height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid transparent;
    border-radius: 5px;
    color: var(--muted);
  }
  .youtube-watch-later-button:hover,
  .youtube-watch-later-button.active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-video-title > a {
    min-height: 44px;
    display: flex;
    align-items: start;
    gap: 7px;
    color: var(--fg);
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.015em;
    line-height: 1.3;
    text-decoration: none;
  }
  .youtube-video-title > a:hover {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .youtube-video-title > :global(a svg) {
    flex: 0 0 auto;
    margin-top: 4px;
    color: var(--muted);
  }
  .youtube-video-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    min-width: 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .youtube-video-channel {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: var(--fg);
    text-decoration: none;
  }
  .youtube-video-channel:hover strong {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .youtube-video-channel strong {
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
  .youtube-video-meta time {
    flex: 0 0 auto;
    font-size: 10px;
  }
  .youtube-feed.compact .youtube-video {
    display: grid;
    grid-template-columns: 152px minmax(0, 1fr);
    min-height: 92px;
    align-items: stretch;
    border: 0;
    border-bottom: 1px solid var(--border);
  }
  .youtube-feed.compact .youtube-video:last-of-type {
    border-bottom: 0;
  }
  .youtube-feed.compact .youtube-video-copy {
    grid-template-columns: minmax(0, 1fr) 220px;
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
  .youtube-feed.compact .youtube-thumbnail > span {
    right: 6px;
    bottom: 6px;
    width: 24px;
    height: 24px;
  }
  .youtube-feed.compact .youtube-video-meta {
    justify-content: flex-end;
  }
  .youtube-directory {
    position: sticky;
    top: 20px;
  }
  .youtube-directory > header {
    min-height: 66px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 13px 15px;
    border-bottom: 1px solid var(--border);
  }
  .youtube-directory > header > div {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .youtube-directory header strong {
    font-family: var(--font-mono);
    font-size: 24px;
    font-weight: 520;
  }
  .youtube-channel {
    padding: 14px;
    border-bottom: 1px solid var(--border);
  }
  .youtube-channel:last-child {
    border-bottom: 0;
  }
  .youtube-channel > a {
    min-height: 44px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: center;
    gap: 11px;
    color: var(--fg);
    text-decoration: none;
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
  .youtube-channel > a:hover strong {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .youtube-channel > a strong,
  .youtube-channel > a small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .youtube-channel > a strong {
    font-size: 15px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .youtube-channel > a small,
  .youtube-channel > p {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .youtube-channel > p {
    margin-top: 8px;
    line-height: 1.45;
  }
  .youtube-channel > p.error {
    color: var(--fg);
  }
  .youtube-channel > div {
    display: flex;
    gap: 6px;
    margin-top: 11px;
  }
  .youtube-channel > div button {
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
  .youtube-channel > div button:hover,
  .youtube-channel > div button.confirm {
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
  :global(.spinning) {
    animation: youtube-spin 0.8s linear infinite;
  }
  @keyframes youtube-spin {
    to {
      transform: rotate(360deg);
    }
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
    max-height: 310px;
    margin: 5px 0 0;
    padding: 0;
    overflow: auto;
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
    .youtube-feed.thumbnails {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
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
      padding: 20px 16px;
    }
    .youtube-header {
      align-items: start;
      flex-direction: column;
    }
    .youtube-layout {
      grid-template-columns: 1fr;
    }
    .youtube-directory {
      position: static;
      order: -1;
    }
  }
  @media (max-width: 760px) {
    .youtube-feed.thumbnails {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 600px) {
    .youtube-header > button {
      width: 100%;
    }
    .youtube-feed.thumbnails {
      grid-template-columns: 1fr;
    }
    .youtube-feed.compact .youtube-video-copy {
      grid-template-columns: 1fr;
      gap: 5px;
      padding: 9px 11px;
    }
    .youtube-feed.compact .youtube-video {
      grid-template-columns: 112px minmax(0, 1fr);
      min-height: 84px;
    }
    .youtube-feed.compact .youtube-thumbnail {
      min-height: 84px;
    }
    .youtube-feed.compact .youtube-video-meta {
      justify-content: space-between;
    }
    .group-manage {
      width: 100%;
    }
    .youtube-search {
      flex-basis: calc(100% - 88px);
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
    :global(.spinning) {
      animation: none;
    }
  }
</style>

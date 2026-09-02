<script lang="ts">
  import Check from "lucide-svelte/icons/check";
  import ImagePlus from "lucide-svelte/icons/image-plus";
  import Pencil from "lucide-svelte/icons/pencil";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Tags from "lucide-svelte/icons/tags";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import Wallpaper from "lucide-svelte/icons/wallpaper";
  import X from "lucide-svelte/icons/x";
  import { onDestroy } from "svelte";
  import { createViewSwap } from "$lib/viewSwap.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    applyWall,
    approveWall,
    deleteWall,
    fetchWallSelections,
    fetchWalls,
    rejectWall,
    suggestWallTags,
    submitWall,
    updateWall,
    wallImageUrl,
    wallThumbnailUrl,
    type Wall,
    type WallSelections,
    type WallSlot,
  } from "$lib/api";

  type WallsView = "collection" | "mine" | "review";

  type WallsSnapshot = { walls: Wall[]; selections: WallSelections };

  type WallsPrefetch = {
    view: WallsView;
    query: string;
    tag: string;
    token: number;
    promise: Promise<WallsSnapshot>;
    value: WallsSnapshot | null;
  };

  let {
    viewerId,
    viewerRole,
    onwallapplied,
  }: {
    viewerId: string;
    viewerRole: "administrator" | "member";
    onwallapplied?: (slot: WallSlot) => void;
  } = $props();

  const acceptedTypes = ["image/jpeg", "image/png", "image/webp", "image/avif"];
  const maxUploadBytes = 30 * 1024 * 1024;
  /** Left-to-right tab order, which decides the swap direction. */
  const viewOrder: WallsView[] = ["collection", "mine", "review"];

  const isAdministrator = $derived(viewerRole === "administrator");

  let activeView = $state<WallsView>("collection");
  let walls = $state<Wall[]>([]);
  let reviewQueue = $state<Wall[]>([]);
  let selections = $state<WallSelections>({ welcome: null, login: null });
  let loading = $state(true);
  /** A reload while walls are already on screen, which must not blank the grid. */
  let refreshing = $state(false);
  /**
   * Whether a snapshot has ever landed. Deliberately not `$state`: the loader reads it
   * to choose between the first-load line and a background refresh, and a reactive read
   * there would make the loader depend on its own completion.
   */
  let hasLoaded = false;
  let pageError = $state("");
  let search = $state("");
  let activeTag = $state("");
  let reloadToken = $state(0);

  const viewSwap = createViewSwap();
  // Parked by a view swap for the loader effect to adopt, so moving between
  // views never fetches the same set twice.
  let wallsPrefetch: WallsPrefetch | null = null;

  // Submission composer.
  let submitOpen = $state(false);
  let submitDialog = $state<HTMLDialogElement>();
  let file = $state<File | null>(null);
  let filePreview = $state("");
  let title = $state("");
  let description = $state("");
  let tagInput = $state("");
  let submitting = $state(false);
  let submitError = $state("");

  // Detail view.
  let openWall = $state<Wall | null>(null);
  let detailDialog = $state<HTMLDialogElement>();
  let detailBusy = $state(false);
  let detailError = $state("");
  let detailNotice = $state("");
  let suggestingTags = $state(false);
  let decisionNote = $state("");
  let editing = $state(false);
  let editTitle = $state("");
  let editDescription = $state("");
  let editTags = $state("");

  const visibleTags = $derived(
    [...new Set(walls.flatMap((wall) => wall.tags))].sort((left, right) =>
      left.localeCompare(right),
    ),
  );

  const ownSubmissions = $derived(
    walls.filter((wall) => wall.user_id === viewerId),
  );

  onDestroy(() => {
    viewSwap.cancel();
  });

  async function loadWalls(
    view: WallsView,
    query: string,
    tag: string,
  ): Promise<WallsSnapshot> {
    const [loaded, currentSelections] = await Promise.all([
      fetchWalls({ scope: view, q: query, tag }),
      fetchWallSelections(),
    ]);
    return { walls: loaded, selections: currentSelections };
  }

  $effect(() => {
    // Re-runs whenever the view, filters, or an explicit reload token changes.
    const view = activeView;
    const query = search.trim();
    const tag = activeTag;
    const token = reloadToken;

    // A view swap starts this set loading before it commits, so adopt that
    // request rather than issuing a second one.
    const prefetched = wallsPrefetch;
    wallsPrefetch = null;
    const matched =
      prefetched &&
      prefetched.view === view &&
      prefetched.query === query &&
      prefetched.tag === tag &&
      prefetched.token === token
        ? prefetched
        : null;

    if (matched?.value) {
      walls = matched.value.walls;
      selections = matched.value.selections;
      pageError = "";
      hasLoaded = true;
      loading = false;
      return;
    }

    let cancelled = false;
    // Only the first load has nothing to show. Every later one keeps the current grid
    // up until the new set lands, and leaves it in place when the request fails.
    if (hasLoaded) refreshing = true;
    (async () => {
      try {
        const snapshot = await (matched?.promise ??
          loadWalls(view, query, tag));
        if (cancelled) return;
        walls = snapshot.walls;
        selections = snapshot.selections;
        pageError = "";
      } catch (error) {
        if (cancelled) return;
        pageError =
          error instanceof Error ? error.message : "Unable to load walls";
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

  $effect(() => {
    // The review badge count is needed on every view, not just the review tab.
    if (!isAdministrator) return;
    void reloadToken;

    let cancelled = false;
    (async () => {
      try {
        const pending = await fetchWalls({ scope: "review" });
        if (!cancelled) reviewQueue = pending;
      } catch {
        if (!cancelled) reviewQueue = [];
      }
    })();

    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    if (submitOpen) submitDialog?.showModal();
    else submitDialog?.close();
  });

  $effect(() => {
    if (openWall) detailDialog?.showModal();
    else detailDialog?.close();
  });

  $effect(() => {
    // Release the object URL when the chosen file changes or the composer closes.
    const preview = filePreview;
    return () => {
      if (preview) URL.revokeObjectURL(preview);
    };
  });

  function reload() {
    reloadToken += 1;
  }

  /**
   * Starts the request the incoming view needs and parks it for the loader.
   * The returned promise settles rather than rejects: the loader owns
   * reporting the failure.
   */
  function prefetchWalls(
    view: WallsView,
    query: string,
    tag: string,
    token: number,
  ) {
    const entry: WallsPrefetch = {
      view,
      query,
      tag,
      token,
      promise: loadWalls(view, query, tag),
      value: null,
    };
    wallsPrefetch = entry;
    return entry.promise.then(
      (loaded) => {
        if (wallsPrefetch === entry) entry.value = loaded;
      },
      () => undefined,
    );
  }

  /**
   * Moves between Collection, Submissions, and Review. The incoming set loads
   * while the outgoing one leaves, so the swap usually resolves straight into
   * walls instead of flashing the loading line.
   */
  async function showView(next: WallsView, refresh = false) {
    if (next === activeView && !refresh) return;
    const query = search.trim();
    const tag = activeTag;
    const token = refresh ? reloadToken + 1 : reloadToken;
    const pending = prefetchWalls(next, query, tag, token);
    await viewSwap.run({
      forward: viewOrder.indexOf(next) >= viewOrder.indexOf(activeView),
      pending,
      commit: () => {
        if (refresh) reloadToken = token;
        activeView = next;
      },
    });
  }

  function selectFile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const chosen = input.files?.[0] ?? null;
    if (!chosen) return;
    if (!acceptedTypes.includes(chosen.type)) {
      submitError = "Choose a JPEG, PNG, WebP, or AVIF image.";
      input.value = "";
      return;
    }
    if (chosen.size > maxUploadBytes) {
      submitError = "Wall images must be 30 MB or smaller.";
      input.value = "";
      return;
    }
    file = chosen;
    filePreview = URL.createObjectURL(chosen);
    if (!title.trim()) {
      title = chosen.name.replace(/\.[^.]+$/, "").slice(0, 120);
    }
    submitError = "";
  }

  function closeComposer() {
    submitOpen = false;
    file = null;
    filePreview = "";
    title = "";
    description = "";
    tagInput = "";
    submitError = "";
  }

  async function send() {
    if (!file || submitting) return;
    if (!title.trim()) {
      submitError = "Give this wall a title.";
      return;
    }
    submitting = true;
    submitError = "";
    try {
      await submitWall(file, {
        title: title.trim(),
        description: description.trim(),
        tags: tagInput
          .split(",")
          .map((tag) => tag.trim().replace(/^#/, ""))
          .filter(Boolean),
      });
      closeComposer();
      void showView("mine", true);
    } catch (error) {
      submitError =
        error instanceof Error ? error.message : "Unable to submit this wall";
    } finally {
      submitting = false;
    }
  }

  async function decide(wall: Wall, approved: boolean) {
    if (detailBusy) return;
    detailBusy = true;
    detailError = "";
    try {
      const decided = approved
        ? await approveWall(wall.id, decisionNote.trim())
        : await rejectWall(wall.id, decisionNote.trim());
      openWall = decided;
      decisionNote = "";
      reload();
    } catch (error) {
      detailError =
        error instanceof Error
          ? error.message
          : "Unable to record that decision";
    } finally {
      detailBusy = false;
    }
  }

  async function use(wall: Wall, slot: WallSlot) {
    if (detailBusy) return;
    detailBusy = true;
    detailError = "";
    try {
      await applyWall(wall.id, slot);
      selections =
        slot === "welcome"
          ? { ...selections, welcome: wall.id }
          : { ...selections, login: wall.id };
      // The wallpaper URL does not change when a wall is applied, so the shell has to
      // bust its own cache or the previous image stays on screen.
      onwallapplied?.(slot);
    } catch (error) {
      detailError =
        error instanceof Error ? error.message : "Unable to apply that wall";
    } finally {
      detailBusy = false;
    }
  }

  async function remove(wall: Wall) {
    if (detailBusy) return;
    detailBusy = true;
    detailError = "";
    try {
      await deleteWall(wall.id);
      openWall = null;
      reload();
    } catch (error) {
      detailError =
        error instanceof Error ? error.message : "Unable to delete that wall";
    } finally {
      detailBusy = false;
    }
  }

  async function suggestTags(wall: Wall) {
    if (detailBusy) return;
    detailBusy = true;
    suggestingTags = true;
    detailError = "";
    detailNotice = "";
    try {
      const suggestion = await suggestWallTags(wall.id);
      editTitle = wall.title;
      editDescription = wall.description;
      editTags = suggestion.tags.join(", ");
      editing = true;
      detailNotice = `${suggestion.tags.length} tags suggested by ${suggestion.model}. Review them before saving.`;
    } catch (error) {
      detailError =
        error instanceof Error
          ? error.message
          : "Unable to suggest tags for this wall";
    } finally {
      detailBusy = false;
      suggestingTags = false;
    }
  }

  function toggleTag(tag: string) {
    activeTag = activeTag === tag ? "" : tag;
  }

  function statusLabel(wall: Wall) {
    if (wall.status === "pending") return "Awaiting review";
    if (wall.status === "approved") return "In the collection";
    return "Not accepted";
  }

  function formatBytes(bytes: number) {
    if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  function canManage(wall: Wall) {
    return isAdministrator || wall.user_id === viewerId;
  }

  function startEditing(wall: Wall) {
    editTitle = wall.title;
    editDescription = wall.description;
    editTags = wall.tags.join(", ");
    detailError = "";
    detailNotice = "";
    editing = true;
  }

  function cancelEditing() {
    editing = false;
    detailError = "";
    detailNotice = "";
  }

  /**
   * Saves the descriptive fields. Editing never touches review state, so a wall keeps
   * its status and decision note whether it is pending, approved, or rejected.
   */
  async function saveEdits(wall: Wall) {
    if (detailBusy) return;
    if (!editTitle.trim()) {
      detailError = "Give this wall a title.";
      return;
    }
    detailBusy = true;
    detailError = "";
    try {
      const updated = await updateWall(wall.id, {
        title: editTitle.trim(),
        description: editDescription.trim(),
        tags: editTags
          .split(",")
          .map((tag) => tag.trim().replace(/^#/, ""))
          .filter(Boolean),
      });
      openWall = updated;
      // Keep the already-rendered grid in step without refetching it.
      walls = walls.map((entry) => (entry.id === updated.id ? updated : entry));
      editing = false;
      detailNotice = "";
    } catch (error) {
      detailError =
        error instanceof Error ? error.message : "Unable to save those details";
    } finally {
      detailBusy = false;
    }
  }
</script>

<section class="walls-page product-page" data-od-id="walls-page">
  <header class="walls-header page-header" data-od-id="walls-header">
    <div>
      <TypedHeading text={`$ walls --${activeView}`} odId="walls-heading" />
      <p>
        Wallpapers submitted by people on this instance. Approved images can be
        used as anyone's background.
      </p>
    </div>
    <div class="header-actions">
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={reload}
        disabled={loading || refreshing}
        data-od-id="refresh-walls"
      >
        <RefreshCw
          class={refreshing ? "spinning" : undefined}
          size={15}
          strokeWidth={1.8}
          aria-hidden="true"
        />
        {refreshing ? "Refreshing…" : "Refresh"}
      </button>
      <button
        class="ui-button ui-button--primary"
        type="button"
        onclick={() => (submitOpen = true)}
        data-od-id="submit-wall"
      >
        <ImagePlus size={15} strokeWidth={1.8} aria-hidden="true" />
        Submit a wall
      </button>
    </div>
  </header>

  {#if pageError}
    <p class="wall-page-error" role="alert">{pageError}</p>
  {/if}

  <div class="wall-toolbar" data-od-id="walls-toolbar">
    <nav class="wall-view-tabs" aria-label="Walls views">
      {#each [["collection", "Collection"], ["mine", "Submissions"], ...(isAdministrator ? [["review", "Review"]] : [])] as [view, label] (view)}
        <button
          class={activeView === view ? "active" : undefined}
          type="button"
          aria-pressed={activeView === view}
          onclick={() => void showView(view as WallsView)}
          data-od-id={`walls-${view}-view`}
        >
          {label}
          {#if view === "mine"}<span>{ownSubmissions.length}</span>{/if}
          {#if view === "review"}<span>{reviewQueue.length}</span>{/if}
        </button>
      {/each}
    </nav>

    <label class="wall-search">
      <Search size={14} strokeWidth={1.8} aria-hidden="true" />
      <input
        type="search"
        placeholder="Search walls"
        bind:value={search}
        aria-label="Search walls"
        data-od-id="walls-search"
      />
    </label>
  </div>

  {#if visibleTags.length > 0}
    <div class="wall-tag-filters" data-od-id="walls-tag-filters">
      {#each visibleTags as tag (tag)}
        <button
          class="wall-tag"
          class:active={activeTag === tag}
          type="button"
          aria-pressed={activeTag === tag}
          onclick={() => toggleTag(tag)}
        >
          #{tag}
        </button>
      {/each}
    </div>
  {/if}

  <div
    class="walls-page-body view-swap"
    data-view-phase={viewSwap.phase}
    data-view-direction={viewSwap.direction}
    aria-busy={refreshing}
    {@attach viewSwap.attach}
  >
    {#if loading}
      <p class="wall-empty">Loading walls…</p>
    {:else if walls.length === 0}
      <p class="wall-empty">
        {#if search.trim() || activeTag}
          No walls match that search.
        {:else if activeView === "review"}
          Nothing is waiting for review.
        {:else if activeView === "mine"}
          You have not submitted a wall yet.
        {:else}
          The collection is empty. Submit the first wall.
        {/if}
      </p>
    {:else}
      <ul class="wall-grid" data-od-id="walls-grid">
        {#each walls as wall (wall.id)}
          <li>
            <button
              class="wall-card"
              class:applied={selections.welcome === wall.id}
              type="button"
              onclick={() => {
                openWall = wall;
                decisionNote = "";
                detailError = "";
                detailNotice = "";
                editing = false;
              }}
              data-od-id={`wall-${wall.id}`}
            >
              <img
                src={wallThumbnailUrl(wall.id)}
                alt=""
                loading="lazy"
                style={`aspect-ratio: ${wall.width} / ${wall.height}`}
              />
              <span class="wall-card-body">
                <span class="wall-card-title">{wall.title}</span>
                <span class="wall-card-meta">
                  {wall.submitted_by_name} · {wall.width}×{wall.height}
                </span>
              </span>
              <span class="wall-badges">
                {#if selections.welcome === wall.id}
                  <span class="wall-badge applied">Applied</span>
                {/if}
                {#if selections.login === wall.id}
                  <span class="wall-badge applied">Login</span>
                {/if}
                {#if wall.status !== "approved"}
                  <span class={`wall-badge ${wall.status}`}>
                    {statusLabel(wall)}
                  </span>
                {/if}
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</section>

<dialog
  class="ui-dialog wall-dialog"
  bind:this={submitDialog}
  onclose={closeComposer}
  data-od-id="wall-submit-dialog"
>
  <header class="wall-dialog-header">
    <div>
      <span class="wall-kicker">[ SUBMIT ]</span>
      <h3>Submit a wall</h3>
      <p>An administrator reviews it before it joins the collection.</p>
    </div>
    <button
      class="ui-button ui-button--ghost ui-button--icon"
      type="button"
      aria-label="Close submission"
      onclick={closeComposer}
    >
      <X size={18} strokeWidth={1.9} />
    </button>
  </header>
  <div class="wall-dialog-body">
    {#if filePreview}
      <img class="wall-preview" src={filePreview} alt="Selected wall preview" />
    {/if}
    <div class="wall-field">
      <span>Image</span>
      <div class="wall-file-row">
        <label class="ui-button ui-button--secondary wall-file-picker">
          {file ? "Replace image" : "Choose image"}
          <input
            type="file"
            accept={acceptedTypes.join(",")}
            onchange={selectFile}
            data-od-id="choose-wall-image"
          />
        </label>
        <span class="wall-file-name" class:selected={file}>
          {file
            ? `${file.name} · ${formatBytes(file.size)}`
            : "No image selected"}
        </span>
      </div>
      <small>JPEG, PNG, WebP, or AVIF. Up to 30 MB.</small>
    </div>
    <label class="wall-field">
      <span>Title</span>
      <input type="text" maxlength="120" bind:value={title} />
    </label>
    <label class="wall-field">
      <span>Description</span>
      <textarea rows="3" maxlength="500" bind:value={description}></textarea>
    </label>
    <label class="wall-field">
      <span>Tags</span>
      <input
        type="text"
        placeholder="dark, terminal, landscape"
        bind:value={tagInput}
      />
      <small>Up to eight, separated by commas.</small>
    </label>
    {#if submitError}
      <p class="wall-error" role="alert">{submitError}</p>
    {/if}
    <div class="wall-dialog-actions">
      <button
        class="ui-button ui-button--ghost"
        type="button"
        onclick={closeComposer}
      >
        Cancel
      </button>
      <button
        class="ui-button ui-button--primary"
        type="button"
        disabled={!file || submitting}
        onclick={send}
      >
        {submitting ? "Submitting…" : "Submit for review"}
      </button>
    </div>
  </div>
</dialog>

<dialog
  class="ui-dialog wall-dialog wall-detail-dialog"
  bind:this={detailDialog}
  onclose={() => {
    openWall = null;
    editing = false;
    detailNotice = "";
  }}
  data-od-id="wall-detail-dialog"
>
  {#if openWall}
    <header class="wall-dialog-header">
      <div>
        <span class="wall-kicker">[ {editing ? "EDIT WALL" : "WALL"} ]</span>
        <h3>{openWall.title}</h3>
        <p>
          {openWall.submitted_by_name} · {openWall.width}×{openWall.height} ·
          {formatBytes(openWall.byte_size)}
        </p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close wall"
        onclick={() => (openWall = null)}
      >
        <X size={18} strokeWidth={1.9} />
      </button>
    </header>
    <div class="wall-dialog-body">
      <img
        class="wall-full"
        src={wallImageUrl(openWall.id)}
        alt={openWall.title}
      />

      {#if editing}
        <label class="wall-field">
          <span>Title</span>
          <input type="text" maxlength="120" bind:value={editTitle} />
        </label>
        <label class="wall-field">
          <span>Description</span>
          <textarea rows="3" maxlength="500" bind:value={editDescription}
          ></textarea>
        </label>
        <label class="wall-field">
          <span>Tags</span>
          <input
            type="text"
            placeholder="dark, terminal, landscape"
            bind:value={editTags}
          />
          <small>Up to eight, separated by commas.</small>
        </label>
      {:else}
        {#if openWall.description}
          <p class="wall-description">{openWall.description}</p>
        {/if}

        {#if openWall.tags.length > 0}
          <div class="wall-tag-filters">
            {#each openWall.tags as tag (tag)}
              <span class="wall-tag">#{tag}</span>
            {/each}
          </div>
        {/if}
      {/if}

      {#if openWall.status !== "approved"}
        <p class={`wall-status ${openWall.status}`}>
          {statusLabel(openWall)}
          {#if openWall.decision_note}
            — {openWall.decision_note}
          {/if}
        </p>
      {/if}

      {#if detailError}
        <p class="wall-error" role="alert">{detailError}</p>
      {/if}
      {#if detailNotice}
        <p class="wall-notice" role="status">{detailNotice}</p>
      {/if}

      {#if isAdministrator && openWall.status === "pending" && !editing}
        <label class="wall-field">
          <span>Decision note</span>
          <input
            type="text"
            maxlength="500"
            placeholder="Optional, shown to the submitter"
            bind:value={decisionNote}
          />
        </label>
      {/if}

      <div class="wall-dialog-actions">
        {#if editing}
          <span class="wall-actions-spacer"></span>
          <button
            class="ui-button ui-button--ghost"
            type="button"
            disabled={detailBusy}
            onclick={cancelEditing}
          >
            Cancel
          </button>
          <button
            class="ui-button ui-button--primary"
            type="button"
            disabled={detailBusy}
            onclick={() => openWall && saveEdits(openWall)}
          >
            {detailBusy ? "Saving…" : "Save details"}
          </button>
        {:else}
          {#if canManage(openWall)}
            <button
              class="ui-button ui-button--danger"
              type="button"
              disabled={detailBusy}
              onclick={() => openWall && remove(openWall)}
            >
              <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
              Delete
            </button>
            <button
              class="ui-button ui-button--secondary"
              type="button"
              disabled={detailBusy}
              onclick={() => openWall && startEditing(openWall)}
              data-od-id="edit-wall"
            >
              <Pencil size={15} strokeWidth={1.8} aria-hidden="true" />
              Edit
            </button>
          {/if}
          {#if isAdministrator}
            <button
              class="ui-button ui-button--secondary"
              type="button"
              disabled={detailBusy}
              onclick={() => openWall && suggestTags(openWall)}
              data-od-id="suggest-wall-tags"
            >
              <Tags size={15} strokeWidth={1.8} aria-hidden="true" />
              {suggestingTags ? "Analyzing…" : "Suggest tags"}
            </button>
          {/if}
          <span class="wall-actions-spacer"></span>
          {#if isAdministrator && openWall.status === "pending"}
          <button
            class="ui-button ui-button--secondary"
            type="button"
            disabled={detailBusy}
            onclick={() => openWall && decide(openWall, false)}
          >
            <X size={15} strokeWidth={1.8} aria-hidden="true" />
            Reject
          </button>
          <button
            class="ui-button ui-button--primary"
            type="button"
            disabled={detailBusy}
            onclick={() => openWall && decide(openWall, true)}
          >
            <Check size={15} strokeWidth={1.8} aria-hidden="true" />
            Approve
          </button>
        {:else if openWall.status === "approved"}
          {#if isAdministrator}
            <button
              class="ui-button ui-button--secondary"
              type="button"
              disabled={detailBusy || selections.login === openWall.id}
              onclick={() => openWall && use(openWall, "login")}
            >
              <Wallpaper size={15} strokeWidth={1.8} aria-hidden="true" />
              {selections.login === openWall.id
                ? "Login screen"
                : "Set as login screen"}
            </button>
          {/if}
          <button
            class="ui-button ui-button--primary"
            type="button"
            disabled={detailBusy || selections.welcome === openWall.id}
            onclick={() => openWall && use(openWall, "welcome")}
          >
            <Wallpaper size={15} strokeWidth={1.8} aria-hidden="true" />
              {selections.welcome === openWall.id
                ? "Current background"
                : "Set as my background"}
            </button>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</dialog>

<style>
  .walls-page {
    display: grid;
    align-content: start;
    gap: 18px;
    min-height: 100%;
  }
  .walls-header {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  .walls-header p {
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

  /* Tabs and search are page furniture, not view content: they sit outside the
     swapping body so moving between views never shifts or reloads them. */
  .wall-toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .wall-view-tabs {
    min-width: 0;
    flex: 1 1 auto;
    display: flex;
    gap: 6px;
    overflow-x: auto;
  }
  .wall-view-tabs button {
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
  .wall-view-tabs button:hover,
  .wall-view-tabs button.active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .wall-view-tabs span {
    color: inherit;
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }

  .walls-page-body {
    display: grid;
    align-content: start;
    gap: 18px;
  }

  .wall-search {
    flex: 0 1 320px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 0 12px;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--muted);
  }
  .wall-search:focus-within {
    border-color: var(--fg);
    color: var(--fg);
  }
  .wall-search input {
    width: 100%;
    min-width: 0;
    border: none;
    background: none;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .wall-search input:focus {
    outline: none;
  }
  .wall-tag-filters {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .wall-tag {
    min-height: 30px;
    padding: 4px 10px;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.02em;
  }
  button.wall-tag {
    min-height: 44px;
  }
  button.wall-tag:hover,
  button.wall-tag.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .wall-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(232px, 1fr));
    gap: 14px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .wall-card {
    position: relative;
    display: grid;
    width: 100%;
    padding: 0;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--fg);
    text-align: left;
  }
  .wall-card:hover,
  .wall-card:focus-visible {
    border-color: var(--accent);
  }
  .wall-card.applied {
    border-color: var(--accent);
  }
  .wall-card img {
    display: block;
    width: 100%;
    max-height: 220px;
    object-fit: cover;
    background: color-mix(in oklch, var(--fg) 8%, transparent);
  }
  .wall-card-body {
    display: grid;
    gap: 3px;
    padding: 10px 12px 12px;
    min-width: 0;
  }
  .wall-card-title {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .wall-card-meta {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }
  .wall-badges {
    position: absolute;
    top: 8px;
    left: 8px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .wall-badge {
    padding: 3px 7px;
    border: 1px solid currentColor;
    background: color-mix(in oklch, var(--bg) 76%, transparent);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .wall-badge.applied {
    color: var(--accent);
  }
  .wall-badge.pending {
    color: oklch(80% 0.13 78);
  }
  .wall-badge.rejected {
    color: var(--danger);
  }

  .wall-empty,
  .wall-page-error {
    margin: 0;
    padding: 14px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 13px;
    line-height: 1.6;
  }
  .wall-page-error,
  .wall-error {
    color: var(--danger);
  }
  .wall-error {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .wall-notice {
    margin: 0;
    padding: 10px 12px;
    border: 1px solid var(--border);
    color: var(--fg);
    background: var(--fg-soft);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.5;
  }

  .wall-dialog {
    width: min(680px, calc(100vw - 24px));
  }
  .wall-detail-dialog {
    width: min(860px, calc(100vw - 24px));
  }
  .wall-dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 76px;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }
  .wall-dialog-header h3 {
    margin: 4px 0 2px;
    font-family: var(--font-mono);
    font-size: 18px;
    font-weight: 550;
  }
  .wall-dialog-header p {
    margin: 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
  }
  .wall-kicker {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    color: var(--muted);
  }
  .wall-dialog-body {
    display: grid;
    gap: 14px;
    padding: 18px;
  }
  .wall-preview,
  .wall-full {
    display: block;
    width: 100%;
    max-height: 46vh;
    border: 1px solid var(--border);
    object-fit: contain;
    background: color-mix(in oklch, var(--fg) 8%, transparent);
  }
  .wall-description {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.6;
  }
  .wall-status {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .wall-status.pending {
    color: oklch(80% 0.13 78);
  }
  .wall-status.rejected {
    color: var(--danger);
  }

  .wall-field {
    display: grid;
    gap: 6px;
  }
  .wall-field > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }
  .wall-field small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .wall-field input[type="text"],
  .wall-field textarea {
    min-height: 44px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .wall-field textarea {
    resize: vertical;
  }

  .wall-file-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .wall-file-picker {
    flex: 0 0 auto;
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    padding-inline: 12px;
    white-space: nowrap;
    cursor: pointer;
  }
  .wall-file-picker input[type="file"] {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .wall-file-picker:focus-within {
    outline: 3px solid color-mix(in oklch, var(--accent) 55%, var(--surface));
    outline-offset: 3px;
  }
  .wall-file-name {
    min-width: 0;
    flex: 1 1 180px;
    overflow: hidden;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .wall-file-name.selected {
    color: var(--fg);
  }

  .wall-dialog-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }
  .wall-actions-spacer {
    flex: 1 1 auto;
  }

  @media (max-width: 720px) {
    .walls-header {
      align-items: start;
    }
    .wall-grid {
      grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    }
  }
</style>

<script lang="ts">
  import Check from "lucide-svelte/icons/check";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import Inbox from "lucide-svelte/icons/inbox";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Settings2 from "lucide-svelte/icons/settings-2";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import {
    createRssSubscription,
    deleteRssSubscription,
    fetchRssReader,
    pruneRssItems,
    refreshRssSubscription,
    setRssItemRead,
    updateRssSubscription,
    type RssReaderItem,
    type RssReaderResponse,
    type RssRetentionMode,
    type RssSubscription,
  } from "$lib/api";

  let reader = $state.raw<RssReaderResponse>({ subscriptions: [], items: [] });
  let loading = $state(true);
  let pageError = $state("");
  let query = $state("");
  let categoryFilter = $state("all");
  let sourceFilter = $state("all");
  let unreadOnly = $state(false);
  let busySubscriptionId = $state("");
  let subscriptionDialog = $state<HTMLDialogElement>();
  let subscriptionUrlInput = $state<HTMLInputElement>();
  let pruneDialog = $state<HTMLDialogElement>();
  let itemDialog = $state<HTMLDialogElement>();
  let selectedItemId = $state<string | null>(null);
  let editingSubscriptionId = $state<string | null>(null);
  let feedUrl = $state("");
  let feedCategory = $state("General");
  let retentionEnabled = $state(false);
  let retentionDays = $state(30);
  let retentionMode = $state<RssRetentionMode>("read");
  let subscriptionError = $state("");
  let savingSubscription = $state(false);
  let confirmingDelete = $state(false);
  let pruneDays = $state(90);
  let pruneMode = $state<RssRetentionMode>("read");
  let pruneError = $state("");
  let pruning = $state(false);

  let categories = $derived.by(() =>
    [...new Set(reader.subscriptions.map((item) => item.category))].sort(
      (left, right) => left.localeCompare(right),
    ),
  );
  let unreadCount = $derived(
    reader.items.filter((item) => item.read_at === null).length,
  );
  let filteredItems = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return reader.items.filter((item) => {
      if (categoryFilter !== "all" && item.category !== categoryFilter)
        return false;
      if (sourceFilter !== "all" && item.subscription_id !== sourceFilter)
        return false;
      if (unreadOnly && item.read_at !== null) return false;
      if (!needle) return true;
      return [
        item.title,
        item.summary,
        item.source,
        item.category,
        item.base_url,
        item.url,
      ].some((value) => value.toLowerCase().includes(needle));
    });
  });
  let editingSubscription = $derived(
    reader.subscriptions.find((item) => item.id === editingSubscriptionId) ??
      null,
  );
  let selectedItem = $derived(
    reader.items.find((item) => item.id === selectedItemId) ?? null,
  );
  let selectedItemContent = $derived(
    selectedItem ? plainText(selectedItem.summary) : "",
  );

  onMount(() => {
    void loadReader();
  });

  async function loadReader() {
    loading = true;
    pageError = "";
    try {
      reader = await fetchRssReader();
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to load RSS feeds";
    } finally {
      loading = false;
    }
  }

  function captureSubscriptionDialog(node: HTMLDialogElement) {
    subscriptionDialog = node;
    return () => {
      subscriptionDialog = undefined;
    };
  }

  function captureSubscriptionUrlInput(node: HTMLInputElement) {
    subscriptionUrlInput = node;
    return () => {
      subscriptionUrlInput = undefined;
    };
  }

  function capturePruneDialog(node: HTMLDialogElement) {
    pruneDialog = node;
    return () => {
      pruneDialog = undefined;
    };
  }

  function captureItemDialog(node: HTMLDialogElement) {
    itemDialog = node;
    return () => {
      itemDialog = undefined;
    };
  }

  async function openAddFeed() {
    editingSubscriptionId = null;
    feedUrl = "";
    feedCategory = "General";
    retentionEnabled = false;
    retentionDays = 30;
    retentionMode = "read";
    subscriptionError = "";
    confirmingDelete = false;
    subscriptionDialog?.showModal();
    await tick();
    subscriptionUrlInput?.focus();
  }

  function openEditFeed(subscription: RssSubscription) {
    editingSubscriptionId = subscription.id;
    feedUrl = subscription.url;
    feedCategory = subscription.category;
    retentionEnabled = subscription.auto_delete_days !== null;
    retentionDays = subscription.auto_delete_days ?? 30;
    retentionMode = subscription.auto_delete_mode;
    subscriptionError = "";
    confirmingDelete = false;
    subscriptionDialog?.showModal();
  }

  function closeSubscriptionDialog() {
    if (!savingSubscription) subscriptionDialog?.close();
  }

  async function saveSubscription(event: SubmitEvent) {
    event.preventDefault();
    if (savingSubscription) return;
    savingSubscription = true;
    subscriptionError = "";
    try {
      const settings = {
        category: feedCategory.trim(),
        auto_delete_days: retentionEnabled ? retentionDays : null,
        auto_delete_mode: retentionMode,
      };
      reader = editingSubscriptionId
        ? await updateRssSubscription(editingSubscriptionId, settings)
        : await createRssSubscription({ url: feedUrl.trim(), ...settings });
      subscriptionDialog?.close();
    } catch (reason: unknown) {
      subscriptionError =
        reason instanceof Error ? reason.message : "Unable to save RSS feed";
    } finally {
      savingSubscription = false;
    }
  }

  async function removeSubscription() {
    if (!editingSubscriptionId || savingSubscription) return;
    if (!confirmingDelete) {
      confirmingDelete = true;
      return;
    }
    savingSubscription = true;
    subscriptionError = "";
    try {
      await deleteRssSubscription(editingSubscriptionId);
      if (sourceFilter === editingSubscriptionId) sourceFilter = "all";
      reader = await fetchRssReader();
      subscriptionDialog?.close();
    } catch (reason: unknown) {
      subscriptionError =
        reason instanceof Error ? reason.message : "Unable to remove RSS feed";
    } finally {
      savingSubscription = false;
    }
  }

  async function refreshFeed(subscription: RssSubscription) {
    if (busySubscriptionId) return;
    busySubscriptionId = subscription.id;
    pageError = "";
    try {
      reader = await refreshRssSubscription(subscription.id);
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to refresh RSS feed";
      reader = await fetchRssReader().catch(() => reader);
    } finally {
      busySubscriptionId = "";
    }
  }

  async function toggleRead(item: RssReaderItem) {
    const nextRead = item.read_at === null;
    const previous = reader;
    reader = {
      ...reader,
      items: reader.items.map((candidate) =>
        candidate.id === item.id
          ? { ...candidate, read_at: nextRead ? new Date().toISOString() : null }
          : candidate,
      ),
    };
    try {
      const updated = await setRssItemRead(item.id, nextRead);
      reader = {
        ...reader,
        items: reader.items.map((candidate) =>
          candidate.id === updated.id ? updated : candidate,
        ),
      };
    } catch (reason: unknown) {
      reader = previous;
      pageError = reason instanceof Error ? reason.message : "Unable to update item";
    }
  }

  function openItemDetail(item: RssReaderItem) {
    selectedItemId = item.id;
    itemDialog?.showModal();
    if (item.read_at === null) void toggleRead(item);
  }

  function closeItemDialog() {
    itemDialog?.close();
  }

  function openArticle(item: RssReaderItem) {
    if (!item.url) return;
    window.open(item.url, "_blank", "noopener,noreferrer");
    if (item.read_at === null) void toggleRead(item);
  }

  function openPrune() {
    pruneDays = 90;
    pruneMode = "read";
    pruneError = "";
    pruneDialog?.showModal();
  }

  async function pruneItems(event: SubmitEvent) {
    event.preventDefault();
    if (pruning) return;
    pruning = true;
    pruneError = "";
    try {
      const result = await pruneRssItems(pruneDays, pruneMode);
      reader = await fetchRssReader();
      pruneDialog?.close();
      pageError = result.deleted === 0 ? "No matching items were old enough to prune." : "";
    } catch (reason: unknown) {
      pruneError = reason instanceof Error ? reason.message : "Unable to prune RSS items";
    } finally {
      pruning = false;
    }
  }

  function itemDate(value: string) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "Date unavailable";
    return new Intl.DateTimeFormat("en", {
      month: "short",
      day: "numeric",
      year: date.getFullYear() === new Date().getFullYear() ? undefined : "numeric",
    }).format(date);
  }

  function itemTimestamp(value: string | null) {
    if (!value) return "Not yet";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "Unavailable";
    return new Intl.DateTimeFormat("en", {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(date);
  }

  function plainText(value: string) {
    return value
      .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, " ")
      .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, " ")
      .replace(/<[^>]+>/g, " ")
      .replace(/&nbsp;|&#160;/gi, " ")
      .replace(/&amp;/gi, "&")
      .replace(/&lt;/gi, "<")
      .replace(/&gt;/gi, ">")
      .replace(/&quot;/gi, '"')
      .replace(/&#39;|&apos;/gi, "'")
      .replace(/&#x([0-9a-f]+);/gi, (_, code: string) =>
        safeCodePoint(Number.parseInt(code, 16)),
      )
      .replace(/&#([0-9]+);/g, (_, code: string) =>
        safeCodePoint(Number.parseInt(code, 10)),
      )
      .replace(/\s+/g, " ")
      .trim();
  }

  function safeCodePoint(value: number) {
    return Number.isFinite(value) && value > 0 && value <= 0x10ffff
      ? String.fromCodePoint(value)
      : "";
  }

  function hostLabel(baseUrl: string) {
    try {
      return new URL(baseUrl).host;
    } catch {
      return baseUrl;
    }
  }
</script>

<section class="rss-reader product-page" data-od-id="rss-page">
  <header class="rss-reader-header page-header">
    <div>
      <h2>$ rss --stream</h2>
      <p>{unreadCount} unread across {reader.subscriptions.length} sources</p>
    </div>
    <div class="rss-header-actions">
      <button class="ui-button ui-button--secondary rss-secondary-button" type="button" onclick={openPrune}>
        <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
        Prune
      </button>
      <button class="ui-button ui-button--primary rss-primary-button" type="button" onclick={openAddFeed}>
        <Plus size={16} strokeWidth={2} aria-hidden="true" />
        Add feed
      </button>
    </div>
  </header>

  <div class="rss-filter-bar" data-od-id="rss-filters">
    <label class="rss-search">
      <Search size={16} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Filter by source URL or article text</span>
      <input
        type="search"
        bind:value={query}
        placeholder="Filter by base URL or any text…"
        data-od-id="rss-text-filter"
      />
    </label>
    <label>
      <span class="sr-only">Filter by source feed</span>
      <select bind:value={sourceFilter} data-od-id="rss-source-filter">
        <option value="all">All sources</option>
        {#each reader.subscriptions as subscription (subscription.id)}
          <option value={subscription.id}>{subscription.title}</option>
        {/each}
      </select>
    </label>
    <label>
      <span class="sr-only">Filter by category</span>
      <select bind:value={categoryFilter} data-od-id="rss-category-filter">
        <option value="all">All categories</option>
        {#each categories as category (category)}
          <option value={category}>{category}</option>
        {/each}
      </select>
    </label>
    <button
      class={["rss-unread-toggle", unreadOnly && "is-active"]}
      type="button"
      aria-pressed={unreadOnly}
      onclick={() => (unreadOnly = !unreadOnly)}
    >
      Unread only
    </button>
  </div>

  {#if pageError}
    <div class="rss-page-message" role="status">
      <span>{pageError}</span>
      <button type="button" onclick={() => (pageError = "")}>Dismiss</button>
    </div>
  {/if}

  <div class="rss-reader-layout">
    <main class="rss-item-list" aria-label="RSS items">
      {#if loading}
        <div class="rss-empty" role="status">
          <RefreshCw class="rss-loading-icon" size={28} strokeWidth={1.5} aria-hidden="true" />
          <strong>Loading reader…</strong>
        </div>
      {:else}
        {#each filteredItems as item (item.id)}
          <article class={["rss-item", item.read_at && "is-read"]}>
            <button
              class="rss-item-open"
              type="button"
              onclick={() => openItemDetail(item)}
              aria-label={`Open details for ${item.title}`}
              data-od-id={`rss-item-${item.id}`}
            >
              <span class="rss-unread-dot" aria-label={item.read_at ? "Read" : "Unread"}></span>
              <span class="rss-item-copy">
                <span class="rss-item-meta">
                  <b>{item.source}</b>
                  <span>{item.category}</span>
                  <time datetime={item.published_at}>{itemDate(item.published_at)}</time>
                </span>
                <strong>{item.title}</strong>
                {#if item.summary}<p>{plainText(item.summary)}</p>{/if}
              </span>
            </button>
            <div class="rss-item-actions">
              {#if item.url}
                <button
                  class="rss-item-action"
                  type="button"
                  aria-label={`Open ${item.title} in a new tab`}
                  title="Open original article"
                  onclick={() => openArticle(item)}
                  data-od-id={`rss-open-article-${item.id}`}
                >
                  <ExternalLink size={16} strokeWidth={1.8} aria-hidden="true" />
                </button>
              {/if}
              <button
                class={["rss-item-action", item.read_at && "is-active"]}
                type="button"
                aria-label={item.read_at ? `Mark ${item.title} unread` : `Mark ${item.title} read`}
                title={item.read_at ? "Mark unread" : "Mark read"}
                onclick={() => toggleRead(item)}
              >
                <Check size={16} strokeWidth={2} aria-hidden="true" />
              </button>
            </div>
          </article>
        {:else}
          <div class="rss-empty">
            <Inbox size={30} strokeWidth={1.4} aria-hidden="true" />
            <strong>{reader.subscriptions.length ? "No items match this view" : "Your reader is empty"}</strong>
            <p>
              {reader.subscriptions.length
                ? "Change the text, source, or category filter."
                : "Subscribe to an RSS or Atom URL to start reading."}
            </p>
            {#if reader.subscriptions.length === 0}
              <button class="ui-button ui-button--secondary rss-secondary-button" type="button" onclick={openAddFeed}>Add your first feed</button>
            {/if}
          </div>
        {/each}
      {/if}
    </main>

    <aside class="rss-source-panel" data-od-id="rss-source-directory">
      <div class="rss-source-heading">
        <div>
          <span>[ SOURCES ]</span>
          <strong>{reader.subscriptions.length}</strong>
        </div>
        {#if sourceFilter !== "all"}
          <button type="button" onclick={() => (sourceFilter = "all")}>Show all</button>
        {/if}
      </div>
      <div class="rss-source-list">
        {#each reader.subscriptions as subscription (subscription.id)}
          <article class={["rss-source", sourceFilter === subscription.id && "is-active"]}>
            <button
              class="rss-source-select"
              type="button"
              aria-pressed={sourceFilter === subscription.id}
              onclick={() =>
                (sourceFilter = sourceFilter === subscription.id ? "all" : subscription.id)}
            >
              <span><strong>{subscription.title}</strong><small>{hostLabel(subscription.base_url)}</small></span>
              <em>{subscription.category}</em>
            </button>
            {#if subscription.last_error}
              <p class="rss-source-error">Refresh failed · {subscription.last_error}</p>
            {/if}
            <div class="rss-source-actions">
              <button
                type="button"
                disabled={busySubscriptionId !== ""}
                onclick={() => refreshFeed(subscription)}
              >
                <RefreshCw
                  class={busySubscriptionId === subscription.id ? "spinning" : undefined}
                  size={14}
                  strokeWidth={1.8}
                  aria-hidden="true"
                />
                Refresh
              </button>
              <button type="button" onclick={() => openEditFeed(subscription)}>
                <Settings2 size={14} strokeWidth={1.8} aria-hidden="true" />
                Manage
              </button>
            </div>
          </article>
        {:else}
          <p class="rss-source-empty">No subscriptions yet.</p>
        {/each}
      </div>
    </aside>
  </div>

  <dialog
    class="rss-dialog rss-item-dialog"
    {@attach captureItemDialog}
    onclose={() => (selectedItemId = null)}
    onclick={(event) => event.target === itemDialog && closeItemDialog()}
    data-od-id="rss-item-detail-dialog"
  >
    {#if selectedItem}
      <header>
        <div>
          <span>[ READER.ITEM ]</span>
          <h2>{selectedItem.title}</h2>
        </div>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          aria-label="Close article details"
          onclick={closeItemDialog}
        >
          <X size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </header>
      <div class="rss-detail-body">
        <dl class="rss-detail-meta">
          <div><dt>Source</dt><dd>{selectedItem.source}</dd></div>
          <div><dt>Category</dt><dd>{selectedItem.category}</dd></div>
          <div><dt>Status</dt><dd>{selectedItem.read_at ? "Read" : "Unread"}</dd></div>
          <div><dt>Published</dt><dd>{itemTimestamp(selectedItem.published_at)}</dd></div>
          <div><dt>Fetched</dt><dd>{itemTimestamp(selectedItem.fetched_at)}</dd></div>
          <div><dt>Host</dt><dd>{hostLabel(selectedItem.base_url)}</dd></div>
        </dl>
        <section class="rss-detail-content" aria-labelledby="rss-detail-content-title">
          <div>
            <span>[ CONTENT ]</span>
            <h3 id="rss-detail-content-title">From the feed</h3>
          </div>
          {#if selectedItemContent}
            <p>{selectedItemContent}</p>
          {:else}
            <p class="rss-detail-empty">
              This source did not include article content in its feed.
            </p>
          {/if}
        </section>
        <p class="rss-detail-destination">{selectedItem.url || selectedItem.base_url}</p>
      </div>
      <footer class="rss-detail-actions">
        <button
          class="ui-button ui-button--secondary rss-secondary-button"
          type="button"
          onclick={() => toggleRead(selectedItem)}
        >
          <Check size={15} strokeWidth={2} aria-hidden="true" />
          Mark {selectedItem.read_at ? "unread" : "read"}
        </button>
        {#if selectedItem.url}
          <button
            class="ui-button ui-button--primary rss-primary-button"
            type="button"
            onclick={() => openArticle(selectedItem)}
          >
            Open original
            <ExternalLink size={15} strokeWidth={1.8} aria-hidden="true" />
          </button>
        {/if}
      </footer>
    {/if}
  </dialog>

  <dialog
    class="rss-dialog"
    {@attach captureSubscriptionDialog}
    onclick={(event) => event.target === subscriptionDialog && closeSubscriptionDialog()}
    data-od-id="rss-subscription-dialog"
  >
    <header>
      <div>
        <span>[ RSS.SOURCE ]</span>
        <h2>{editingSubscription ? "Manage feed" : "Add feed"}</h2>
      </div>
      <button class="ui-button ui-button--ghost ui-button--icon" type="button" aria-label="Close feed settings" onclick={closeSubscriptionDialog}>
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </header>
    <form onsubmit={saveSubscription}>
      <label for="rss-feed-url">Feed URL</label>
      <input
        id="rss-feed-url"
        type="url"
        bind:value={feedUrl}
        {@attach captureSubscriptionUrlInput}
        placeholder="https://example.com/feed.xml"
        maxlength="2048"
        disabled={editingSubscriptionId !== null}
        required
      />
      <small>Public HTTPS RSS and Atom feeds are supported.</small>

      <label for="rss-feed-category">Category</label>
      <input
        id="rss-feed-category"
        list="rss-category-options"
        bind:value={feedCategory}
        maxlength="40"
        placeholder="Technology"
        required
      />
      <datalist id="rss-category-options">
        {#each categories as category (category)}<option value={category}></option>{/each}
      </datalist>

      <label class="rss-check-row">
        <input type="checkbox" bind:checked={retentionEnabled} />
        <span><strong>Auto-delete old items</strong><small>Applied whenever the reader loads or this feed refreshes.</small></span>
      </label>

      {#if retentionEnabled}
        <div class="rss-retention-grid">
          <label for="rss-retention-days">After</label>
          <input id="rss-retention-days" type="number" bind:value={retentionDays} min="1" max="3650" required />
          <span>days</span>
        </div>
        <fieldset>
          <legend>Delete scope</legend>
          <label><input type="radio" bind:group={retentionMode} value="read" /> Only items I have read</label>
          <label><input type="radio" bind:group={retentionMode} value="all" /> Read and unread items</label>
        </fieldset>
      {/if}

      {#if subscriptionError}<p class="rss-form-error" role="alert">{subscriptionError}</p>{/if}

      <footer>
        {#if editingSubscription}
          <button class="ui-button ui-button--danger rss-danger-button" type="button" disabled={savingSubscription} onclick={removeSubscription}>
            <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
            {confirmingDelete ? "Confirm remove" : "Remove feed"}
          </button>
        {/if}
        <button class="ui-button ui-button--secondary rss-secondary-button" type="button" onclick={closeSubscriptionDialog}>Cancel</button>
        <button class="ui-button ui-button--primary rss-primary-button" type="submit" disabled={savingSubscription}>
          {savingSubscription ? (editingSubscription ? "Saving…" : "Fetching…") : editingSubscription ? "Save settings" : "Subscribe"}
        </button>
      </footer>
    </form>
  </dialog>

  <dialog
    class="rss-dialog rss-prune-dialog"
    {@attach capturePruneDialog}
    onclick={(event) => event.target === pruneDialog && pruneDialog.close()}
    data-od-id="rss-prune-dialog"
  >
    <header>
      <div><span>[ READER.PRUNE ]</span><h2>Trim old items</h2></div>
      <button class="ui-button ui-button--ghost ui-button--icon" type="button" aria-label="Close prune dialog" onclick={() => pruneDialog?.close()}>
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </header>
    <form onsubmit={pruneItems}>
      <p class="rss-prune-copy">Remove items older than a fixed age across every subscription. Feed-specific auto-delete settings are unchanged.</p>
      <div class="rss-retention-grid">
        <label for="rss-prune-days">Older than</label>
        <input id="rss-prune-days" type="number" bind:value={pruneDays} min="1" max="3650" required />
        <span>days</span>
      </div>
      <fieldset>
        <legend>Items to remove</legend>
        <label><input type="radio" bind:group={pruneMode} value="read" /> Only read items</label>
        <label><input type="radio" bind:group={pruneMode} value="all" /> Read and unread items</label>
      </fieldset>
      {#if pruneError}<p class="rss-form-error" role="alert">{pruneError}</p>{/if}
      <footer>
        <button class="ui-button ui-button--secondary rss-secondary-button" type="button" onclick={() => pruneDialog?.close()}>Cancel</button>
        <button class="ui-button ui-button--danger rss-danger-button" type="submit" disabled={pruning}>{pruning ? "Pruning…" : "Prune items"}</button>
      </footer>
    </form>
  </dialog>
</section>

<style>
  .rss-reader { display: grid; gap: 18px; padding: clamp(24px, 3vw, 42px); min-width: 0; }
  .rss-reader-header { display: flex; align-items: end; justify-content: space-between; gap: 24px; padding-bottom: 18px; border-bottom: 1px solid var(--border); }
  .rss-dialog header span, .rss-source-heading span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .09em; }
  .rss-reader-header h2 { margin-top: 8px; font-family: var(--font-mono); font-size: clamp(26px, 3vw, 42px); font-weight: 540; letter-spacing: -.04em; line-height: 1.05; }
  .rss-reader-header p { margin-top: 8px; color: var(--muted); font-family: var(--font-mono); font-size: 11px; }
  .rss-header-actions, .rss-source-actions, .rss-dialog footer { display: flex; align-items: center; gap: 8px; }
  button, input, select { font: inherit; }
  button { min-height: 42px; }
  .rss-primary-button, .rss-secondary-button, .rss-danger-button { display: inline-flex; align-items: center; justify-content: center; gap: 7px; padding: 0 14px; border: 1px solid var(--border); border-radius: 7px; font-family: var(--font-mono); font-size: 11px; font-weight: 560; letter-spacing: .02em; }
  .rss-primary-button { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-primary-button:hover { background: transparent; color: var(--fg); }
  .rss-secondary-button { background: var(--page-surface, var(--surface)); color: var(--fg); }
  .rss-secondary-button:hover { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-danger-button { border-color: color-mix(in oklch, var(--fg) 55%, var(--border)); background: transparent; color: var(--fg); }
  .rss-danger-button:hover { background: var(--fg); color: var(--surface); }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--fg); outline-offset: 2px; }
  button:disabled { cursor: wait; opacity: .55; }
  .rss-filter-bar { display: grid; grid-template-columns: minmax(240px, 1fr) minmax(150px, .55fr) minmax(150px, .55fr) auto; gap: 8px; padding: 8px; border: 1px solid var(--border); border-radius: 9px; background: color-mix(in oklch, var(--page-surface, var(--surface)) 86%, transparent); }
  .rss-search { display: flex; align-items: center; gap: 9px; min-width: 0; padding: 0 12px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--muted); }
  .rss-search input, .rss-filter-bar select { width: 100%; min-height: 42px; border: 0; outline: 0; background: transparent; color: var(--fg); font-family: var(--font-mono); font-size: 12px; }
  .rss-filter-bar label:not(.rss-search) { min-width: 170px; padding: 0 10px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); }
  .rss-unread-toggle { padding: 0 13px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); font-family: var(--font-mono); font-size: 11px; }
  .rss-unread-toggle.is-active { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-page-message { display: flex; justify-content: space-between; gap: 16px; padding: 11px 13px; border: 1px solid var(--border); background: var(--page-surface, var(--surface)); color: var(--muted); font-family: var(--font-mono); font-size: 11px; }
  .rss-page-message button { min-height: auto; color: var(--fg); text-decoration: underline; }
  .rss-reader-layout { display: grid; grid-template-columns: minmax(0, 1fr) minmax(240px, 300px); gap: 18px; align-items: start; }
  .rss-item-list, .rss-source-panel { border: 1px solid var(--border); background: var(--page-surface, var(--surface)); }
  .rss-item { position: relative; display: grid; grid-template-columns: minmax(0, 1fr) auto; border-bottom: 1px solid var(--border); transition: background-color 120ms cubic-bezier(.2, 0, 0, 1); }
  .rss-item:last-child { border-bottom: 0; }
  .rss-item:hover, .rss-item:focus-within { background: var(--fg-soft); }
  .rss-item-open { display: grid; grid-template-columns: 8px minmax(0, 1fr); align-items: center; gap: 11px; min-width: 0; min-height: 76px; padding: 10px 12px; text-align: left; }
  .rss-item-copy { display: grid; min-width: 0; gap: 3px; }
  .rss-item-copy > strong { overflow: hidden; color: var(--fg); font-family: var(--font-display); font-size: 15px; font-weight: 600; letter-spacing: -.01em; line-height: 1.3; text-overflow: ellipsis; white-space: nowrap; }
  .rss-item-copy > p { overflow: hidden; color: var(--muted); font-size: 10.5px; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
  .rss-item.is-read .rss-item-copy > strong { color: color-mix(in oklch, var(--fg) 68%, var(--muted)); font-weight: 500; }
  .rss-item-meta { display: flex; align-items: center; gap: 8px; min-width: 0; color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-item-meta b { max-width: 32ch; overflow: hidden; color: var(--fg); font-weight: 560; text-overflow: ellipsis; white-space: nowrap; }
  .rss-item-meta span::before { content: "/"; margin-right: 8px; color: var(--border); }
  .rss-item-meta time { margin-left: auto; font-variant-numeric: tabular-nums; }
  .rss-unread-dot { width: 6px; height: 6px; flex: 0 0 auto; border: 1px solid var(--muted); border-radius: 50%; }
  .rss-item:not(.is-read) .rss-unread-dot { border-color: var(--accent); background: var(--accent); }
  .rss-item-actions { display: flex; align-items: center; gap: 3px; padding: 0 8px; border-left: 1px solid var(--border); }
  .rss-item-action { display: grid; width: 44px; min-height: 44px; place-items: center; border: 1px solid transparent; color: var(--muted); transition: border-color 100ms ease, background-color 100ms ease, color 100ms ease; }
  .rss-item-action:hover { border-color: var(--fg); background: var(--page-surface, var(--surface)); color: var(--fg); }
  .rss-item-action:active { transform: translateY(1px); }
  .rss-item-action.is-active { color: var(--fg); }
  .rss-source-panel { position: sticky; top: 20px; }
  .rss-source-heading { min-height: 66px; display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 13px 15px; border-bottom: 1px solid var(--border); }
  .rss-source-heading > div { display: flex; align-items: baseline; gap: 10px; }
  .rss-source-heading strong { font-family: var(--font-mono); font-size: 24px; font-weight: 520; }
  .rss-source-heading button { min-height: auto; color: var(--fg); font-family: var(--font-mono); font-size: 10px; text-decoration: underline; }
  .rss-source { padding: 14px; border-bottom: 1px solid var(--border); }
  .rss-source:last-child { border-bottom: 0; }
  .rss-source.is-active { background: var(--bg); }
  .rss-source-select { width: 100%; min-height: auto; display: flex; align-items: start; justify-content: space-between; gap: 10px; text-align: left; }
  .rss-source-select > span { min-width: 0; display: grid; gap: 3px; }
  .rss-source-select strong, .rss-source-select small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rss-source-select strong { font-size: 12px; font-weight: 560; }
  .rss-source-select small { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-source-select em { flex: 0 0 auto; color: var(--muted); font-family: var(--font-mono); font-size: 9px; font-style: normal; }
  .rss-source-actions { margin-top: 11px; }
  .rss-source-actions button { min-height: 31px; display: inline-flex; align-items: center; gap: 5px; padding: 0 8px; border: 1px solid var(--border); color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-source-actions button:hover { border-color: var(--fg); color: var(--fg); }
  .rss-source-error { margin-top: 8px; color: var(--fg); font-size: 10px; line-height: 1.4; }
  .rss-source-empty { padding: 20px 15px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
  .rss-empty { min-height: 360px; display: grid; place-items: center; align-content: center; gap: 8px; padding: 30px; color: var(--muted); text-align: center; }
  .rss-empty strong { color: var(--fg); font-family: var(--font-display); font-size: 19px; }
  .rss-empty p { max-width: 42ch; font-size: 12px; }
  .rss-empty button { margin-top: 8px; }
  :global(.rss-loading-icon), :global(.spinning) { animation: rss-spin .8s linear infinite; }
  @keyframes rss-spin { to { transform: rotate(360deg); } }
  .rss-dialog { width: min(600px, calc(100vw - 32px)); max-height: min(780px, calc(100vh - 32px)); margin: auto; padding: 0; overflow: auto; border: 1px solid var(--border); border-radius: 10px; background: var(--page-surface, var(--surface)); color: var(--fg); box-shadow: 0 24px 80px rgba(0, 0, 0, .48); animation: rss-dialog-in 240ms cubic-bezier(.2, 0, 0, 1); }
  .rss-dialog::backdrop { background: rgba(0, 0, 0, .7); backdrop-filter: blur(7px); animation: rss-fade-in 180ms ease-out; }
  .rss-dialog header { min-height: 76px; display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 16px 20px; border-bottom: 1px solid var(--border); }
  .rss-dialog header h2 { margin-top: 5px; font-family: var(--font-display); font-size: 24px; font-weight: 600; letter-spacing: -.02em; }
  .rss-dialog header > button { width: 42px; min-height: 42px; display: grid; place-items: center; border: 1px solid var(--border); border-radius: 7px; }
  .rss-dialog form { display: grid; gap: 10px; padding: 22px; }
  .rss-dialog form > label:not(.rss-check-row), .rss-dialog legend { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .05em; }
  .rss-dialog input[type="url"], .rss-dialog input[list], .rss-dialog input[type="number"] { min-height: 44px; width: 100%; padding: 0 12px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); font-family: var(--font-mono); font-size: 12px; }
  .rss-dialog input:disabled { color: var(--muted); }
  .rss-dialog form > small { margin-top: -4px; color: var(--muted); font-size: 10px; }
  .rss-check-row { display: flex; align-items: start; gap: 10px; margin-top: 10px; padding: 13px; border: 1px solid var(--border); }
  .rss-check-row input { margin-top: 3px; accent-color: var(--fg); }
  .rss-check-row span { display: grid; gap: 3px; }
  .rss-check-row strong { font-size: 12px; font-weight: 560; }
  .rss-check-row small { color: var(--muted); font-size: 10px; }
  .rss-retention-grid { display: grid; grid-template-columns: auto 100px auto; align-items: center; gap: 9px; }
  .rss-retention-grid label, .rss-retention-grid span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
  .rss-dialog fieldset { display: grid; gap: 8px; margin: 2px 0 8px; padding: 12px; border: 1px solid var(--border); }
  .rss-dialog fieldset label { display: flex; align-items: center; gap: 8px; color: var(--fg); font-size: 11px; }
  .rss-dialog fieldset input { accent-color: var(--fg); }
  .rss-dialog footer { justify-content: flex-end; margin-top: 8px; padding-top: 16px; border-top: 1px solid var(--border); }
  .rss-dialog footer .rss-danger-button:first-child { margin-right: auto; }
  .rss-item-dialog { width: min(760px, calc(100vw - 32px)); }
  .rss-item-dialog header { align-items: start; }
  .rss-item-dialog header h2 { max-width: 28ch; line-height: 1.2; text-wrap: balance; }
  .rss-detail-body { display: grid; gap: 20px; padding: 20px 22px 24px; }
  .rss-detail-meta { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); margin: 0; border: 1px solid var(--border); }
  .rss-detail-meta > div { min-width: 0; padding: 10px 12px; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); }
  .rss-detail-meta > div:nth-child(3n) { border-right: 0; }
  .rss-detail-meta > div:nth-last-child(-n + 3) { border-bottom: 0; }
  .rss-detail-meta dt { margin-bottom: 4px; color: var(--muted); font-family: var(--font-mono); font-size: 8px; letter-spacing: .08em; text-transform: uppercase; }
  .rss-detail-meta dd { margin: 0; overflow: hidden; color: var(--fg); font-family: var(--font-mono); font-size: 10px; line-height: 1.4; text-overflow: ellipsis; white-space: nowrap; }
  .rss-detail-content { display: grid; gap: 14px; padding-top: 2px; }
  .rss-detail-content > div { display: flex; align-items: baseline; gap: 10px; padding-bottom: 9px; border-bottom: 1px solid var(--border); }
  .rss-detail-content span { color: var(--muted); font-family: var(--font-mono); font-size: 9px; letter-spacing: .08em; }
  .rss-detail-content h3 { font-family: var(--font-display); font-size: 18px; font-weight: 600; letter-spacing: -.01em; }
  .rss-detail-content p { max-width: 68ch; margin: 0; color: var(--fg); font-size: 13px; line-height: 1.72; white-space: pre-wrap; }
  .rss-detail-content .rss-detail-empty { color: var(--muted); }
  .rss-detail-destination { overflow-wrap: anywhere; color: var(--muted); font-family: var(--font-mono); font-size: 9px; line-height: 1.5; }
  .rss-detail-actions { justify-content: space-between; padding: 14px 22px; border-top: 1px solid var(--border); }
  .rss-form-error { padding: 10px; border: 1px solid color-mix(in oklch, var(--fg) 28%, var(--border)); background: var(--fg-soft); color: var(--fg); font-size: 11px; }
  .rss-prune-copy { color: var(--muted); font-size: 12px; line-height: 1.55; }
  @keyframes rss-dialog-in { from { opacity: 0; transform: translateY(-36px); } to { opacity: 1; transform: translateY(0); } }
  @keyframes rss-fade-in { from { opacity: 0; } to { opacity: 1; } }
  @media (max-width: 920px) {
    .rss-reader { padding: 20px 16px; }
    .rss-reader-header { align-items: start; flex-direction: column; }
    .rss-filter-bar { grid-template-columns: 1fr; }
    .rss-filter-bar label:not(.rss-search) { min-width: 0; }
    .rss-reader-layout { grid-template-columns: 1fr; }
    .rss-source-panel { position: static; order: -1; }
  }
  @media (max-width: 560px) {
    .rss-header-actions { width: 100%; }
    .rss-header-actions button { flex: 1; }
    .rss-item-open { min-height: 82px; padding: 10px; }
    .rss-item-actions { padding: 0 4px; }
    .rss-item-action { width: 42px; min-height: 44px; }
    .rss-item-actions .rss-item-action:last-child:not(:first-child) { display: none; }
    .rss-item-meta span { display: none; }
    .rss-item-meta b { max-width: 20ch; }
    .rss-detail-meta { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .rss-detail-meta > div, .rss-detail-meta > div:nth-child(3n) { border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); }
    .rss-detail-meta > div:nth-child(2n) { border-right: 0; }
    .rss-detail-meta > div:nth-last-child(-n + 2) { border-bottom: 0; }
    .rss-detail-actions { align-items: stretch; flex-direction: column-reverse; }
    .rss-detail-actions button { width: 100%; }
    .rss-dialog footer { flex-wrap: wrap; }
    .rss-dialog footer button { flex: 1; }
    .rss-dialog footer .rss-danger-button:first-child { flex-basis: 100%; margin-right: 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .rss-dialog, .rss-dialog::backdrop { animation: none; }
    :global(.rss-loading-icon), :global(.spinning) { animation: none; }
  }
</style>

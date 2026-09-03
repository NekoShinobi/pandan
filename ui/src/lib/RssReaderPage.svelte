<script lang="ts">
  import Check from "lucide-svelte/icons/check";
  import Bookmark from "lucide-svelte/icons/bookmark";
  import Copy from "lucide-svelte/icons/copy";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import FileCode2 from "lucide-svelte/icons/file-code-2";
  import ImageIcon from "lucide-svelte/icons/image";
  import Inbox from "lucide-svelte/icons/inbox";
  import MessageCircle from "lucide-svelte/icons/message-circle";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Settings2 from "lucide-svelte/icons/settings-2";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    createRssSubscription,
    deleteRssSubscription,
    fetchRssReader,
    fetchRssItemXmlSnippet,
    pruneRssItems,
    refreshRssSubscription,
    rssItemImageUrl,
    setRssItemRead,
    setRssItemSaved,
    updateRssSubscription,
    type RssReaderItem,
    type RssReaderResponse,
    type RssRetentionMode,
    type RssSubscription,
  } from "$lib/api";

  type FeedSourceKind = "feed" | "reddit";
  type RssView = "inbox" | "current" | "read-later";
  type RedditSort = "hot" | "new" | "top" | "rising";
  type RedditTopPeriod = "hour" | "day" | "week" | "month" | "year" | "all";

  const BACKGROUND_SYNC_MS = 5 * 60 * 1000;
  const DEFAULT_RETENTION_DAYS = 7;
  const DEFAULT_CURRENT_ENTRY_LIMIT = 25;
  const MAX_CURRENT_ENTRY_LIMIT = 200;

  let reader = $state.raw<RssReaderResponse>({ subscriptions: [], items: [] });
  let loading = $state(true);
  let pageError = $state("");
  let query = $state("");
  let activeView = $state<RssView>("inbox");
  let categoryFilter = $state("all");
  let sourceFilter = $state("all");
  let unreadOnly = $state(false);
  let busySubscriptionId = $state("");
  let sourcesDialog = $state<HTMLDialogElement>();
  let subscriptionDialog = $state<HTMLDialogElement>();
  let subscriptionUrlInput = $state<HTMLInputElement>();
  let pruneDialog = $state<HTMLDialogElement>();
  let itemDialog = $state<HTMLDialogElement>();
  let itemImageDialog = $state<HTMLDialogElement>();
  let itemContextDialog = $state<HTMLDialogElement>();
  let itemXmlDialog = $state<HTMLDialogElement>();
  let selectedItemId = $state<string | null>(null);
  let imageItemId = $state<string | null>(null);
  let xmlItemId = $state<string | null>(null);
  let contextItem = $state.raw<RssReaderItem | null>(null);
  let contextDialogX = $state(0);
  let contextDialogY = $state(0);
  let imageLoaded = $state(false);
  let imageLoadFailed = $state(false);
  let xmlSnippet = $state("");
  let xmlSnippetError = $state("");
  let xmlSnippetLoading = $state(false);
  let xmlSnippetCopied = $state(false);
  let editingSubscriptionId = $state<string | null>(null);
  let feedSourceKind = $state<FeedSourceKind>("feed");
  let feedUrl = $state("");
  let feedUrlCopied = $state(false);
  let redditSubreddit = $state("");
  let redditSort = $state<RedditSort>("hot");
  let redditTopPeriod = $state<RedditTopPeriod>("day");
  let feedName = $state("");
  let feedCategory = $state("General");
  let retentionEnabled = $state(true);
  let retentionDays = $state(DEFAULT_RETENTION_DAYS);
  let retentionMode = $state<RssRetentionMode>("all");
  let currentEntryLimit = $state(DEFAULT_CURRENT_ENTRY_LIMIT);
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
  let readLaterCount = $derived(
    reader.items.filter((item) => item.saved_at !== null).length,
  );
  let currentCount = $derived(
    reader.items.filter((item) => item.is_current).length,
  );
  let currentSourceCount = $derived(
    reader.subscriptions.filter((item) => item.refresh_generation > 0).length,
  );
  let latestSnapshotAt = $derived.by(() =>
    reader.subscriptions
      .map((item) => item.last_fetched_at)
      .filter((value): value is string => value !== null)
      .sort()
      .at(-1) ?? null,
  );
  let filteredItems = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return reader.items.filter((item) => {
      if (activeView === "read-later" && item.saved_at === null) return false;
      if (activeView === "current" && !item.is_current) return false;
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
        item.comments_url,
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
  let imageItem = $derived(
    reader.items.find((item) => item.id === imageItemId) ?? null,
  );
  let xmlItem = $derived(
    reader.items.find((item) => item.id === xmlItemId) ?? null,
  );
  let selectedItemContent = $derived(
    selectedItem ? plainText(selectedItem.summary) : "",
  );
  let redditFeedPreview = $derived(
    buildRedditFeedUrl(redditSubreddit, redditSort, redditTopPeriod),
  );

  onMount(() => {
    void loadReader();
  });

  // The server refreshes subscriptions in the background, so an open reader polls quietly to
  // pick up entries fetched after the page loaded.
  $effect(() => {
    if (typeof window === "undefined") return;
    let active = true;
    const sync = async () => {
      if (!active || document.visibilityState !== "visible") return;
      if (loading || savingSubscription || pruning || busySubscriptionId) return;
      try {
        const next = await fetchRssReader();
        if (active) reader = next;
      } catch {
        // A background sync stays silent; the next deliberate action surfaces any failure.
      }
    };
    const timer = window.setInterval(() => void sync(), BACKGROUND_SYNC_MS);
    const onVisibilityChange = () => void sync();
    document.addEventListener("visibilitychange", onVisibilityChange);
    return () => {
      active = false;
      window.clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
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

  function captureSourcesDialog(node: HTMLDialogElement) {
    sourcesDialog = node;
    return () => {
      sourcesDialog = undefined;
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

  function captureItemImageDialog(node: HTMLDialogElement) {
    itemImageDialog = node;
    return () => {
      itemImageDialog = undefined;
    };
  }

  function captureItemContextDialog(node: HTMLDialogElement) {
    itemContextDialog = node;
    return () => {
      itemContextDialog = undefined;
    };
  }

  function captureItemXmlDialog(node: HTMLDialogElement) {
    itemXmlDialog = node;
    return () => {
      itemXmlDialog = undefined;
    };
  }

  async function openAddFeed() {
    editingSubscriptionId = null;
    feedSourceKind = "feed";
    feedUrl = "";
    feedUrlCopied = false;
    redditSubreddit = "";
    redditSort = "hot";
    redditTopPeriod = "day";
    feedName = "";
    feedCategory = "General";
    retentionEnabled = true;
    retentionDays = DEFAULT_RETENTION_DAYS;
    retentionMode = "all";
    currentEntryLimit = DEFAULT_CURRENT_ENTRY_LIMIT;
    subscriptionError = "";
    confirmingDelete = false;
    subscriptionDialog?.showModal();
    await tick();
    subscriptionUrlInput?.focus();
  }

  function openEditFeed(subscription: RssSubscription) {
    editingSubscriptionId = subscription.id;
    feedSourceKind = "feed";
    feedUrl = subscription.url;
    feedUrlCopied = false;
    feedName = subscription.custom_name ?? "";
    feedCategory = subscription.category;
    retentionEnabled = subscription.auto_delete_days !== null;
    retentionDays = subscription.auto_delete_days ?? DEFAULT_RETENTION_DAYS;
    retentionMode =
      subscription.auto_delete_days === null
        ? "all"
        : subscription.auto_delete_mode;
    currentEntryLimit = subscription.current_entry_limit;
    subscriptionError = "";
    confirmingDelete = false;
    subscriptionDialog?.showModal();
  }

  function closeSubscriptionDialog() {
    if (!savingSubscription) subscriptionDialog?.close();
  }

  function openSources() {
    sourcesDialog?.showModal();
  }

  function chooseSource(subscriptionId: string) {
    sourceFilter = subscriptionId;
    sourcesDialog?.close();
  }

  function editFeedFromSources(subscription: RssSubscription) {
    sourcesDialog?.close();
    openEditFeed(subscription);
  }

  function pruneFromSources() {
    sourcesDialog?.close();
    openPrune();
  }

  async function copyFeedUrl() {
    const value = feedUrl.trim();
    if (!editingSubscriptionId || !value) return;
    try {
      await navigator.clipboard.writeText(value);
      feedUrlCopied = true;
      subscriptionError = "";
    } catch {
      feedUrlCopied = false;
      subscriptionError = "Unable to copy the feed URL.";
    }
  }

  function selectFeedSource(kind: FeedSourceKind) {
    feedSourceKind = kind;
    subscriptionError = "";
    if (kind === "reddit" && feedCategory === "General") {
      feedCategory = "Reddit";
    }
  }

  async function saveSubscription(event: SubmitEvent) {
    event.preventDefault();
    if (savingSubscription) return;
    const sourceUrl =
      editingSubscriptionId !== null || feedSourceKind === "feed"
        ? feedUrl.trim()
        : redditFeedPreview;
    if (!sourceUrl) {
      subscriptionError = "Enter a subreddit name or Reddit community URL.";
      return;
    }
    savingSubscription = true;
    subscriptionError = "";
    try {
      const settings = {
        custom_name: feedName.trim(),
        category: feedCategory.trim(),
        auto_delete_days: retentionEnabled ? retentionDays : null,
        auto_delete_mode: retentionMode,
        current_entry_limit: currentEntryLimit,
      };
      reader = editingSubscriptionId
        ? await updateRssSubscription(editingSubscriptionId, settings)
        : await createRssSubscription({ url: sourceUrl, ...settings });
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

  async function toggleSaved(item: RssReaderItem) {
    const nextSaved = item.saved_at === null;
    const previous = reader;
    reader = {
      ...reader,
      items: reader.items.map((candidate) =>
        candidate.id === item.id
          ? {
              ...candidate,
              saved_at: nextSaved ? new Date().toISOString() : null,
            }
          : candidate,
      ),
    };
    try {
      const updated = await setRssItemSaved(item.id, nextSaved);
      reader = {
        ...reader,
        items: reader.items.map((candidate) =>
          candidate.id === updated.id ? updated : candidate,
        ),
      };
    } catch (reason: unknown) {
      reader = previous;
      pageError =
        reason instanceof Error ? reason.message : "Unable to update Read Later";
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

  function openItemImage(item: RssReaderItem) {
    imageItemId = item.id;
    imageLoaded = false;
    imageLoadFailed = false;
    itemImageDialog?.showModal();
  }

  function closeItemImage() {
    itemImageDialog?.close();
  }

  function resetItemImage() {
    imageItemId = null;
    imageLoaded = false;
    imageLoadFailed = false;
  }

  function openItemContext(item: RssReaderItem, event: MouseEvent) {
    event.preventDefault();
    if (itemContextDialog?.open) {
      closeItemContext();
      return;
    }
    const menuWidth = 244;
    const menuHeight = 58;
    const inset = 12;
    contextItem = item;
    contextDialogX = Math.max(
      inset,
      Math.min(event.clientX, window.innerWidth - menuWidth - inset),
    );
    contextDialogY = Math.max(
      inset,
      Math.min(event.clientY, window.innerHeight - menuHeight - inset),
    );
    itemContextDialog?.showModal();
  }

  function openItemContextFromKeyboard(
    item: RssReaderItem,
    event: KeyboardEvent,
  ) {
    if (
      event.key !== "ContextMenu" &&
      !(event.shiftKey && event.key === "F10")
    ) {
      return;
    }
    event.preventDefault();
    const trigger = event.currentTarget;
    if (!(trigger instanceof HTMLElement)) return;
    const rect = trigger.getBoundingClientRect();
    openItemContext(
      item,
      new MouseEvent("contextmenu", {
        clientX: rect.left + Math.min(rect.width - 12, 36),
        clientY: rect.top + Math.min(rect.height - 12, 36),
      }),
    );
  }

  function closeItemContext() {
    itemContextDialog?.close();
  }

  function resetItemContext() {
    contextItem = null;
  }

  async function openItemXmlSnippet() {
    if (!contextItem) return;
    const item = contextItem;
    closeItemContext();
    xmlItemId = item.id;
    xmlSnippet = "";
    xmlSnippetError = "";
    xmlSnippetLoading = true;
    xmlSnippetCopied = false;
    await tick();
    itemXmlDialog?.showModal();
    try {
      const response = await fetchRssItemXmlSnippet(item.id);
      if (xmlItemId === item.id) xmlSnippet = response.xml;
    } catch (reason: unknown) {
      if (xmlItemId !== item.id) return;
      xmlSnippetError =
        reason instanceof Error && reason.message.includes("not found")
          ? "No XML snippet is stored for this item yet. Refresh its source and try again."
          : reason instanceof Error
            ? reason.message
            : "Unable to load this item's XML snippet.";
    } finally {
      if (xmlItemId === item.id) xmlSnippetLoading = false;
    }
  }

  function closeItemXmlSnippet() {
    itemXmlDialog?.close();
  }

  function resetItemXmlSnippet() {
    xmlItemId = null;
    xmlSnippet = "";
    xmlSnippetError = "";
    xmlSnippetLoading = false;
    xmlSnippetCopied = false;
  }

  async function copyItemXmlSnippet() {
    if (!xmlSnippet) return;
    try {
      await navigator.clipboard.writeText(xmlSnippet);
      xmlSnippetCopied = true;
    } catch {
      xmlSnippetError = "Unable to copy the XML snippet.";
    }
  }

  function articleDestination(item: RssReaderItem) {
    return item.url || item.comments_url || item.base_url;
  }

  function openArticle(item: RssReaderItem) {
    const destination = articleDestination(item);
    if (!destination) return;
    window.open(destination, "_blank", "noopener,noreferrer");
    if (item.read_at === null) void toggleRead(item);
  }

  function openComments(item: RssReaderItem) {
    if (!item.comments_url) return;
    window.open(item.comments_url, "_blank", "noopener,noreferrer");
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

  function relativeTime(value: string | null) {
    if (!value) return "waiting for first refresh";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "refresh time unavailable";
    const minutes = Math.round((date.getTime() - Date.now()) / 60_000);
    const formatter = new Intl.RelativeTimeFormat("en", { numeric: "auto" });
    if (Math.abs(minutes) < 60) return formatter.format(minutes, "minute");
    const hours = Math.round(minutes / 60);
    if (Math.abs(hours) < 24) return formatter.format(hours, "hour");
    return formatter.format(Math.round(hours / 24), "day");
  }

  function subscriptionCurrentCount(subscriptionId: string) {
    return reader.items.filter(
      (item) => item.subscription_id === subscriptionId && item.is_current,
    ).length;
  }

  function subscriptionUnreadCount(subscriptionId: string) {
    return reader.items.filter(
      (item) => item.subscription_id === subscriptionId && item.read_at === null,
    ).length;
  }

  function subscriptionViewCount(subscriptionId: string) {
    if (activeView === "current") {
      return subscriptionCurrentCount(subscriptionId);
    }
    if (activeView === "read-later") {
      return reader.items.filter(
        (item) => item.subscription_id === subscriptionId && item.saved_at !== null,
      ).length;
    }
    return subscriptionUnreadCount(subscriptionId);
  }

  function subscriptionLabel(subscription: RssSubscription) {
    return subscription.custom_name?.trim() || subscription.title;
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

  function buildRedditFeedUrl(
    value: string,
    sort: RedditSort,
    topPeriod: RedditTopPeriod,
  ) {
    const subreddit = parseSubreddit(value);
    if (!subreddit) return "";
    const url = new URL(`https://www.reddit.com/r/${subreddit}/${sort}.rss`);
    url.searchParams.set("limit", "25");
    if (sort === "top") url.searchParams.set("t", topPeriod);
    return url.toString();
  }

  function parseSubreddit(value: string) {
    const input = value.trim();
    if (!input) return "";
    let candidate = input.replace(/^\/?r\//i, "").split(/[/?#]/, 1)[0];
    if (/^https?:\/\//i.test(input)) {
      try {
        const url = new URL(input);
        if (!/(^|\.)reddit\.com$/i.test(url.hostname)) return "";
        const segments = url.pathname.split("/").filter(Boolean);
        if (segments[0]?.toLowerCase() !== "r") return "";
        candidate = segments[1] ?? "";
      } catch {
        return "";
      }
    }
    return /^[a-z0-9_.-]{1,100}$/i.test(candidate) ? candidate : "";
  }
</script>

<section class="rss-reader product-page" data-od-id="rss-page">
  <header class="rss-reader-header page-header">
    <div>
      <TypedHeading text={`$ rss --${activeView}`} odId="rss-heading" />
      <p>
        {activeView === "inbox"
          ? `${unreadCount} unread across ${reader.subscriptions.length} sources`
          : activeView === "current"
            ? `${currentCount} items in the latest cached snapshots`
            : `${readLaterCount} saved ${readLaterCount === 1 ? "article" : "articles"}`}
      </p>
    </div>
    <div class="rss-header-actions">
      <button
        class="ui-button ui-button--secondary rss-secondary-button rss-sources-trigger"
        type="button"
        onclick={openSources}
        data-od-id="rss-sources-menu-button"
      >
        <Settings2 size={16} strokeWidth={1.8} aria-hidden="true" />
        <span>
          <strong>Edit sources</strong>
          <small>Latest cached {relativeTime(latestSnapshotAt)}</small>
        </span>
      </button>
      <button class="ui-button ui-button--primary rss-primary-button" type="button" onclick={openAddFeed}>
        <Plus size={16} strokeWidth={2} aria-hidden="true" />
        Add Feed
      </button>
    </div>
  </header>

  <nav class="rss-view-tabs" aria-label="RSS reader views" data-od-id="rss-reader-views">
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={activeView === "inbox"}
      onclick={() => (activeView = "inbox")}
      data-od-id="rss-inbox-view"
    >
      Inbox <span>{unreadCount}</span>
    </button>
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={activeView === "current"}
      onclick={() => (activeView = "current")}
      data-od-id="rss-current-view"
    >
      Current <span>{currentCount}</span>
    </button>
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={activeView === "read-later"}
      onclick={() => (activeView = "read-later")}
      data-od-id="rss-read-later-view"
    >
      <Bookmark size={15} strokeWidth={1.8} aria-hidden="true" />
      Read later <span>{readLaterCount}</span>
    </button>
  </nav>

  <div class="rss-filter-bar" data-od-id="rss-filters">
    <label class="rss-search">
      <Search size={16} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Filter by source URL or article text</span>
      <input
        type="search"
        bind:value={query}
        placeholder="Search titles, sources, or URLs…"
        data-od-id="rss-text-filter"
      />
    </label>
    <label class="rss-category-filter">
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

  {#if reader.subscriptions.length > 0}
    <nav class="rss-source-strip" aria-label="Filter by feed" data-od-id="rss-source-strip">
      <button
        class:active={sourceFilter === "all"}
        type="button"
        aria-pressed={sourceFilter === "all"}
        onclick={() => (sourceFilter = "all")}
        data-od-id="rss-source-all"
      >
        All feeds
      </button>
      {#each reader.subscriptions as subscription (subscription.id)}
        <button
          class:active={sourceFilter === subscription.id}
          type="button"
          aria-pressed={sourceFilter === subscription.id}
          onclick={() =>
            (sourceFilter = sourceFilter === subscription.id ? "all" : subscription.id)}
          data-od-id={`rss-source-tab-${subscription.id}`}
        >
          {subscriptionLabel(subscription)}
          <span>{subscriptionViewCount(subscription.id)}</span>
        </button>
      {/each}
    </nav>
  {/if}

  {#if pageError}
    <div class="rss-page-message" role="status">
      <span>{pageError}</span>
      <button type="button" onclick={() => (pageError = "")}>Dismiss</button>
    </div>
  {/if}

  <main class="rss-item-list" aria-label="RSS items">
      {#if loading}
        <div class="rss-empty" role="status">
          <RefreshCw class="rss-loading-icon" size={28} strokeWidth={1.5} aria-hidden="true" />
          <strong>Loading reader…</strong>
        </div>
      {:else}
        {#each filteredItems as item (item.id)}
          <article
            class={["rss-item", item.read_at && "is-read"]}
            oncontextmenu={(event) => openItemContext(item, event)}
          >
            <button
              class="rss-item-open"
              type="button"
              onclick={() => openItemDetail(item)}
              onkeydown={(event) => openItemContextFromKeyboard(item, event)}
              aria-label={`Open details for ${item.title}`}
              data-od-id={`rss-item-${item.id}`}
            >
              <span class="rss-unread-dot" aria-label={item.read_at ? "Read" : "Unread"}></span>
              <span class="rss-item-copy">
                <span class="rss-item-meta">
                  <b>{item.source}</b>
                  <span>{item.category}</span>
                  {#if activeView === "read-later" && !item.is_current}
                    <span class="rss-history-state">No longer current</span>
                  {/if}
                  <time datetime={item.published_at}>{itemDate(item.published_at)}</time>
                </span>
                <strong>{item.title}</strong>
                {#if item.summary}<p>{plainText(item.summary)}</p>{/if}
              </span>
            </button>
            <div class="rss-item-actions">
              <button
                class={["rss-item-action", item.saved_at && "is-active"]}
                type="button"
                aria-label={item.saved_at
                  ? `Remove ${item.title} from Read Later`
                  : `Save ${item.title} to Read Later`}
                title={item.saved_at ? "Remove from Read Later" : "Save to Read Later"}
                onclick={() => toggleSaved(item)}
                data-od-id={`rss-save-later-${item.id}`}
              >
                <Bookmark
                  size={16}
                  strokeWidth={1.8}
                  fill={item.saved_at ? "currentColor" : "none"}
                  aria-hidden="true"
                />
              </button>
              {#if item.has_image}
                <button
                  class="rss-item-action"
                  type="button"
                  aria-label={`View the image associated with ${item.title}`}
                  title="View image"
                  onclick={() => openItemImage(item)}
                  data-od-id={`rss-view-image-${item.id}`}
                >
                  <ImageIcon size={16} strokeWidth={1.8} aria-hidden="true" />
                </button>
              {/if}
              {#if item.comments_url}
                <button
                  class="rss-item-action"
                  type="button"
                  aria-label={`Open comments for ${item.title} in a new tab`}
                  title="Open comments"
                  onclick={() => openComments(item)}
                  data-od-id={`rss-open-comments-${item.id}`}
                >
                  <MessageCircle size={16} strokeWidth={1.8} aria-hidden="true" />
                </button>
              {/if}
              {#if articleDestination(item)}
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
            <strong>
              {activeView === "read-later"
                ? "Nothing saved for later"
                : activeView === "current" && currentSourceCount === 0
                  ? "Waiting for the first cached snapshot"
                : reader.subscriptions.length
                  ? "No items match this view"
                  : "Your reader is empty"}
            </strong>
            <p>
              {activeView === "read-later"
                ? "Use the bookmark control on any article to keep it out of pruning and return to it here."
                : activeView === "current" && currentSourceCount === 0
                  ? "The background worker will populate Current after each source completes a successful refresh."
                : reader.subscriptions.length
                  ? "Change the text, source, or category filter."
                  : "Subscribe to an RSS, Atom, or Reddit source to start reading."}
            </p>
            {#if activeView === "inbox" && reader.subscriptions.length === 0}
              <button class="ui-button ui-button--secondary rss-secondary-button" type="button" onclick={openAddFeed}>Add your first feed</button>
            {/if}
          </div>
        {/each}
      {/if}
  </main>

  <dialog
    class="rss-dialog rss-sources-dialog"
    {@attach captureSourcesDialog}
    aria-labelledby="rss-sources-title"
    onclick={(event) => event.target === sourcesDialog && sourcesDialog.close()}
    data-od-id="rss-sources-dialog"
  >
    <header>
      <div>
        <span>[ {reader.subscriptions.length} SOURCES ]</span>
        <h2 id="rss-sources-title">Edit sources</h2>
      </div>
      <button class="ui-button ui-button--ghost ui-button--icon" type="button" aria-label="Close source editor" onclick={() => sourcesDialog?.close()}>
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </header>
    <div class="rss-sources-dialog-body">
      <div class="rss-sources-summary" role="status">
        <span>Latest cached snapshot</span>
        <strong>{relativeTime(latestSnapshotAt)}</strong>
        <small>{currentSourceCount} of {reader.subscriptions.length} sources ready</small>
      </div>
      <p class="rss-sources-intro">
        Choose a feed to edit its name, category, Current limit, auto-delete policy, or remove the subscription.
      </p>
      <div class="rss-source-list">
        {#each reader.subscriptions as subscription (subscription.id)}
          <article
            class={["rss-source", sourceFilter === subscription.id && "is-active"]}
            data-od-id={`rss-source-${subscription.id}`}
          >
            <button
              class="rss-source-select"
              type="button"
              aria-pressed={sourceFilter === subscription.id}
              onclick={() => chooseSource(subscription.id)}
            >
              <span>
                <strong>{subscriptionLabel(subscription)}</strong>
                <small>
                  {hostLabel(subscription.base_url)} · {subscriptionCurrentCount(subscription.id)} current · {subscriptionUnreadCount(subscription.id)} unread
                </small>
              </span>
              <em>{subscription.category}</em>
            </button>
            <p class="rss-source-freshness">
              {subscription.refresh_generation > 0
                ? `Updated ${relativeTime(subscription.last_fetched_at)}`
                : "Waiting for first refresh"}
            </p>
            {#if subscription.custom_name}
              <p class="rss-source-original-title">Feed title · {subscription.title}</p>
            {/if}
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
              <button
                class="rss-source-edit-button"
                type="button"
                onclick={() => editFeedFromSources(subscription)}
                data-od-id={`rss-edit-source-${subscription.id}`}
              >
                <Settings2 size={14} strokeWidth={1.8} aria-hidden="true" />
                Edit settings
              </button>
            </div>
          </article>
        {:else}
          <p class="rss-source-empty">No subscriptions yet.</p>
        {/each}
      </div>
    </div>
    <footer>
      <button class="ui-button ui-button--secondary rss-secondary-button" type="button" onclick={pruneFromSources}>Prune archive</button>
      <button class="ui-button ui-button--primary rss-primary-button" type="button" onclick={() => sourcesDialog?.close()}>Done</button>
    </footer>
  </dialog>

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
        <p class="rss-detail-destination">
          {articleDestination(selectedItem)}
        </p>
      </div>
      <footer class="rss-detail-actions">
        <button
          class="ui-button ui-button--secondary rss-secondary-button"
          type="button"
          onclick={() => toggleSaved(selectedItem)}
          data-od-id="rss-detail-save-later"
        >
          <Bookmark
            size={15}
            strokeWidth={1.8}
            fill={selectedItem.saved_at ? "currentColor" : "none"}
            aria-hidden="true"
          />
          {selectedItem.saved_at ? "Remove from later" : "Read later"}
        </button>
        <button
          class="ui-button ui-button--secondary rss-secondary-button"
          type="button"
          onclick={() => toggleRead(selectedItem)}
        >
          <Check size={15} strokeWidth={2} aria-hidden="true" />
          Mark {selectedItem.read_at ? "unread" : "read"}
        </button>
        {#if selectedItem.comments_url}
          <button
            class="ui-button ui-button--secondary rss-secondary-button"
            type="button"
            onclick={() => openComments(selectedItem)}
            data-od-id="rss-detail-open-comments"
          >
            Open comments
            <MessageCircle size={15} strokeWidth={1.8} aria-hidden="true" />
          </button>
        {/if}
        {#if articleDestination(selectedItem)}
          <button
            class="ui-button ui-button--primary rss-primary-button"
            type="button"
            onclick={() => openArticle(selectedItem)}
            data-od-id="rss-detail-open-original"
          >
            Open original
            <ExternalLink size={15} strokeWidth={1.8} aria-hidden="true" />
          </button>
        {/if}
      </footer>
    {/if}
  </dialog>

  <dialog
    class="rss-dialog rss-image-dialog"
    {@attach captureItemImageDialog}
    aria-labelledby="rss-image-dialog-title"
    onclose={resetItemImage}
    onclick={(event) => event.target === itemImageDialog && closeItemImage()}
    data-od-id="rss-item-image-dialog"
  >
    {#if imageItem}
      <header>
        <div>
          <span>[ FEED.MEDIA ]</span>
          <h2 id="rss-image-dialog-title">{imageItem.title}</h2>
        </div>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          aria-label="Close feed image"
          onclick={closeItemImage}
        >
          <X size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </header>
      <div class="rss-image-stage">
        {#if imageLoadFailed}
          <div class="rss-image-error" role="status">
            <ImageIcon size={28} strokeWidth={1.5} aria-hidden="true" />
            <strong>Image unavailable</strong>
            <p>The feed image could not be loaded through Pandan's network policy.</p>
          </div>
        {:else}
          {#if !imageLoaded}
            <span class="rss-image-loading" role="status">Loading feed image…</span>
          {/if}
          <img
            class:ready={imageLoaded}
            src={rssItemImageUrl(imageItem.id)}
            alt={`Associated feed image for ${imageItem.title}`}
            onload={() => (imageLoaded = true)}
            onerror={() => (imageLoadFailed = true)}
          />
        {/if}
      </div>
    {/if}
  </dialog>

  <dialog
    class="ui-dialog rss-context-dialog"
    {@attach captureItemContextDialog}
    style:--context-x={`${contextDialogX}px`}
    style:--context-y={`${contextDialogY}px`}
    aria-label="RSS listing developer actions"
    onclose={resetItemContext}
    onclick={(event) => event.target === event.currentTarget && closeItemContext()}
    oncontextmenu={(event) => {
      event.preventDefault();
      closeItemContext();
    }}
    data-od-id="rss-item-context-dialog"
  >
    {#if contextItem}
      <div class="rss-context-actions" role="group">
        <button
          type="button"
          onclick={openItemXmlSnippet}
          data-od-id="rss-view-item-xml"
        >
          <FileCode2 size={16} strokeWidth={1.8} aria-hidden="true" />
          View XML snippet
        </button>
      </div>
    {/if}
  </dialog>

  <dialog
    class="rss-dialog rss-xml-dialog"
    {@attach captureItemXmlDialog}
    aria-labelledby="rss-xml-dialog-title"
    onclose={resetItemXmlSnippet}
    onclick={(event) => event.target === itemXmlDialog && closeItemXmlSnippet()}
    data-od-id="rss-item-xml-dialog"
  >
    {#if xmlItem}
      <header>
        <div>
          <span>[ FEED.XML.ITEM ]</span>
          <h2 id="rss-xml-dialog-title">{xmlItem.title}</h2>
        </div>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          aria-label="Close XML snippet"
          onclick={closeItemXmlSnippet}
        >
          <X size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </header>
      <div class="rss-xml-stage">
        {#if xmlSnippetLoading}
          <div class="rss-xml-state" role="status">
            <RefreshCw class="rss-loading-icon" size={22} strokeWidth={1.5} aria-hidden="true" />
            <span>Loading XML snippet…</span>
          </div>
        {:else if xmlSnippetError && !xmlSnippet}
          <div class="rss-xml-state" role="alert">
            <FileCode2 size={24} strokeWidth={1.5} aria-hidden="true" />
            <span>{xmlSnippetError}</span>
          </div>
        {:else}
          <textarea
            readonly
            spellcheck="false"
            aria-label="RSS item XML snippet"
            value={xmlSnippet}
          ></textarea>
          {#if xmlSnippetError}
            <p class="rss-xml-copy-error" role="alert">{xmlSnippetError}</p>
          {/if}
        {/if}
      </div>
      <footer class="rss-xml-actions">
        <span>Only this listing’s stored source fragment is shown.</span>
        <div>
          <button
            class="ui-button ui-button--secondary rss-secondary-button"
            type="button"
            disabled={xmlSnippetLoading || !xmlSnippet}
            onclick={copyItemXmlSnippet}
            data-od-id="rss-copy-item-xml"
          >
            {#if xmlSnippetCopied}
              <Check size={15} strokeWidth={2} aria-hidden="true" />
              Copied
            {:else}
              <Copy size={15} strokeWidth={1.8} aria-hidden="true" />
              Copy snippet
            {/if}
          </button>
          <button
            class="ui-button ui-button--secondary rss-secondary-button"
            type="button"
            onclick={closeItemXmlSnippet}
          >Close</button>
        </div>
      </footer>
    {/if}
  </dialog>

  <dialog
    class="rss-dialog rss-subscription-dialog"
    {@attach captureSubscriptionDialog}
    onclick={(event) => event.target === subscriptionDialog && closeSubscriptionDialog()}
    data-od-id="rss-subscription-dialog"
  >
    <header>
      <div>
        <h2>{editingSubscription ? "Edit feed" : "Add Feed"}</h2>
      </div>
      <button class="ui-button ui-button--ghost ui-button--icon" type="button" aria-label="Close feed settings" onclick={closeSubscriptionDialog}>
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </header>
    <form class="rss-subscription-form" onsubmit={saveSubscription}>
      <div class="rss-subscription-scroll">
      {#if !editingSubscription}
        <div class="rss-source-kind" role="group" aria-label="Feed source type" data-od-id="rss-source-type">
          <button
            class={feedSourceKind === "feed" ? "active" : undefined}
            type="button"
            aria-pressed={feedSourceKind === "feed"}
            onclick={() => selectFeedSource("feed")}
          >
            RSS / Atom
          </button>
          <button
            class={feedSourceKind === "reddit" ? "active" : undefined}
            type="button"
            aria-pressed={feedSourceKind === "reddit"}
            onclick={() => selectFeedSource("reddit")}
          >
            Reddit
          </button>
        </div>
      {/if}

      {#if editingSubscription || feedSourceKind === "feed"}
        <label for="rss-feed-url">Feed URL</label>
        <div class={["rss-feed-url-row", editingSubscription && "is-editing"]}>
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
          {#if editingSubscription}
            <button
              class="ui-button ui-button--secondary ui-button--icon rss-copy-feed-url"
              type="button"
              aria-label={feedUrlCopied ? "Feed URL copied" : "Copy feed URL"}
              title={feedUrlCopied ? "Copied" : "Copy feed URL"}
              onclick={copyFeedUrl}
              data-od-id="rss-copy-feed-url"
            >
              {#if feedUrlCopied}
                <Check size={16} strokeWidth={2} aria-hidden="true" />
              {:else}
                <Copy size={16} strokeWidth={1.8} aria-hidden="true" />
              {/if}
            </button>
          {/if}
        </div>
        <small>Public HTTPS RSS and Atom feeds are supported.</small>
      {:else}
        <section class="rss-reddit-helper" aria-labelledby="rss-reddit-helper-title" data-od-id="rss-reddit-helper">
          <div class="rss-reddit-heading">
            <span>[ REDDIT.SOURCE ]</span>
            <strong id="rss-reddit-helper-title">Build a subreddit feed</strong>
          </div>
          <label for="rss-reddit-subreddit">Subreddit</label>
          <input
            id="rss-reddit-subreddit"
            type="text"
            bind:value={redditSubreddit}
            placeholder="selfhosted or reddit.com/r/selfhosted"
            maxlength="200"
            autocomplete="off"
            required
            data-od-id="rss-reddit-subreddit"
          />
          <div class="rss-reddit-options">
            <label for="rss-reddit-sort">
              <span>Sort</span>
              <select id="rss-reddit-sort" bind:value={redditSort} data-od-id="rss-reddit-sort">
                <option value="hot">Hot</option>
                <option value="new">New</option>
                <option value="top">Top</option>
                <option value="rising">Rising</option>
              </select>
            </label>
            {#if redditSort === "top"}
              <label for="rss-reddit-period">
                <span>Period</span>
                <select id="rss-reddit-period" bind:value={redditTopPeriod} data-od-id="rss-reddit-period">
                  <option value="hour">Past hour</option>
                  <option value="day">Past day</option>
                  <option value="week">Past week</option>
                  <option value="month">Past month</option>
                  <option value="year">Past year</option>
                  <option value="all">All time</option>
                </select>
              </label>
            {/if}
          </div>
          <p class="rss-reddit-preview">
            {redditFeedPreview || "Enter a subreddit name to prepare its Reddit listing."}
          </p>
        </section>
      {/if}

      <div class="rss-feed-identity-grid">
        <label class="rss-form-field" for="rss-feed-name">
          <span>Feed name <small>Optional</small></span>
          <input
            id="rss-feed-name"
            type="text"
            bind:value={feedName}
            maxlength="80"
            placeholder={editingSubscription?.title ?? "Use the feed title"}
            data-od-id="rss-feed-name"
          />
        </label>
        <label class="rss-form-field" for="rss-feed-category">
          <span>Category</span>
          <input
            id="rss-feed-category"
            list="rss-category-options"
            bind:value={feedCategory}
            maxlength="40"
            placeholder="Technology"
            required
            data-od-id="rss-feed-category"
          />
        </label>
      </div>
      <datalist id="rss-category-options">
        {#each categories as category (category)}<option value={category}></option>{/each}
      </datalist>

      <div class="rss-setting-indicators" data-od-id="rss-feed-setting-indicators">
        <div class="rss-indicator-setting" data-od-id="rss-current-entry-limit-setting">
          <label class="rss-indicator-label" for="rss-current-entry-limit">Current view</label>
          <div class="rss-indicator-controls">
            <div class="rss-current-indicator">
              <input
                id="rss-current-entry-limit"
                type="number"
                bind:value={currentEntryLimit}
                min="1"
                max={MAX_CURRENT_ENTRY_LIMIT}
                required
                data-od-id="rss-current-entry-limit"
              />
              <span aria-hidden="true">items</span>
            </div>
            <span class="rss-help-control">
              <button
                class="rss-help-button"
                type="button"
                aria-label="About the Current view limit"
                aria-describedby="rss-current-view-help"
                data-od-id="rss-current-view-help-button"
              >?</button>
              <span class="rss-help-tooltip" id="rss-current-view-help" role="tooltip">
                Shows this many latest entries per feed from its last successful refresh. Current items are protected from pruning.
              </span>
            </span>
          </div>
        </div>

        <div class="rss-indicator-setting" data-od-id="rss-auto-delete-setting">
          <span class="rss-indicator-label" id="rss-auto-delete-label">Auto-delete old items</span>
          <div class="rss-indicator-controls">
            <button
              class="ui-toggle-button rss-indicator-toggle"
              type="button"
              aria-labelledby="rss-auto-delete-label"
              aria-pressed={retentionEnabled}
              onclick={() => (retentionEnabled = !retentionEnabled)}
              data-od-id="rss-auto-delete-toggle"
            >
              <span class="ui-toggle-indicator" aria-hidden="true"></span>
            </button>
            <span class="rss-help-control">
              <button
                class="rss-help-button"
                type="button"
                aria-label="About auto-deleting old items"
                aria-describedby="rss-auto-delete-help"
                data-od-id="rss-auto-delete-help-button"
              >?</button>
              <span class="rss-help-tooltip" id="rss-auto-delete-help" role="tooltip">
                Runs when the reader loads or this feed refreshes. Current and Read Later items are always protected.
              </span>
            </span>
          </div>
        </div>
      </div>

      {#if retentionEnabled}
        <fieldset class="rss-retention-settings">
          <legend>Auto-Delete Settings</legend>
          <div class="rss-retention-setting">
            <label class="rss-retention-setting-label" for="rss-retention-days">After</label>
            <div class="rss-retention-age">
              <input id="rss-retention-days" type="number" bind:value={retentionDays} min="1" max="3650" required />
              <span>days</span>
            </div>
          </div>
          <div class="rss-retention-setting rss-retention-scope">
            <span class="rss-retention-setting-label" id="rss-retention-scope-label">Delete scope</span>
            <div class="rss-retention-options" role="radiogroup" aria-labelledby="rss-retention-scope-label">
              <label><input type="radio" bind:group={retentionMode} value="read" /> Only items I have read</label>
              <label><input type="radio" bind:group={retentionMode} value="all" /> Read and unread items</label>
            </div>
          </div>
        </fieldset>
      {/if}

        {#if subscriptionError}<p class="rss-form-error" role="alert">{subscriptionError}</p>{/if}
      </div>

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
      <p class="rss-prune-copy">Remove historical items older than a fixed age across every subscription. Items in the latest cached snapshot and saved Read Later items are always kept, and feed-specific auto-delete settings are unchanged.</p>
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
  .rss-dialog header span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .09em; }
  .rss-reader-header p { margin-top: 8px; color: var(--muted); font-family: var(--font-mono); font-size: 11px; }
  .rss-header-actions, .rss-source-actions, .rss-dialog footer { display: flex; align-items: center; gap: 8px; }
  .rss-sources-trigger { min-height: 50px; justify-content: flex-start; padding-inline: 12px 15px; }
  .rss-sources-trigger > span { display: grid; gap: 2px; text-align: left; }
  .rss-sources-trigger strong { color: inherit; font-size: 11px; font-weight: 560; }
  .rss-sources-trigger small { color: var(--muted); font-size: 8px; font-weight: 450; letter-spacing: .01em; }
  .rss-sources-trigger:hover small { color: var(--surface); }
  .rss-view-tabs { display: flex; gap: 6px; overflow-x: auto; }
  .rss-view-tabs span { color: inherit; font-variant-numeric: tabular-nums; opacity: .7; }
  button, input, select { font: inherit; }
  button { min-height: 44px; }
  .rss-primary-button, .rss-secondary-button, .rss-danger-button { display: inline-flex; align-items: center; justify-content: center; gap: 7px; padding: 0 14px; border: 1px solid var(--border); border-radius: 7px; font-family: var(--font-mono); font-size: 11px; font-weight: 560; letter-spacing: .02em; }
  .rss-primary-button { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-primary-button:hover { background: transparent; color: var(--fg); }
  .rss-secondary-button { background: var(--page-surface, var(--surface)); color: var(--fg); }
  .rss-secondary-button:hover { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-danger-button { border-color: color-mix(in oklch, var(--fg) 55%, var(--border)); background: transparent; color: var(--fg); }
  .rss-danger-button:hover { background: var(--fg); color: var(--surface); }
  button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid var(--fg); outline-offset: 2px; }
  button:disabled { cursor: wait; opacity: .55; }
  .rss-filter-bar { display: grid; grid-template-columns: minmax(240px, 1fr) minmax(220px, .7fr) auto; gap: 8px; padding: 8px; border: 1px solid var(--border); border-radius: 9px; background: color-mix(in oklch, var(--page-surface, var(--surface)) 86%, transparent); }
  .rss-search { display: flex; align-items: center; gap: 9px; min-width: 0; padding: 0 12px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--muted); }
  .rss-search input, .rss-filter-bar select { width: 100%; min-height: 42px; border: 0; outline: 0; background: transparent; color: var(--fg); font-family: var(--font-mono); font-size: 12px; }
  .rss-filter-bar label:not(.rss-search) { min-width: 220px; padding: 0; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); }
  .rss-category-filter select { padding: 0 36px 0 14px; background: var(--bg); color: var(--fg); color-scheme: dark; }
  .rss-category-filter option { background: var(--bg); color: var(--fg); padding: 8px 14px; }
  .rss-unread-toggle { padding: 0 13px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); font-family: var(--font-mono); font-size: 11px; }
  .rss-unread-toggle.is-active { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-source-strip { display: flex; gap: 6px; min-width: 0; overflow-x: auto; overscroll-behavior-inline: contain; scrollbar-width: none; }
  .rss-source-strip::-webkit-scrollbar { display: none; }
  .rss-source-strip button { min-height: 44px; flex: 0 0 auto; display: inline-flex; align-items: center; gap: 8px; padding: 0 12px; border: 1px solid var(--border); border-radius: 999px; background: var(--page-surface, var(--surface)); color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
  .rss-source-strip button:hover { border-color: var(--fg); color: var(--fg); }
  .rss-source-strip button.active { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-source-strip span { min-width: 18px; padding: 2px 5px; border: 1px solid currentColor; border-radius: 999px; font-size: 8px; text-align: center; opacity: .72; }
  .rss-page-message { display: flex; justify-content: space-between; gap: 16px; padding: 11px 13px; border: 1px solid var(--border); background: var(--page-surface, var(--surface)); color: var(--muted); font-family: var(--font-mono); font-size: 11px; }
  .rss-page-message button { min-height: auto; color: var(--fg); text-decoration: underline; }
  .rss-item-list { min-width: 0; border: 1px solid var(--border); background: var(--page-surface, var(--surface)); }
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
  .rss-item-meta .rss-history-state { color: var(--fg); }
  .rss-item-meta time { margin-left: auto; font-variant-numeric: tabular-nums; }
  .rss-unread-dot { width: 6px; height: 6px; flex: 0 0 auto; border: 1px solid var(--muted); border-radius: 50%; }
  .rss-item:not(.is-read) .rss-unread-dot { border-color: var(--accent); background: var(--accent); }
  .rss-item-actions { display: flex; align-items: center; gap: 3px; padding: 0 8px; border-left: 1px solid var(--border); }
  .rss-item-action { display: grid; width: 44px; min-height: 44px; place-items: center; border: 1px solid transparent; color: var(--muted); transition: border-color 100ms ease, background-color 100ms ease, color 100ms ease; }
  .rss-item-action:hover { border-color: var(--fg); background: var(--page-surface, var(--surface)); color: var(--fg); }
  .rss-item-action:active { transform: translateY(1px); }
  .rss-item-action.is-active { color: var(--fg); }
  .rss-context-dialog::backdrop { background: color-mix(in oklch, var(--bg) 12%, transparent); backdrop-filter: none; }
  .rss-context-dialog { position: fixed; inset: auto; top: var(--context-y); left: var(--context-x); width: min(244px, calc(100vw - 24px)); margin: 0; padding: 0; overflow: hidden; border: 1px solid var(--border); border-radius: 0; background: color-mix(in oklch, var(--surface) 98%, var(--bg)); color: var(--fg); box-shadow: 0 24px 64px color-mix(in oklch, var(--bg) 76%, transparent); }
  .rss-context-actions { display: grid; padding: 5px; }
  .rss-context-actions button { min-height: 44px; display: flex; align-items: center; gap: 9px; padding: 0 11px; border: 1px solid transparent; color: var(--fg); font-family: var(--font-mono); font-size: 10px; letter-spacing: .02em; text-align: left; }
  .rss-context-actions button:hover:not(:disabled) { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-context-actions button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .rss-source-list { border: 1px solid var(--border); }
  .rss-source { padding: 14px; border-bottom: 1px solid var(--border); }
  .rss-source:last-child { border-bottom: 0; }
  .rss-source.is-active { background: var(--bg); }
  .rss-source-select { width: 100%; min-height: 44px; display: flex; align-items: start; justify-content: space-between; gap: 10px; text-align: left; }
  .rss-source-select > span { min-width: 0; display: grid; gap: 3px; }
  .rss-source-select strong, .rss-source-select small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rss-source-select strong { font-size: 12px; font-weight: 560; }
  .rss-source-select small { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-source-select em { flex: 0 0 auto; color: var(--muted); font-family: var(--font-mono); font-size: 9px; font-style: normal; }
  .rss-source-freshness { margin-top: 7px; color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-source-original-title { margin-top: 4px; overflow: hidden; color: var(--muted); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .rss-source-actions { margin-top: 11px; }
  .rss-source-actions button { min-height: 44px; display: inline-flex; align-items: center; gap: 5px; padding: 0 11px; border: 1px solid var(--border); color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-source-actions button:hover { border-color: var(--fg); color: var(--fg); }
  .rss-source-error { margin-top: 8px; color: var(--fg); font-size: 10px; line-height: 1.4; }
  .rss-source-empty { padding: 20px 15px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
  .rss-empty { min-height: 360px; display: grid; place-items: center; align-content: center; gap: 8px; padding: 30px; color: var(--muted); text-align: center; }
  .rss-empty strong { color: var(--fg); font-family: var(--font-display); font-size: 19px; }
  .rss-empty p { max-width: 42ch; font-size: 12px; }
  .rss-empty button { margin-top: 8px; }
  :global(.rss-loading-icon) { animation: rss-spin .8s linear infinite; }
  @keyframes rss-spin { to { transform: rotate(360deg); } }
  .rss-dialog { width: min(600px, calc(100vw - 32px)); max-height: min(780px, calc(100vh - 32px)); margin: auto; padding: 0; overflow: auto; border: 1px solid var(--border); border-radius: 10px; background: var(--page-surface, var(--surface)); color: var(--fg); box-shadow: 0 24px 80px rgba(0, 0, 0, .48); }
  .rss-dialog::backdrop { background: rgba(0, 0, 0, .7); backdrop-filter: blur(7px); }
  .rss-dialog header { min-height: 76px; display: flex; align-items: center; justify-content: space-between; gap: 20px; padding: 16px 20px; border-bottom: 1px solid var(--border); }
  .rss-dialog header h2 { margin-top: 5px; font-family: var(--font-display); font-size: 24px; font-weight: 600; letter-spacing: -.02em; }
  .rss-dialog header > button { width: 44px; min-height: 44px; display: grid; place-items: center; border: 1px solid var(--border); border-radius: 7px; }
  .rss-dialog form { display: grid; gap: 10px; padding: 22px; }
  .rss-sources-dialog { width: min(680px, calc(100vw - 32px)); overflow: hidden; }
  .rss-sources-dialog[open] { display: grid; grid-template-rows: auto minmax(0, 1fr) auto; }
  .rss-sources-dialog-body { min-height: 0; display: grid; align-content: start; gap: 14px; overflow-y: auto; overscroll-behavior: contain; scrollbar-gutter: stable; padding: 18px 20px; }
  .rss-sources-summary { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: baseline; gap: 4px 14px; padding: 12px 14px; border: 1px solid var(--border); background: var(--bg); }
  .rss-sources-summary span, .rss-sources-summary small { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-sources-summary strong { color: var(--fg); font-family: var(--font-mono); font-size: 11px; font-weight: 560; }
  .rss-sources-summary small { grid-column: 1 / -1; }
  .rss-sources-intro { max-width: 62ch; margin: -2px 0 0; color: var(--muted); font-size: 11px; line-height: 1.5; }
  .rss-source-actions .rss-source-edit-button { border-color: color-mix(in oklch, var(--fg) 32%, var(--border)); color: var(--fg); }
  .rss-sources-dialog > footer { justify-content: space-between; margin: 0; padding: 16px 20px max(16px, env(safe-area-inset-bottom)); background: var(--page-surface, var(--surface)); }
  .rss-subscription-dialog { overflow: hidden; }
  .rss-dialog .rss-subscription-form { max-height: calc(min(780px, calc(100dvh - 32px)) - 77px); grid-template-rows: minmax(0, 1fr) auto; gap: 0; overflow: hidden; padding: 0; }
  .rss-subscription-scroll { min-height: 0; display: grid; gap: 10px; overflow-y: auto; overscroll-behavior: contain; scrollbar-gutter: stable; padding: 22px; }
  .rss-subscription-scroll > label, .rss-dialog legend { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .05em; }
  .rss-dialog input[type="url"], .rss-dialog input[type="text"], .rss-dialog input[list], .rss-dialog input[type="number"], .rss-dialog select { min-height: 44px; width: 100%; padding: 0 12px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); font-family: var(--font-mono); font-size: 12px; }
  .rss-dialog input:disabled { color: var(--muted); }
  .rss-subscription-scroll > small { margin-top: -4px; color: var(--muted); font-size: 10px; }
  .rss-feed-url-row { min-width: 0; display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; }
  .rss-feed-url-row.is-editing { grid-template-columns: minmax(0, 1fr) 44px; }
  .rss-copy-feed-url { width: 44px; min-height: 44px; }
  .rss-source-kind { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; padding: 4px; border: 1px solid var(--border); border-radius: 7px; background: var(--bg); }
  .rss-source-kind button { min-height: 44px; border: 1px solid transparent; border-radius: 4px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .04em; }
  .rss-source-kind button:hover { border-color: var(--border); color: var(--fg); }
  .rss-source-kind button.active { border-color: var(--fg); background: var(--fg); color: var(--surface); }
  .rss-reddit-helper { display: grid; gap: 10px; padding: 14px; border: 1px solid var(--border); background: color-mix(in oklch, var(--bg) 82%, transparent); }
  .rss-reddit-heading { display: grid; gap: 3px; padding-bottom: 9px; border-bottom: 1px solid var(--border); }
  .rss-reddit-heading span { color: var(--muted); font-family: var(--font-mono); font-size: 9px; letter-spacing: .08em; }
  .rss-reddit-heading strong { font-family: var(--font-display); font-size: 17px; font-weight: 600; letter-spacing: -.01em; }
  .rss-reddit-helper > label, .rss-reddit-options label > span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .05em; }
  .rss-reddit-options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .rss-reddit-options label { display: grid; gap: 6px; }
  .rss-reddit-preview { overflow-wrap: anywhere; color: var(--muted); font-family: var(--font-mono); font-size: 9px; line-height: 1.5; }
  .rss-subscription-dialog header h2 { margin-top: 0; }
  .rss-feed-identity-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
  .rss-form-field { min-width: 0; display: grid; align-content: start; gap: 6px; }
  .rss-form-field > span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .05em; }
  .rss-form-field > span small { margin-left: 5px; color: var(--muted); font-size: 8px; letter-spacing: .02em; }
  .rss-setting-indicators { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); margin-top: 2px; border: 1px solid var(--border); }
  .rss-indicator-setting { min-width: 0; display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 12px; padding: 10px 12px; }
  .rss-indicator-setting + .rss-indicator-setting { border-left: 1px solid var(--border); }
  .rss-indicator-label { color: var(--fg); font-family: var(--font-mono); font-size: 10px; font-weight: 560; letter-spacing: .03em; line-height: 1.4; }
  .rss-indicator-controls { display: flex; align-items: center; gap: 4px; }
  .rss-current-indicator { display: grid; grid-template-columns: 52px auto; align-items: center; gap: 7px; }
  .rss-dialog .rss-current-indicator input[type="number"] { padding: 0 7px; text-align: center; }
  .rss-current-indicator > span { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-indicator-toggle { width: 46px; justify-content: center; padding: 4px; }
  .rss-help-control { position: relative; display: inline-grid; place-items: center; }
  .rss-help-button { width: 44px; min-height: 44px; display: grid; place-items: center; border: 1px solid transparent; border-radius: 50%; color: var(--muted); font-family: var(--font-mono); font-size: 12px; font-weight: 600; }
  .rss-help-button:hover, .rss-help-button:focus-visible { border-color: var(--border); background: var(--bg); color: var(--fg); }
  .rss-help-tooltip { position: absolute; z-index: 4; right: 0; bottom: calc(100% + 7px); width: min(270px, 72vw); padding: 9px 10px; border: 1px solid var(--border); background: var(--bg); color: var(--fg); box-shadow: 0 12px 36px rgba(0, 0, 0, .34); font-size: 10px; line-height: 1.5; opacity: 0; pointer-events: none; transform: translateY(3px); transition: opacity 120ms var(--ease-out), transform 120ms var(--ease-out); }
  .rss-help-control:hover .rss-help-tooltip, .rss-help-control:focus-within .rss-help-tooltip { opacity: 1; transform: translateY(0); }
  .rss-dialog .rss-retention-settings { gap: 0; padding: 14px; }
  .rss-retention-setting { display: grid; grid-template-columns: 88px minmax(0, 1fr); align-items: start; gap: 16px; }
  .rss-retention-setting + .rss-retention-setting { margin-top: 13px; padding-top: 13px; border-top: 1px solid var(--border); }
  .rss-retention-setting-label { min-height: 44px; display: flex; align-items: center; }
  .rss-retention-scope { grid-template-columns: 1fr; gap: 4px; }
  .rss-retention-scope .rss-retention-setting-label { min-height: 0; }
  .rss-dialog .rss-retention-setting-label, .rss-retention-age span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .03em; }
  .rss-retention-age { display: grid; grid-template-columns: 100px auto; align-items: center; justify-content: start; gap: 9px; }
  .rss-retention-options { display: grid; gap: 9px; }
  .rss-retention-options label { min-height: 44px; }
  .rss-dialog fieldset { display: grid; gap: 8px; margin: 2px 0 8px; padding: 12px; border: 1px solid var(--border); }
  .rss-dialog fieldset label { display: flex; align-items: center; gap: 8px; color: var(--fg); font-size: 11px; }
  .rss-dialog fieldset input { accent-color: var(--fg); }
  .rss-dialog footer { justify-content: flex-end; margin-top: 8px; padding-top: 16px; border-top: 1px solid var(--border); }
  .rss-subscription-form > footer { margin: 0; padding: 16px 22px 22px; background: var(--page-surface, var(--surface)); }
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
  .rss-image-dialog { width: min(1000px, calc(100vw - 32px)); overflow: hidden; }
  .rss-image-dialog[open] { display: grid; grid-template-rows: auto minmax(0, 1fr); }
  .rss-image-dialog header h2 { max-width: 34ch; overflow: hidden; font-size: 20px; line-height: 1.25; text-overflow: ellipsis; white-space: nowrap; }
  .rss-image-stage { position: relative; min-height: 260px; display: grid; place-items: center; overflow: auto; padding: 18px; background: var(--bg); }
  .rss-image-stage img { display: block; width: auto; height: auto; max-width: 100%; max-height: calc(100dvh - 170px); object-fit: contain; opacity: 0; transition: opacity 150ms var(--ease-out); }
  .rss-image-stage img.ready { opacity: 1; }
  .rss-image-loading { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .04em; }
  .rss-image-error { display: grid; place-items: center; gap: 7px; max-width: 42ch; color: var(--muted); text-align: center; }
  .rss-image-error strong { color: var(--fg); font-family: var(--font-display); font-size: 18px; font-weight: 600; }
  .rss-image-error p { margin: 0; font-size: 11px; line-height: 1.5; }
  .rss-xml-dialog { width: min(900px, calc(100vw - 32px)); overflow: hidden; }
  .rss-xml-dialog[open] { display: grid; grid-template-rows: auto minmax(0, 1fr) auto; }
  .rss-xml-dialog header h2 { max-width: 34ch; overflow: hidden; font-size: 20px; line-height: 1.25; text-overflow: ellipsis; white-space: nowrap; }
  .rss-xml-stage { min-height: 300px; display: grid; overflow: auto; padding: 18px; background: var(--bg); scrollbar-gutter: stable; }
  .rss-xml-stage textarea { width: 100%; min-height: 264px; margin: 0; padding: 16px; resize: none; border: 1px solid var(--border); background: var(--page-surface, var(--surface)); color: var(--fg); font-family: var(--font-mono); font-size: 11px; line-height: 1.65; overflow-wrap: anywhere; tab-size: 2; white-space: pre-wrap; }
  .rss-xml-stage textarea:focus-visible { outline: 2px solid var(--fg); outline-offset: -2px; }
  .rss-xml-state { min-height: 264px; display: grid; place-items: center; align-content: center; gap: 9px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; text-align: center; }
  .rss-xml-copy-error { margin: 12px 0 0; color: var(--fg); font-family: var(--font-mono); font-size: 10px; }
  .rss-xml-actions { justify-content: space-between; padding: 14px 18px max(14px, env(safe-area-inset-bottom)); border-top: 1px solid var(--border); }
  .rss-xml-actions > span { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .rss-xml-actions > div { display: flex; align-items: center; gap: 8px; }
  .rss-form-error { padding: 10px; border: 1px solid color-mix(in oklch, var(--fg) 28%, var(--border)); background: var(--fg-soft); color: var(--fg); font-size: 11px; }
  .rss-prune-copy { color: var(--muted); font-size: 12px; line-height: 1.55; }
  @media (max-width: 920px) {
    .rss-reader { padding: 20px 16px; }
    .rss-reader-header { align-items: start; flex-direction: column; }
    .rss-filter-bar { grid-template-columns: 1fr; }
    .rss-filter-bar label:not(.rss-search) { min-width: 0; }
  }
  @media (max-width: 560px) {
    .rss-header-actions { width: 100%; }
    .rss-header-actions button { flex: 1; }
    .rss-sources-trigger small { max-width: 21ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .rss-item { grid-template-columns: 1fr; }
    .rss-item-open { min-height: 82px; padding: 10px; }
    .rss-item-actions { justify-content: flex-end; padding: 4px; border-top: 1px solid var(--border); border-left: 0; }
    .rss-item-action { width: 42px; min-height: 44px; }
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
    .rss-xml-actions { align-items: stretch; flex-direction: column; }
    .rss-xml-actions > div { width: 100%; }
    .rss-xml-actions > div button { flex: 1; }
    .rss-reddit-options { grid-template-columns: 1fr; }
    .rss-feed-identity-grid { grid-template-columns: 1fr; }
    .rss-setting-indicators { grid-template-columns: 1fr; }
    .rss-indicator-setting + .rss-indicator-setting { border-top: 1px solid var(--border); border-left: 0; }
    .rss-retention-setting { grid-template-columns: 1fr; gap: 7px; }
    .rss-retention-setting-label { min-height: 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.rss-loading-icon) { animation: none; }
    .rss-help-tooltip { transition: none; }
    .rss-image-stage img { transition: none; }
  }
</style>

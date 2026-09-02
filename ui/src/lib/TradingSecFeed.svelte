<script lang="ts">
  import ExternalLink from "lucide-svelte/icons/external-link";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Rss from "lucide-svelte/icons/rss";
  import { onMount } from "svelte";
  import {
    fetchTradingSecFeed,
    type TradingSecFeedResponse,
    type TradingSecFeedSource,
  } from "$lib/api";

  type SourceFilter = "all" | TradingSecFeedSource;

  const emptyFeed: TradingSecFeedResponse = {
    fetched_at: "",
    sources: [],
    items: [],
    warning: null,
  };

  let feed = $state.raw<TradingSecFeedResponse>(emptyFeed);
  let sourceFilter = $state<SourceFilter>("all");
  let loading = $state(false);
  let refreshing = $state(false);
  let error = $state("");
  let mounted = false;

  let visibleItems = $derived.by(() =>
    sourceFilter === "all"
      ? feed.items
      : feed.items.filter((item) => item.source === sourceFilter),
  );
  let availableSources = $derived(
    feed.sources.filter((source) => source.error === null).length,
  );

  onMount(() => {
    mounted = true;
    void loadFeed();
    return () => {
      mounted = false;
    };
  });

  async function loadFeed() {
    if (loading || refreshing) return;
    const firstLoad = feed.fetched_at === "";
    if (firstLoad) {
      loading = true;
    } else {
      refreshing = true;
    }
    error = "";
    try {
      const snapshot = await fetchTradingSecFeed();
      if (mounted) feed = snapshot;
    } catch (reason: unknown) {
      if (mounted) {
        error = errorMessage(
          reason,
          "Unable to load the SEC feeds. Existing results remain visible.",
        );
      }
    } finally {
      if (mounted) {
        loading = false;
        refreshing = false;
      }
    }
  }

  function errorMessage(reason: unknown, fallback: string) {
    return reason instanceof Error && reason.message.trim()
      ? reason.message
      : fallback;
  }

  function formatTimestamp(value: string) {
    if (!value) return "Not loaded";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  }
</script>

<section
  class="sec-feed-panel"
  aria-busy={loading || refreshing}
  data-od-id="trading-sec-feed"
>
  <header>
    <div>
      <span>[ SEC.GOV / CURRENT ]</span>
      <h2>SEC activity monitor</h2>
      <p>
        Current announcements, statements, proceedings, litigation releases,
        and trading suspensions. Nothing is stored.
      </p>
    </div>
    <button
      class="ui-button ui-button--secondary"
      type="button"
      disabled={loading || refreshing}
      onclick={loadFeed}
      data-od-id="refresh-trading-sec-feed"
    >
      <RefreshCw
        size={15}
        strokeWidth={1.8}
        class={refreshing ? "spinning" : undefined}
        aria-hidden="true"
      />
      {refreshing ? "Refreshing…" : "Refresh SEC"}
    </button>
  </header>

  <div class="sec-feed-toolbar">
    <label for="trading-sec-source">Feed</label>
    <select
      id="trading-sec-source"
      bind:value={sourceFilter}
      disabled={loading || feed.sources.length === 0}
      data-od-id="trading-sec-source-filter"
    >
      <option value="all">All SEC sources</option>
      {#each feed.sources as source (source.id)}
        <option value={source.id}>
          {source.label}{source.error ? " · unavailable" : ""}
        </option>
      {/each}
    </select>
    <div class="sec-feed-status" aria-live="polite">
      <strong>{visibleItems.length} current items</strong>
      <small>
        {#if feed.fetched_at}
          Fetched {formatTimestamp(feed.fetched_at)} · {availableSources}/{feed.sources.length}
          sources
        {:else}
          Fetches only while Trading is open
        {/if}
      </small>
    </div>
  </div>

  {#if error}
    <p class="sec-feed-message sec-feed-message--error" role="alert">{error}</p>
  {/if}
  {#if feed.warning}
    <p class="sec-feed-message" role="status">{feed.warning}</p>
  {/if}

  {#if loading}
    <div class="sec-feed-loading" role="status">
      <Rss size={24} strokeWidth={1.5} aria-hidden="true" />
      <span>Loading the current SEC feeds…</span>
    </div>
  {:else if visibleItems.length > 0}
    <ol class="sec-feed-list" data-od-id="trading-sec-feed-items">
      {#each visibleItems as item, index (item.id)}
        <li data-od-id={`sec-feed-item-${item.source}-${index + 1}`}>
          <a href={item.url} target="_blank" rel="noopener noreferrer">
            <span class="sec-feed-source">{item.source_label}</span>
            <strong>{item.title}</strong>
            <span class="sec-feed-meta">
              <time datetime={item.published_at}>
                {formatTimestamp(item.published_at)}
              </time>
              <ExternalLink size={14} strokeWidth={1.8} aria-hidden="true" />
            </span>
          </a>
        </li>
      {/each}
    </ol>
  {:else if !error}
    <div class="sec-feed-empty" data-od-id="trading-sec-feed-empty">
      <Rss size={24} strokeWidth={1.5} aria-hidden="true" />
      <strong>No current items in this feed</strong>
      <span>Try another source or refresh the SEC response.</span>
    </div>
  {/if}
</section>

<style>
  .sec-feed-panel {
    min-width: 0;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
  }

  .sec-feed-panel > header {
    display: flex;
    min-height: 78px;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
  }

  .sec-feed-panel > header > div {
    min-width: 0;
  }

  .sec-feed-panel > header span,
  .sec-feed-toolbar label,
  .sec-feed-source {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .sec-feed-panel h2 {
    margin: 5px 0 0;
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 550;
  }

  .sec-feed-panel header p {
    max-width: 70ch;
    margin: 5px 0 0;
    color: var(--muted);
    font-size: 10px;
    line-height: 1.55;
  }

  .sec-feed-panel > header button {
    min-height: 44px;
    flex: 0 0 auto;
    gap: 8px;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.02em;
  }

  .sec-feed-toolbar {
    display: grid;
    grid-template-columns: auto minmax(180px, 260px) minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }

  .sec-feed-toolbar select {
    width: 100%;
    min-height: 44px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--page-surface, var(--surface));
    padding: 0 36px 0 11px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .sec-feed-status {
    min-width: 0;
    display: grid;
    justify-items: end;
    gap: 3px;
    text-align: right;
  }

  .sec-feed-status strong,
  .sec-feed-status small {
    overflow-wrap: anywhere;
    font-family: var(--font-mono);
  }

  .sec-feed-status strong {
    font-size: 10px;
    font-weight: 550;
  }

  .sec-feed-status small {
    color: var(--muted);
    font-size: 9px;
  }

  .sec-feed-message {
    margin: 0;
    border-bottom: 1px solid var(--border);
    background: color-mix(in oklch, var(--fg) 4%, transparent);
    padding: 9px 14px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    line-height: 1.55;
  }

  .sec-feed-message--error {
    border-color: color-mix(in oklch, var(--danger) 55%, var(--border));
    background: color-mix(in oklch, var(--danger) 10%, transparent);
    color: var(--fg);
  }

  .sec-feed-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .sec-feed-list li {
    border-bottom: 1px solid var(--border);
  }

  .sec-feed-list li:last-child {
    border-bottom: 0;
  }

  .sec-feed-list a {
    display: grid;
    min-height: 64px;
    grid-template-columns: minmax(145px, 0.42fr) minmax(0, 1.6fr) auto;
    align-items: center;
    gap: 16px;
    padding: 10px 14px;
    color: var(--fg);
    text-decoration: none;
    transition:
      background-color 120ms var(--ease-out),
      border-color 120ms var(--ease-out);
  }

  .sec-feed-list a:hover {
    background: color-mix(in oklch, var(--fg) 6%, transparent);
  }

  .sec-feed-list a:focus-visible,
  .sec-feed-panel button:focus-visible,
  .sec-feed-panel select:focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: -2px;
  }

  .sec-feed-list strong {
    overflow-wrap: anywhere;
    font-size: 11px;
    font-weight: 550;
    line-height: 1.55;
  }

  .sec-feed-meta {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    white-space: nowrap;
  }

  .sec-feed-empty,
  .sec-feed-loading {
    display: grid;
    min-height: 180px;
    place-content: center;
    justify-items: center;
    gap: 9px;
    padding: 24px;
    color: var(--muted);
    text-align: center;
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .sec-feed-empty strong {
    color: var(--fg);
    font-size: 11px;
    font-weight: 550;
  }

  .sec-feed-empty span {
    line-height: 1.55;
  }

  @media (max-width: 760px) {
    .sec-feed-panel > header {
      align-items: stretch;
      flex-direction: column;
    }

    .sec-feed-panel > header button {
      width: 100%;
    }

    .sec-feed-toolbar {
      grid-template-columns: auto minmax(0, 1fr);
    }

    .sec-feed-status {
      grid-column: 1 / -1;
      justify-items: start;
      text-align: left;
    }

    .sec-feed-list a {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 6px 12px;
    }

    .sec-feed-source {
      grid-column: 1;
    }

    .sec-feed-list strong {
      grid-column: 1 / -1;
      grid-row: 2;
    }

    .sec-feed-meta {
      grid-column: 2;
      grid-row: 1;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .sec-feed-list a {
      transition: none;
    }
  }
</style>

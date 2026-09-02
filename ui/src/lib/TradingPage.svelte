<script lang="ts">
  import Database from "lucide-svelte/icons/database";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import KeyRound from "lucide-svelte/icons/key-round";
  import Plus from "lucide-svelte/icons/plus";
  import Radio from "lucide-svelte/icons/radio";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Settings2 from "lucide-svelte/icons/settings-2";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import { onMount } from "svelte";
  import {
    createTradingSymbol,
    deleteTradingFinnhubKey,
    deleteTradingSymbol,
    fetchTrading,
    refreshTrading,
    saveTradingFinnhubKey,
    tradingEventsUrl,
    type TradingResponse,
    type TradingStreamEvent,
  } from "$lib/api";
  import { motionDisclosure } from "$lib/motion.svelte";
  import TradingSecFeed from "$lib/TradingSecFeed.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";

  type StreamState =
    | "off"
    | "connecting"
    | "live"
    | "reconnecting"
    | "interrupted";

  const emptyTrading: TradingResponse = {
    watchlist: [],
    provider: "yahoo",
    has_finnhub_api_key: false,
    secret_storage_enabled: false,
    last_refresh_at: null,
    last_refresh_error: null,
    stream_interval_seconds: null,
  };

  let trading = $state.raw<TradingResponse>(emptyTrading);
  let loading = $state(true);
  let refreshing = $state(false);
  let pageError = $state("");
  let sourceOpen = $state(false);
  let symbol = $state("");
  let savingSymbol = $state(false);
  let deletingSymbolId = $state("");
  let finnhubKey = $state("");
  let savingKey = $state(false);
  let removingKey = $state(false);
  let streamState = $state<StreamState>("off");
  let eventSource: EventSource | undefined;
  let mounted = false;

  let providerStatus = $derived.by(() => {
    if (!trading.has_finnhub_api_key) {
      return "Yahoo cached · refreshes when this page opens";
    }
    if (streamState === "live") {
      return `Finnhub live · every ${trading.stream_interval_seconds ?? 20}s while open`;
    }
    if (streamState === "connecting") {
      return "Finnhub · connecting";
    }
    if (streamState === "reconnecting") {
      return "Finnhub · reconnecting with cached prices visible";
    }
    if (streamState === "interrupted") {
      return "Finnhub · live updates interrupted";
    }
    return "Finnhub configured · opens a live feed on this page";
  });

  onMount(() => {
    mounted = true;
    void loadTrading();
    return () => {
      mounted = false;
      stopStream();
    };
  });

  async function loadTrading() {
    loading = true;
    pageError = "";
    try {
      applySnapshot(await fetchTrading());
    } catch (reason: unknown) {
      pageError = errorMessage(reason, "Unable to load the Trading cache.");
      return;
    } finally {
      loading = false;
    }
    if (!mounted || trading.watchlist.length === 0) return;
    if (trading.has_finnhub_api_key) {
      startStream();
    } else {
      void refreshPrices();
    }
  }

  function applySnapshot(snapshot: TradingResponse) {
    trading = snapshot;
  }

  async function refreshPrices() {
    if (refreshing || trading.watchlist.length === 0) return;
    refreshing = true;
    pageError = "";
    try {
      const snapshot = await refreshTrading();
      if (mounted) applySnapshot(snapshot);
    } catch (reason: unknown) {
      if (mounted) {
        pageError = errorMessage(
          reason,
          "Unable to refresh prices. Cached values remain visible.",
        );
      }
    } finally {
      if (mounted) refreshing = false;
    }
  }

  function startStream() {
    stopStream();
    if (
      !mounted ||
      !trading.has_finnhub_api_key ||
      trading.watchlist.length === 0
    ) {
      return;
    }
    streamState = "connecting";
    const source = new EventSource(tradingEventsUrl, { withCredentials: true });
    eventSource = source;
    source.onopen = () => {
      if (eventSource === source) streamState = "connecting";
    };
    source.addEventListener("snapshot", (event) => {
      if (eventSource !== source) return;
      try {
        const payload = JSON.parse(
          (event as MessageEvent<string>).data,
        ) as TradingStreamEvent;
        applySnapshot(payload.snapshot);
        pageError = "";
        streamState = "live";
      } catch {
        streamState = "interrupted";
      }
    });
    source.addEventListener("stream-error", () => {
      if (eventSource === source) streamState = "interrupted";
    });
    source.onerror = () => {
      if (eventSource === source) streamState = "reconnecting";
    };
  }

  function stopStream() {
    eventSource?.close();
    eventSource = undefined;
    streamState = "off";
  }

  async function addSymbol(event: SubmitEvent) {
    event.preventDefault();
    const normalized = symbol.trim().toUpperCase();
    if (!normalized || savingSymbol) return;
    savingSymbol = true;
    pageError = "";
    try {
      applySnapshot(await createTradingSymbol(normalized));
      symbol = "";
      if (trading.has_finnhub_api_key) {
        startStream();
      } else {
        void refreshPrices();
      }
    } catch (reason: unknown) {
      pageError = errorMessage(reason, "Unable to add that symbol.");
    } finally {
      savingSymbol = false;
    }
  }

  async function removeSymbol(id: string) {
    if (deletingSymbolId) return;
    deletingSymbolId = id;
    pageError = "";
    try {
      applySnapshot(await deleteTradingSymbol(id));
      if (trading.watchlist.length === 0) stopStream();
    } catch (reason: unknown) {
      pageError = errorMessage(reason, "Unable to remove that symbol.");
    } finally {
      deletingSymbolId = "";
    }
  }

  async function saveFinnhubKey(event: SubmitEvent) {
    event.preventDefault();
    const key = finnhubKey.trim();
    if (!key || savingKey) return;
    savingKey = true;
    pageError = "";
    try {
      applySnapshot(await saveTradingFinnhubKey(key));
      finnhubKey = "";
      if (trading.watchlist.length > 0) startStream();
    } catch (reason: unknown) {
      pageError = errorMessage(
        reason,
        "Unable to validate and save the Finnhub API key.",
      );
    } finally {
      savingKey = false;
    }
  }

  async function removeFinnhubKey() {
    if (removingKey) return;
    removingKey = true;
    pageError = "";
    stopStream();
    try {
      applySnapshot(await deleteTradingFinnhubKey());
      if (trading.watchlist.length > 0) void refreshPrices();
    } catch (reason: unknown) {
      pageError = errorMessage(reason, "Unable to remove the Finnhub API key.");
      if (trading.has_finnhub_api_key) startStream();
    } finally {
      removingKey = false;
    }
  }

  function errorMessage(reason: unknown, fallback: string) {
    return reason instanceof Error && reason.message.trim()
      ? reason.message
      : fallback;
  }

  function formatDecimal(value: string | null, digits = 2) {
    if (!value) return "—";
    const match = /^([+-]?)(\d+)(?:\.(\d+))?$/.exec(value.trim());
    if (!match) return value;
    const [, sign, integer, rawFraction = ""] = match;
    const grouped = integer.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
    const fraction = rawFraction.slice(0, digits).replace(/0+$/, "");
    return `${sign}${grouped}${fraction ? `.${fraction}` : ""}`;
  }

  function formatPrice(price: string, currency: string) {
    const value = formatDecimal(price);
    return currency ? `${value} ${currency}` : value;
  }

  function formatPercent(value: string | null) {
    if (!value) return "—";
    const formatted = formatDecimal(value);
    return `${formatted.startsWith("+") || formatted.startsWith("-") ? "" : "+"}${formatted}%`;
  }

  function changeTone(value: string | null) {
    if (!value) return "neutral";
    if (value.trim().startsWith("-")) return "negative";
    return "positive";
  }

  function formatTimestamp(value: string | null) {
    if (!value) return "No successful refresh yet";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
      second: "2-digit",
    }).format(date);
  }

  function sourceLabel(source: "yahoo" | "finnhub") {
    return source === "finnhub" ? "Finnhub" : "Yahoo Finance";
  }

  function yahooQuoteUrl(symbol: string) {
    return `https://finance.yahoo.com/quote/${encodeURIComponent(symbol)}/`;
  }
</script>

<section class="trading-page product-page" data-od-id="trading-page">
  <header class="trading-header page-header" data-od-id="trading-header">
    <div>
      <TypedHeading text="$ trading --watch" odId="trading-heading" />
      <p>
        Follow a focused watchlist with an immediate cached snapshot and
        page-scoped price updates.
      </p>
    </div>
    <div class="trading-header-actions" data-od-id="trading-toolbar">
      <button
        class="ui-button ui-button--secondary"
        type="button"
        disabled={loading || refreshing || trading.watchlist.length === 0}
        onclick={() => refreshPrices()}
        data-od-id="refresh-trading"
      >
        <RefreshCw size={15} strokeWidth={1.8} aria-hidden="true" />
        {refreshing ? "Refreshing…" : "Refresh now"}
      </button>
      <button
        class="ui-button ui-button--ghost"
        type="button"
        aria-expanded={sourceOpen}
        aria-controls="trading-source-panel"
        onclick={() => (sourceOpen = !sourceOpen)}
        data-od-id="configure-trading-source"
      >
        <Settings2 size={15} strokeWidth={1.8} aria-hidden="true" />
        Data source
      </button>
    </div>
  </header>

  <div class="refresh-state" aria-live="polite" data-od-id="trading-refresh-state">
    <span
      class="provider-indicator"
      data-state={refreshing
        ? "refreshing"
        : trading.has_finnhub_api_key && streamState === "live"
          ? "live"
          : "cached"}
      aria-hidden="true"
    ></span>
    <span>
      <strong>{refreshing ? "Refreshing prices" : providerStatus}</strong>
      <small>
        Last successful refresh: {formatTimestamp(trading.last_refresh_at)}
      </small>
    </span>
  </div>

  {#if pageError}
    <p class="trading-error" role="alert">{pageError}</p>
  {/if}
  {#if trading.last_refresh_error}
    <p class="provider-error" role="status">
      {trading.last_refresh_error}
    </p>
  {/if}

  <div
    class="source-disclosure"
    id="trading-source-panel"
    aria-hidden={!sourceOpen}
    inert={!sourceOpen}
    data-od-id="trading-source-panel"
    {@attach motionDisclosure(sourceOpen)}
  >
    <section class="source-panel">
      <header>
        <div>
          <span>[ PRICE.SOURCE ]</span>
          <h2>Choose the refresh path</h2>
        </div>
        {#if trading.has_finnhub_api_key}
          <span class="source-badge"><Radio size={13} strokeWidth={1.8} /> Live</span>
        {:else}
          <span class="source-badge"><Database size={13} strokeWidth={1.8} /> Cached</span>
        {/if}
      </header>
      <div class="source-grid">
        <div>
          <strong>Yahoo Finance</strong>
          <p>
            The default source. Pandan loads saved prices immediately, then
            refreshes once when Trading opens or whenever you request it.
          </p>
        </div>
        <div>
          <strong>Finnhub live</strong>
          <p>
            An optional key enables 20-second updates only while this page is
            open. The key stays encrypted on the server and is never returned.
          </p>
        </div>
      </div>

      {#if !trading.secret_storage_enabled}
        <p class="credential-note">
          Finnhub keys require <code>PANDAN_SECRET_KEY</code> on the server.
          Yahoo Finance remains available without it.
        </p>
      {:else if trading.has_finnhub_api_key}
        <div class="connected-source">
          <span>
            <KeyRound size={16} strokeWidth={1.8} aria-hidden="true" />
            <span>
              <strong>Finnhub key stored</strong>
              <small>The saved value cannot be viewed from the browser.</small>
            </span>
          </span>
          <button
            class="ui-button ui-button--danger"
            type="button"
            disabled={removingKey}
            onclick={removeFinnhubKey}
            data-od-id="remove-finnhub-key"
          >
            {removingKey ? "Removing…" : "Remove key"}
          </button>
        </div>
      {:else}
        <form class="key-form" onsubmit={saveFinnhubKey} data-od-id="finnhub-key-form">
          <label for="finnhub-api-key">Finnhub API key</label>
          <div>
            <input
              id="finnhub-api-key"
              type="password"
              bind:value={finnhubKey}
              maxlength="256"
              autocomplete="off"
              spellcheck="false"
              placeholder="Paste a Finnhub key"
              required
            />
            <button
              class="ui-button ui-button--secondary"
              type="submit"
              disabled={savingKey || !finnhubKey.trim()}
              data-od-id="save-finnhub-key"
            >
              {savingKey ? "Validating…" : "Save key"}
            </button>
          </div>
        </form>
      {/if}
    </section>
  </div>

  <section class="watchlist-panel" aria-busy={loading} data-od-id="trading-watchlist">
    <header>
      <div>
        <span>[ WATCHLIST ]</span>
        <h2>Tracked symbols</h2>
      </div>
      <span>{trading.watchlist.length} / 10</span>
    </header>

    <form class="symbol-form" onsubmit={addSymbol} data-od-id="add-trading-symbol">
      <label for="trading-symbol">Ticker symbol</label>
      <div>
        <input
          id="trading-symbol"
          bind:value={symbol}
          maxlength="16"
          autocapitalize="characters"
          autocomplete="off"
          spellcheck="false"
          placeholder="AAPL"
          disabled={loading || trading.watchlist.length >= 10}
          required
        />
        <button
          class="ui-button ui-button--primary"
          type="submit"
          disabled={savingSymbol || !symbol.trim() || trading.watchlist.length >= 10}
          data-od-id="submit-trading-symbol"
        >
          <Plus size={15} strokeWidth={1.8} aria-hidden="true" />
          {savingSymbol ? "Adding…" : "Add symbol"}
        </button>
      </div>
    </form>

    {#if loading}
      <div class="trading-loading" role="status">
        <span>Loading the last saved snapshot…</span>
        <div aria-hidden="true"></div>
        <div aria-hidden="true"></div>
        <div aria-hidden="true"></div>
      </div>
    {:else if trading.watchlist.length === 0}
      <div class="trading-empty" data-od-id="trading-empty-state">
        <Database size={28} strokeWidth={1.4} aria-hidden="true" />
        <h3>No symbols tracked</h3>
        <p>Add a ticker above. Its first price will load without hiding this page.</p>
      </div>
    {:else}
      <div class="quote-list">
        {#each trading.watchlist as item (item.id)}
          <article class="quote-card" data-od-id={`trading-quote-${item.symbol.toLowerCase()}`}>
            <header>
              <div>
                <strong>{item.symbol}</strong>
                <span>{item.quote?.name ?? "Waiting for the first quote"}</span>
              </div>
              <div class="quote-card-actions">
                <a
                  class="ui-button ui-button--ghost quote-card-link"
                  href={yahooQuoteUrl(item.symbol)}
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label={`Open ${item.symbol} on Yahoo Finance in a new tab`}
                  title={`View ${item.symbol} on Yahoo Finance`}
                  data-od-id={`open-yahoo-${item.symbol.toLowerCase()}`}
                >
                  <ExternalLink size={14} strokeWidth={1.8} aria-hidden="true" />
                  Yahoo
                </a>
                <button
                  class="ui-button ui-button--ghost ui-button--icon"
                  type="button"
                  aria-label={`Remove ${item.symbol} from the watchlist`}
                  title={`Remove ${item.symbol}`}
                  disabled={deletingSymbolId !== ""}
                  onclick={() => removeSymbol(item.id)}
                  data-od-id={`remove-trading-symbol-${item.symbol.toLowerCase()}`}
                >
                  <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
                </button>
              </div>
            </header>
            {#if item.quote}
              <div class="quote-primary">
                <strong>{formatPrice(item.quote.price, item.quote.currency)}</strong>
                <span data-tone={changeTone(item.quote.change_percent)}>
                  {formatPercent(item.quote.change_percent)}
                </span>
              </div>
              <dl>
                <div><dt>Open</dt><dd>{formatDecimal(item.quote.day_open)}</dd></div>
                <div><dt>High</dt><dd>{formatDecimal(item.quote.day_high)}</dd></div>
                <div><dt>Low</dt><dd>{formatDecimal(item.quote.day_low)}</dd></div>
                <div>
                  <dt>Previous</dt>
                  <dd>{formatDecimal(item.quote.previous_close)}</dd>
                </div>
              </dl>
              <footer>
                <span>{sourceLabel(item.quote.source)}</span>
                {#if item.quote.market_state}
                  <span>{item.quote.market_state}</span>
                {/if}
                <time datetime={item.quote.refreshed_at}>
                  Saved {formatTimestamp(item.quote.refreshed_at)}
                </time>
              </footer>
            {:else}
              <div class="quote-unavailable">
                <span>No cached price yet.</span>
                <small>The next refresh will try this symbol.</small>
              </div>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  </section>

  <TradingSecFeed />
</section>

<style>
  .trading-page {
    display: grid;
    gap: 18px;
  }

  .trading-header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 24px;
  }

  .trading-header > div:first-child {
    min-width: 0;
  }

  .trading-header p {
    max-width: 68ch;
    margin: 8px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.6;
  }

  .trading-header-actions {
    display: flex;
    flex: 0 0 auto;
    gap: 8px;
  }

  .trading-header-actions button,
  .symbol-form button,
  .key-form button,
  .connected-source button {
    min-height: 44px;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.02em;
  }

  .trading-header-actions button,
  .symbol-form button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
  }

  .refresh-state {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 12px;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    padding: 11px 14px;
  }

  .refresh-state > span:last-child {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .refresh-state strong,
  .refresh-state small {
    overflow-wrap: anywhere;
    font-family: var(--font-mono);
  }

  .refresh-state strong {
    font-size: 10px;
    font-weight: 550;
    letter-spacing: 0.02em;
  }

  .refresh-state small {
    color: var(--muted);
    font-size: 9px;
  }

  .provider-indicator {
    width: 9px;
    height: 9px;
    flex: 0 0 9px;
    border: 1px solid var(--muted);
    background: transparent;
  }

  .provider-indicator[data-state="live"] {
    border-color: var(--accent);
    background: var(--accent);
  }

  .provider-indicator[data-state="refreshing"] {
    border-color: var(--fg);
    background: var(--fg);
  }

  .trading-error,
  .provider-error {
    margin: 0;
    border: 1px solid color-mix(in oklch, var(--danger) 55%, var(--border));
    background: color-mix(in oklch, var(--danger) 10%, transparent);
    padding: 10px 12px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.55;
  }

  .provider-error {
    border-color: var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--muted);
  }

  .source-disclosure {
    min-width: 0;
  }

  .source-panel,
  .watchlist-panel {
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
  }

  .source-panel > header,
  .watchlist-panel > header {
    display: flex;
    min-height: 62px;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }

  .source-panel > header span:first-child,
  .watchlist-panel > header span:first-child {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
  }

  .source-panel h2,
  .watchlist-panel h2 {
    margin: 5px 0 0;
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 550;
  }

  .source-badge {
    display: inline-flex;
    min-height: 30px;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--border);
    padding: 0 9px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .source-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border-bottom: 1px solid var(--border);
  }

  .source-grid > div {
    min-width: 0;
    padding: 16px 14px;
    border-right: 1px solid var(--border);
  }

  .source-grid > div:last-child {
    border-right: 0;
  }

  .source-grid strong,
  .connected-source strong {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 550;
  }

  .source-grid p,
  .credential-note {
    max-width: 62ch;
    margin: 7px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.65;
  }

  .credential-note {
    margin: 14px;
    border: 1px solid var(--border);
    padding: 12px;
  }

  .credential-note code {
    color: var(--fg);
    font-family: var(--font-mono);
  }

  .connected-source {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px;
  }

  .connected-source > span {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 10px;
  }

  .connected-source > span > span {
    display: grid;
    gap: 3px;
  }

  .connected-source small {
    color: var(--muted);
    font-size: 10px;
  }

  .key-form,
  .symbol-form {
    display: grid;
    gap: 8px;
    padding: 14px;
  }

  .symbol-form {
    border-bottom: 1px solid var(--border);
  }

  .key-form label,
  .symbol-form label {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .key-form > div,
  .symbol-form > div {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
  }

  .key-form input,
  .symbol-form input {
    width: 100%;
    min-width: 0;
    min-height: 44px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--bg);
    padding: 0 12px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .key-form input::placeholder,
  .symbol-form input::placeholder {
    color: var(--muted);
  }

  .watchlist-panel > header > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .quote-list {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .quote-card {
    min-width: 0;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .quote-card:nth-child(2n) {
    border-right: 0;
  }

  .quote-card:nth-last-child(-n + 2) {
    border-bottom: 0;
  }

  .quote-card:nth-last-child(2):nth-child(2n) {
    border-bottom: 1px solid var(--border);
  }

  .quote-card > header {
    display: flex;
    min-height: 62px;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 10px 9px 14px;
    border-bottom: 1px solid var(--border);
  }

  .quote-card > header > div {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .quote-card > header strong {
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 600;
    letter-spacing: 0.02em;
  }

  .quote-card > header span {
    overflow: hidden;
    color: var(--muted);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .quote-card > header > .quote-card-actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 2px;
  }

  .quote-card-link {
    padding-inline: 9px;
    font-size: 9px;
    letter-spacing: 0.04em;
  }

  .quote-card > header button {
    width: 44px;
    height: 44px;
    min-height: 44px;
    flex: 0 0 44px;
    padding: 0;
  }

  .quote-primary {
    display: flex;
    min-height: 96px;
    align-items: end;
    justify-content: space-between;
    gap: 16px;
    padding: 18px 14px;
  }

  .quote-primary > strong {
    overflow-wrap: anywhere;
    font-family: var(--font-mono);
    font-size: clamp(22px, 3vw, 32px);
    font-weight: 550;
    letter-spacing: -0.02em;
    line-height: 1.1;
  }

  .quote-primary > span {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .quote-primary > span[data-tone="negative"] {
    color: var(--danger);
  }

  .quote-primary > span[data-tone="neutral"] {
    color: var(--muted);
  }

  .quote-card dl {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    margin: 0;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
  }

  .quote-card dl > div {
    min-width: 0;
    display: grid;
    gap: 5px;
    padding: 10px;
    border-right: 1px solid var(--border);
  }

  .quote-card dl > div:last-child {
    border-right: 0;
  }

  .quote-card dt {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .quote-card dd {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .quote-card footer {
    display: flex;
    flex-wrap: wrap;
    gap: 5px 12px;
    padding: 9px 14px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 8px;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  .quote-card footer time {
    margin-left: auto;
  }

  .quote-unavailable {
    display: grid;
    min-height: 150px;
    place-content: center;
    gap: 5px;
    padding: 20px;
    color: var(--muted);
    text-align: center;
    font-family: var(--font-mono);
  }

  .quote-unavailable span {
    color: var(--fg);
    font-size: 11px;
  }

  .quote-unavailable small {
    font-size: 9px;
  }

  .trading-empty,
  .trading-loading {
    display: grid;
    min-height: 280px;
    place-content: center;
    justify-items: center;
    padding: 28px;
    color: var(--muted);
    text-align: center;
  }

  .trading-empty h3 {
    margin: 14px 0 5px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 550;
  }

  .trading-empty p {
    max-width: 46ch;
    margin: 0;
    font-size: 11px;
    line-height: 1.6;
  }

  .trading-loading {
    width: min(420px, 100%);
    margin: auto;
    gap: 10px;
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .trading-loading div {
    width: 100%;
    height: 8px;
    background: color-mix(in oklch, var(--fg) 10%, transparent);
  }

  .trading-loading div:nth-child(3) {
    width: 76%;
  }

  .trading-loading div:nth-child(4) {
    width: 52%;
  }

  :focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }

  @media (max-width: 900px) {
    .trading-header {
      align-items: stretch;
      flex-direction: column;
    }

    .quote-list {
      grid-template-columns: 1fr;
    }

    .quote-card,
    .quote-card:nth-child(2n),
    .quote-card:nth-last-child(-n + 2),
    .quote-card:nth-last-child(2):nth-child(2n) {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }

    .quote-card:last-child {
      border-bottom: 0;
    }
  }

  @media (max-width: 620px) {
    .trading-header-actions,
    .source-grid,
    .key-form > div,
    .symbol-form > div {
      grid-template-columns: 1fr;
    }

    .trading-header-actions {
      display: grid;
    }

    .source-grid {
      display: grid;
    }

    .source-grid > div {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }

    .source-grid > div:last-child {
      border-bottom: 0;
    }

    .connected-source {
      align-items: stretch;
      flex-direction: column;
    }

    .connected-source button {
      width: 100%;
    }

    .quote-primary {
      align-items: start;
      flex-direction: column;
    }

    .quote-card dl {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .quote-card dl > div:nth-child(2) {
      border-right: 0;
    }

    .quote-card dl > div:nth-child(-n + 2) {
      border-bottom: 1px solid var(--border);
    }

    .quote-card footer time {
      width: 100%;
      margin-left: 0;
    }
  }
</style>

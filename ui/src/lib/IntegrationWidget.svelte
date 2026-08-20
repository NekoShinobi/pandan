<script lang="ts">
  import Check from "lucide-svelte/icons/check";
  import X from "lucide-svelte/icons/x";
  import { onMount } from "svelte";
  import {
    fetchWidgetCapabilities,
    fetchWidgetData,
    updateDashboardWidgetConfig,
    type DashboardWidget,
    type WidgetData,
    type WidgetDataItem,
  } from "$lib/api";

  let {
    widget,
    onUpdate,
    onToast,
    onOpenCalendarDate,
  }: {
    widget: DashboardWidget;
    onUpdate: (widget: DashboardWidget) => void;
    onToast: (message: string) => void;
    onOpenCalendarDate: (date: string) => void;
  } = $props();

  const remoteKinds = new Set([
    "youtube",
    "rss",
    "reddit",
    "stocks",
    "releases",
    "streams",
  ]);
  const dataKinds = new Set([...remoteKinds, "bible-verse"]);
  const calendarWeekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

  type CalendarMonthDay = {
    key: string;
    label: string;
    dayNumber: number;
    currentMonth: boolean;
    today: boolean;
    eventCount: number;
  };

  let data = $state.raw<WidgetData | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  /** A reload with provider data already on screen, which must not blank the widget. */
  let refreshing = $state(false);
  let saving = $state(false);
  let formError = $state("");
  let secretStorageEnabled = $state(false);
  let configDialog = $state<HTMLDialogElement>();
  let listPrimary = $state("");
  let listSecondary = $state("");
  let textPrimary = $state("");
  let textSecondary = $state("");
  let selectValue = $state("");
  let toggleValue = $state(false);
  let secretValue = $state("");
  let clearSecret = $state(false);
  let now = $state(new Date());

  let isRemote = $derived(remoteKinds.has(widget.kind));
  let isDataBacked = $derived(dataKinds.has(widget.kind));
  let isConfigured = $derived.by(() => {
    const config = widget.config;
    switch (widget.kind) {
      case "youtube":
        return hasList(config.channels) || hasList(config.playlists);
      case "rss":
        return hasList(config.urls);
      case "reddit":
        return hasText(config.subreddit);
      case "stocks":
        return hasList(config.symbols);
      case "clock":
        return hasList(config.timezones);
      case "iframe":
        return hasText(config.url);
      case "html":
        return hasText(config.source);
      case "releases":
        return hasList(config.repositories);
      case "streams":
        return hasList(config.channels);
      default:
        return true;
    }
  });
  let visibleItems = $derived(
    data?.items.slice(
      0,
      widget.size === "compact" ? 3 : widget.size === "standard" ? 5 : 10,
    ) ?? [],
  );
  let configuredTimezones = $derived(
    stringArray(widget.config.timezones).slice(0, 8),
  );
  let calendarEvents = $derived(
    stringArray(widget.config.events)
      .map(parseCalendarEvent)
      .filter(
        (event): event is { date: string; title: string } => event !== null,
      )
      .sort((a, b) => a.date.localeCompare(b.date)),
  );
  let calendarMonthDays = $derived(buildCalendarMonthDays(now, calendarEvents));
  let calendarMonthLabel = $derived(
    new Intl.DateTimeFormat("en", {
      month: "long",
      year: "numeric",
    }).format(now),
  );
  let calendarMonthEventCount = $derived(
    calendarMonthDays
      .filter((day) => day.currentMonth)
      .reduce((count, day) => count + day.eventCount, 0),
  );
  let htmlDocument = $derived(
    `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width"><style>html{color-scheme:dark}body{margin:0;padding:18px;background:oklch(0.2 0.01 155);color:oklch(0.94 0.01 155);font:15px/1.55 ui-sans-serif,system-ui}a{color:inherit}</style>${String(widget.config.source ?? "")}`,
  );

  onMount(() => {
    const timer = window.setInterval(() => {
      now = new Date();
      if (
        widget.kind === "bible-verse" &&
        data?.items[0]?.published_at !== now.toISOString().slice(0, 10)
      ) {
        void loadData();
      }
    }, 30_000);
    if (isDataBacked && isConfigured) void loadData();
    return () => window.clearInterval(timer);
  });

  function captureConfigDialog(node: HTMLDialogElement) {
    configDialog = node;
    return () => {
      configDialog = undefined;
    };
  }

  function stringArray(value: unknown): string[] {
    return Array.isArray(value)
      ? value.filter((item): item is string => typeof item === "string")
      : [];
  }

  function hasList(value: unknown) {
    return stringArray(value).some((item) => item.trim().length > 0);
  }

  function hasText(value: unknown) {
    return typeof value === "string" && value.trim().length > 0;
  }

  function lines(value: unknown) {
    return stringArray(value).join("\n");
  }

  function splitLines(value: string) {
    return value
      .split(/[\n,]/)
      .map((item) => item.trim())
      .filter(Boolean);
  }

  function isValidTimezone(value: string) {
    try {
      new Intl.DateTimeFormat("en", { timeZone: value }).format();
      return true;
    } catch {
      return false;
    }
  }

  function parseCalendarEvent(value: string) {
    const [date, ...title] = value.split("|");
    const normalizedDate = date?.trim();
    const normalizedTitle = title.join("|").trim();
    return /^\d{4}-\d{2}-\d{2}$/.test(normalizedDate) && normalizedTitle
      ? { date: normalizedDate, title: normalizedTitle }
      : null;
  }

  function calendarDateKey(date: Date) {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function buildCalendarMonthDays(
    reference: Date,
    events: Array<{ date: string; title: string }>,
  ): CalendarMonthDay[] {
    const firstOfMonth = new Date(
      reference.getFullYear(),
      reference.getMonth(),
      1,
    );
    const firstCell = new Date(
      reference.getFullYear(),
      reference.getMonth(),
      1 - firstOfMonth.getDay(),
    );
    const todayKey = calendarDateKey(reference);

    return Array.from({ length: 42 }, (_, index) => {
      const date = new Date(
        firstCell.getFullYear(),
        firstCell.getMonth(),
        firstCell.getDate() + index,
      );
      const key = calendarDateKey(date);
      const eventCount = events.filter((event) => event.date === key).length;
      return {
        key,
        label: new Intl.DateTimeFormat("en", {
          weekday: "long",
          month: "long",
          day: "numeric",
          year: "numeric",
        }).format(date),
        dayNumber: date.getDate(),
        currentMonth: date.getMonth() === reference.getMonth(),
        today: key === todayKey,
        eventCount,
      };
    });
  }

  function formatRelative(value: string | number | undefined) {
    if (value === undefined || value === "") return "";
    const date =
      typeof value === "number" ? new Date(value * 1_000) : new Date(value);
    if (Number.isNaN(date.getTime())) return "";
    const minutes = Math.round((date.getTime() - now.getTime()) / 60_000);
    const formatter = new Intl.RelativeTimeFormat("en", { numeric: "auto" });
    if (Math.abs(minutes) < 60) return formatter.format(minutes, "minute");
    const hours = Math.round(minutes / 60);
    if (Math.abs(hours) < 24) return formatter.format(hours, "hour");
    return formatter.format(Math.round(hours / 24), "day");
  }

  function formatClock(timezone: string) {
    try {
      return new Intl.DateTimeFormat("en", {
        timeZone: timezone,
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      }).format(now);
    } catch {
      return "Invalid zone";
    }
  }

  function openConfig() {
    const config = widget.config;
    listPrimary = "";
    listSecondary = "";
    textPrimary = "";
    textSecondary = "";
    selectValue = "";
    toggleValue = false;
    secretValue = "";
    clearSecret = false;
    formError = "";
    switch (widget.kind) {
      case "youtube":
        listPrimary = lines(config.channels);
        listSecondary = lines(config.playlists);
        toggleValue = config.include_shorts === true;
        break;
      case "rss":
        listPrimary = lines(config.urls);
        break;
      case "reddit":
        textPrimary = String(config.subreddit ?? "");
        textSecondary = String(config.client_id ?? "");
        selectValue = String(config.sort ?? "hot");
        break;
      case "stocks":
        listPrimary = lines(config.symbols);
        break;
      case "calendar":
        listPrimary = lines(config.events);
        break;
      case "clock":
        listPrimary = lines(config.timezones);
        break;
      case "iframe":
        textPrimary = String(config.url ?? "");
        break;
      case "html":
        textPrimary = String(config.source ?? "");
        break;
      case "releases":
        listPrimary = lines(config.repositories);
        break;
      case "streams":
        listPrimary = lines(config.channels);
        selectValue = String(config.platform ?? "twitch");
        textSecondary = String(config.client_id ?? "");
        break;
    }
    void fetchWidgetCapabilities()
      .then(
        (capabilities) =>
          (secretStorageEnabled = capabilities.secret_storage_enabled),
      )
      .catch(() => (secretStorageEnabled = false));
    configDialog?.showModal();
  }

  function buildConfig(): Record<string, unknown> {
    switch (widget.kind) {
      case "youtube":
        return {
          channels: splitLines(listPrimary),
          playlists: splitLines(listSecondary),
          include_shorts: toggleValue,
          limit: 20,
        };
      case "rss":
        return { urls: splitLines(listPrimary), limit: 24 };
      case "reddit":
        return {
          subreddit: textPrimary.trim(),
          sort: selectValue || "hot",
          client_id: textSecondary.trim(),
          limit: 20,
        };
      case "stocks":
        return {
          symbols: splitLines(listPrimary).map((symbol) =>
            symbol.toUpperCase(),
          ),
        };
      case "calendar":
        return { events: splitLines(listPrimary) };
      case "clock":
        return { timezones: splitLines(listPrimary).slice(0, 8) };
      case "iframe":
        return { url: textPrimary.trim() };
      case "html":
        return { source: textPrimary };
      case "releases":
        return { repositories: splitLines(listPrimary) };
      case "streams":
        return {
          channels: splitLines(listPrimary),
          platform: selectValue || "twitch",
          client_id: textSecondary.trim(),
        };
      default:
        return {};
    }
  }

  async function saveConfig(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    if (widget.kind === "clock") {
      const zones = splitLines(listPrimary);
      if (zones.length > 8) {
        formError = "A clock widget can track up to eight timezones.";
        return;
      }
      const invalid = zones.find((zone) => !isValidTimezone(zone));
      if (invalid) {
        formError = `${invalid} is not a valid IANA timezone.`;
        return;
      }
    }
    saving = true;
    formError = "";
    try {
      const updated = await updateDashboardWidgetConfig(widget.id, {
        config: buildConfig(),
        ...(secretValue.trim() ? { secret: secretValue.trim() } : {}),
        clear_secret: clearSecret,
      });
      onUpdate(updated);
      configDialog?.close();
      onToast("Widget configuration saved");
      if (remoteKinds.has(updated.kind)) await loadData(true);
    } catch (reason: unknown) {
      formError =
        reason instanceof Error
          ? reason.message
          : "Configuration was not saved";
    } finally {
      saving = false;
    }
  }

  /**
   * Loads provider data. Only the first load has nothing to show; a refresh keeps the
   * last good response on screen and reports a failure beside it rather than replacing
   * it with an error card.
   */
  async function loadData(refresh = false) {
    if (data) refreshing = true;
    else loading = true;
    loadError = "";
    try {
      data = await fetchWidgetData(widget.id, refresh);
    } catch (reason: unknown) {
      loadError =
        reason instanceof Error
          ? reason.message
          : "Provider data is unavailable";
    } finally {
      loading = false;
      refreshing = false;
    }
  }

  function openItem(item: WidgetDataItem) {
    return item.url && item.url !== "#" ? item.url : item.comments_url;
  }
</script>

<div class="integration-widget">
  <div class="integration-head">
    <span class="widget-kicker">
      {widget.kind === "youtube"
        ? "YouTube uploads"
        : widget.kind === "rss"
          ? "RSS / Atom"
          : widget.kind === "reddit"
            ? "Reddit"
            : widget.kind === "stocks"
              ? "Markets"
              : widget.kind === "releases"
                ? "Releases"
                : widget.kind === "streams"
                  ? "Live channels"
                  : widget.kind === "bible-verse"
                    ? "Daily verse"
                    : widget.kind}
    </span>
    <div class="integration-actions">
      {#if isRemote && isConfigured}
        <button
          class="ui-button ui-button--ghost text-button"
          type="button"
          disabled={loading || refreshing}
          onclick={() => loadData(true)}
          >{loading || refreshing ? "Loading…" : "Refresh"}</button
        >
      {/if}
      {#if widget.kind !== "bible-verse"}
        <button
          class="ui-button ui-button--ghost text-button"
          type="button"
          onclick={openConfig}>Configure</button
        >
      {/if}
    </div>
  </div>

  {#if loadError && data}
    <p class="integration-stale" role="status">
      Showing the last response · {loadError}
    </p>
  {/if}

  {#if !isConfigured}
    <button class="integration-empty" type="button" onclick={openConfig}>
      <strong>Connect this widget</strong>
      <span>Add its sources and display preferences.</span>
    </button>
  {:else if widget.kind === "clock"}
    <div class="clock-grid">
      {#each configuredTimezones.slice(0, widget.size === "compact" ? 3 : 8) as timezone (timezone)}
        <div class="clock-zone">
          <strong class="mono">{formatClock(timezone)}</strong>
          <span>{timezone.replaceAll("_", " ")}</span>
        </div>
      {/each}
    </div>
  {:else if widget.kind === "calendar"}
    <div class="calendar-layout">
      <div class="calendar-today">
        <span
          >{new Intl.DateTimeFormat("en", { month: "long" }).format(now)}</span
        >
        <strong class="mono">{now.getDate()}</strong>
        <small
          >{new Intl.DateTimeFormat("en", { weekday: "long" }).format(
            now,
          )}</small
        >
      </div>
      <div class="integration-list calendar-events">
        {#each calendarEvents.slice(0, widget.size === "compact" ? 2 : 6) as event (event.date + event.title)}
          <div class="integration-row">
            <span class="mono">{event.date.slice(5)}</span>
            <strong>{event.title}</strong>
          </div>
        {:else}
          <p class="integration-note">No personal events added.</p>
        {/each}
      </div>
      <section
        class="calendar-month"
        aria-label={`${calendarMonthLabel} calendar`}
        data-od-id={`calendar-month-${widget.id}`}
      >
        <header class="calendar-month-header">
          <strong>{calendarMonthLabel}</strong>
          <span>
            {calendarMonthEventCount}
            {calendarMonthEventCount === 1 ? "event" : "events"}
          </span>
        </header>
        <div
          class="calendar-month-grid"
          role="group"
          aria-label={`${calendarMonthLabel} dates`}
        >
          {#each calendarWeekdays as weekday (weekday)}
            <span class="calendar-month-weekday" aria-hidden="true">{weekday}</span>
          {/each}
          {#each calendarMonthDays as day (day.key)}
            <button
              class={[
                "calendar-month-day",
                !day.currentMonth && "is-outside",
                day.today && "is-today",
                day.eventCount > 0 && "has-events",
              ]}
              type="button"
              onclick={() => onOpenCalendarDate(day.key)}
              aria-current={day.today ? "date" : undefined}
              aria-label={`${day.label}${day.eventCount ? `, ${day.eventCount} ${day.eventCount === 1 ? "event" : "events"}` : ""}`}
              data-od-id={`calendar-day-${widget.id}-${day.key}`}
            >
              <time datetime={day.key}>{day.dayNumber}</time>
              {#if day.eventCount > 0}
                <span class="calendar-event-marker" aria-hidden="true"></span>
              {/if}
            </button>
          {/each}
        </div>
      </section>
    </div>
  {:else if widget.kind === "iframe"}
    <iframe
      class="custom-frame"
      title="Custom dashboard frame"
      src={String(widget.config.url)}
      sandbox="allow-forms allow-popups allow-scripts"
      referrerpolicy="no-referrer"
    ></iframe>
  {:else if widget.kind === "html"}
    <iframe
      class="custom-frame"
      title="Custom HTML widget"
      srcdoc={htmlDocument}
      sandbox=""
      referrerpolicy="no-referrer"
    ></iframe>
  {:else if loading && !data}
    <div class="integration-loading" aria-live="polite">
      Loading provider data…
    </div>
  {:else if loadError && !data}
    <div class="integration-error" role="status">
      <strong>Could not load this source</strong>
      <span>{loadError}</span>
      <button
        class="ui-button ui-button--ghost text-button"
        type="button"
        onclick={() => loadData(true)}>Try again</button
      >
    </div>
  {:else if widget.kind === "bible-verse"}
    <figure class="bible-verse-card">
      <blockquote>{data?.items[0]?.title ?? "No verse available."}</blockquote>
      <figcaption>
        <strong>{data?.items[0]?.source ?? ""}</strong>
        <span>{data?.items[0]?.version ?? "English Revised Version"}</span>
      </figcaption>
    </figure>
  {:else if widget.kind === "youtube"}
    <div class="video-grid">
      {#each visibleItems as item (item.url)}
        <!-- eslint-disable svelte/no-navigation-without-resolve -- provider-supplied external URL -->
        <a
          class="video-card"
          href={openItem(item)}
          target="_blank"
          rel="noreferrer"
        >
          {#if item.thumbnail}
            <img
              src={item.thumbnail}
              alt=""
              loading="lazy"
              referrerpolicy="no-referrer"
            />
          {/if}
          <span>
            <strong>{item.title}</strong>
            <small>{item.source} · {formatRelative(item.published_at)}</small>
          </span>
        </a>
        <!-- eslint-enable svelte/no-navigation-without-resolve -->
      {/each}
    </div>
  {:else if widget.kind === "stocks"}
    <div class="market-grid">
      {#each visibleItems as item (item.symbol)}
        <!-- eslint-disable svelte/no-navigation-without-resolve -- provider-supplied external URL -->
        <a
          class="market-quote"
          href={item.url}
          target="_blank"
          rel="noreferrer"
        >
          <span class="mono">{item.symbol}</span>
          <strong class="mono">{item.value?.toLocaleString() ?? "—"}</strong>
          <small class:item-positive={(item.change ?? 0) >= 0}>
            {item.change === undefined
              ? "No change data"
              : `${item.change >= 0 ? "+" : ""}${item.change.toFixed(2)}%`}
          </small>
        </a>
        <!-- eslint-enable svelte/no-navigation-without-resolve -->
      {/each}
    </div>
  {:else if widget.kind === "streams"}
    <div class="stream-grid">
      {#each visibleItems as item (item.url)}
        <!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- provider-supplied external URL -->
        <a class="stream-row" href={item.url} target="_blank" rel="noreferrer">
          <span class:item-live={item.live} class="stream-state"
            >{item.live ? "Live" : "Offline"}</span
          >
          <span>
            <strong>{item.title}</strong>
            <small>{item.category ?? "No current category"}</small>
          </span>
          <span class="mono">{item.viewers?.toLocaleString() ?? ""}</span>
        </a>
      {/each}
    </div>
  {:else}
    <div class="integration-list">
      {#each visibleItems as item (item.url ?? item.title)}
        <!-- eslint-disable svelte/no-navigation-without-resolve -- provider-supplied external URL -->
        <a
          class="integration-row"
          href={openItem(item)}
          target="_blank"
          rel="noreferrer"
        >
          <span>
            <strong>{item.title}</strong>
            <small>
              {item.version ??
                item.source ??
                item.provider ??
                formatRelative(item.published_at)}
            </small>
          </span>
          {#if widget.kind === "reddit"}
            <span class="mono row-stat">{item.score ?? 0} ↑</span>
          {:else}
            <span aria-hidden="true">↗</span>
          {/if}
        </a>
        <!-- eslint-enable svelte/no-navigation-without-resolve -->
      {/each}
    </div>
  {/if}
</div>

<dialog
  class="settings-dialog integration-dialog"
  {@attach captureConfigDialog}
  onclick={(event) => event.target === configDialog && configDialog.close()}
  data-od-id={`configure-widget-${widget.id}`}
>
  <div class="settings-heading">
    <div>
      <h2>Configure {widget.kind}</h2>
      <p>Settings apply only to this widget.</p>
    </div>
    <button
      class="ui-button ui-button--ghost ui-button--icon dialog-close"
      type="button"
      aria-label="Close widget configuration"
      onclick={() => configDialog?.close()}
      ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
    >
  </div>
  <form class="settings-form integration-form" onsubmit={saveConfig}>
    <div class="settings-form-scroll integration-form-fields">
      {#if widget.kind === "youtube"}
        <label for={`channels-${widget.id}`}>Channel IDs</label>
        <textarea id={`channels-${widget.id}`} bind:value={listPrimary} rows="4"
        ></textarea>
        <p class="field-note">
          One UC… channel ID per line. Uploads-only feeds exclude Shorts by
          default, matching Glance.
        </p>
        <label for={`playlists-${widget.id}`}>Playlist IDs</label>
        <textarea
          id={`playlists-${widget.id}`}
          bind:value={listSecondary}
          rows="3"></textarea>
        <button class="ui-toggle-button switch-row" type="button" aria-pressed={toggleValue} onclick={() => (toggleValue = !toggleValue)}>
          <span class="ui-toggle-indicator" aria-hidden="true">{#if toggleValue}<Check size={13} />{/if}</span>
          <span>Include Shorts when channel feeds support them</span>
        </button>
      {:else if widget.kind === "rss"}
        <label for={`feeds-${widget.id}`}>Feed URLs</label>
        <textarea
          id={`feeds-${widget.id}`}
          bind:value={listPrimary}
          rows="6"
          placeholder="https://example.com/feed.xml"></textarea>
        <p class="field-note">
          HTTPS RSS and Atom feeds only. Private-network destinations and
          redirects are blocked.
        </p>
      {:else if widget.kind === "reddit"}
        <label for={`subreddit-${widget.id}`}>Subreddit</label>
        <input
          id={`subreddit-${widget.id}`}
          bind:value={textPrimary}
          placeholder="selfhosted"
        />
        <label for={`reddit-sort-${widget.id}`}>Sort</label>
        <select id={`reddit-sort-${widget.id}`} bind:value={selectValue}>
          <option value="hot">Hot</option>
          <option value="new">New</option>
          <option value="top">Top</option>
          <option value="rising">Rising</option>
        </select>
        <label for={`reddit-client-${widget.id}`}
          >Reddit app client ID (optional)</label
        >
        <input id={`reddit-client-${widget.id}`} bind:value={textSecondary} />
      {:else if widget.kind === "stocks"}
        <label for={`symbols-${widget.id}`}>Market symbols</label>
        <textarea
          id={`symbols-${widget.id}`}
          bind:value={listPrimary}
          rows="5"
          placeholder="SPY&#10;BTC-USD&#10;AAPL"></textarea>
      {:else if widget.kind === "calendar"}
        <label for={`events-${widget.id}`}>Personal events</label>
        <textarea
          id={`events-${widget.id}`}
          bind:value={listPrimary}
          rows="7"
          placeholder="2026-08-21 | Project review"></textarea>
        <p class="field-note">One event per line using YYYY-MM-DD | Title.</p>
      {:else if widget.kind === "clock"}
        <label for={`zones-${widget.id}`}>IANA timezones</label>
        <textarea
          id={`zones-${widget.id}`}
          bind:value={listPrimary}
          rows="5"
          placeholder="America/New_York&#10;Europe/London"></textarea>
        <p class="field-note">
          One per line, up to eight. Each clock widget keeps its own list.
        </p>
      {:else if widget.kind === "iframe"}
        <label for={`frame-${widget.id}`}>HTTPS source URL</label>
        <input
          id={`frame-${widget.id}`}
          type="url"
          bind:value={textPrimary}
          placeholder="https://example.com/embed"
        />
        <p class="field-note">
          Embedded without same-origin access. The destination may still block
          framing.
        </p>
      {:else if widget.kind === "html"}
        <label for={`html-${widget.id}`}>HTML source</label>
        <textarea
          class="integration-code-editor"
          id={`html-${widget.id}`}
          bind:value={textPrimary}
          rows="10"
          placeholder="<p>Private dashboard note</p>"></textarea>
        <p class="field-note">
          Rendered in an isolated sandbox. Scripts, navigation, and parent-page
          access are disabled.
        </p>
      {:else if widget.kind === "releases"}
        <label for={`repos-${widget.id}`}>Repositories</label>
        <textarea
          id={`repos-${widget.id}`}
          bind:value={listPrimary}
          rows="7"
          placeholder="github:glanceapp/glance&#10;gitlab:owner/project&#10;codeberg:forgejo/forgejo&#10;gitea@git.example.com:owner/project"
        ></textarea>
        <p class="field-note">
          Supports GitHub, GitLab, Codeberg, and public HTTPS Gitea or Forgejo
          instances.
        </p>
      {:else if widget.kind === "streams"}
        <label for={`stream-provider-${widget.id}`}>Platform</label>
        <select id={`stream-provider-${widget.id}`} bind:value={selectValue}>
          <option value="twitch">Twitch</option>
          <option value="kick">Kick</option>
        </select>
        <label for={`stream-channels-${widget.id}`}>Channel names</label>
        <textarea
          id={`stream-channels-${widget.id}`}
          bind:value={listPrimary}
          rows="6"></textarea>
        {#if selectValue === "twitch"}
          <label for={`twitch-client-${widget.id}`}>Twitch client ID</label>
          <input id={`twitch-client-${widget.id}`} bind:value={textSecondary} />
        {/if}
      {/if}

      {#if widget.kind === "reddit" || widget.kind === "releases" || (widget.kind === "streams" && selectValue === "twitch")}
        <label for={`secret-${widget.id}`}>
          {widget.kind === "reddit"
            ? "Reddit app secret"
            : widget.kind === "streams"
              ? "Twitch client secret"
              : "Provider API token (optional)"}
        </label>
        <input
          id={`secret-${widget.id}`}
          type="password"
          bind:value={secretValue}
          disabled={!secretStorageEnabled}
          autocomplete="new-password"
          placeholder={widget.has_secret ? "Stored — enter to replace" : ""}
        />
        {#if !secretStorageEnabled}
          <p class="field-note">
            Set PANDAN_SECRET_KEY on the server to enable encrypted
            credential storage.
          </p>
        {:else if widget.has_secret}
          <button class="ui-toggle-button switch-row" type="button" aria-pressed={clearSecret} onclick={() => (clearSecret = !clearSecret)}>
            <span class="ui-toggle-indicator" aria-hidden="true">{#if clearSecret}<Check size={13} />{/if}</span>
            <span>Remove the stored credential</span>
          </button>
        {/if}
      {/if}

      {#if formError}<p class="form-error" role="alert">{formError}</p>{/if}
    </div>
    <div class="settings-actions">
      <button
        class="ui-button ui-button--secondary secondary-btn"
        type="button"
        onclick={() => configDialog?.close()}>Cancel</button
      >
      <button
        class="ui-button ui-button--primary primary-btn"
        type="submit"
        disabled={saving}>{saving ? "Saving…" : "Save configuration"}</button
      >
    </div>
  </form>
</dialog>

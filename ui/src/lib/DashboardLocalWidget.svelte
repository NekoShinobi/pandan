<script lang="ts">
  import ArrowRight from "lucide-svelte/icons/arrow-right";
  import BookmarkIcon from "lucide-svelte/icons/bookmark";
  import CalendarDays from "lucide-svelte/icons/calendar-days";
  import ChevronLeft from "lucide-svelte/icons/chevron-left";
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import Plus from "lucide-svelte/icons/plus";
  import {
    bookmarkFaviconUrl,
    type Bookmark,
    type DashboardWidget,
  } from "$lib/api";

  type DashboardClock = {
    timezone: string;
    hourAngle: number;
    minuteAngle: number;
    secondAngle: number;
    label: string;
  };

  type DashboardCalendarDay = {
    key: string;
    day: number;
    currentMonth: boolean;
    today: boolean;
  };

  type DashboardCalendarEventSummary = {
    count: number;
    colors: string[];
  };

  let {
    widget,
    firstName,
    clocks,
    clockMarks,
    dateLabel,
    calendarMonthLabel,
    calendarWeekdays,
    calendarDays,
    calendarEventsByDate,
    bookmarks,
    onShowCurrentMonth,
    onChangeCalendarMonth,
    onOpenCalendarDate,
    onManageBookmarks,
  }: {
    widget: DashboardWidget;
    firstName: string;
    clocks: DashboardClock[];
    clockMarks: number[];
    dateLabel: string;
    calendarMonthLabel: string;
    calendarWeekdays: readonly string[];
    calendarDays: DashboardCalendarDay[];
    calendarEventsByDate: Record<string, DashboardCalendarEventSummary>;
    bookmarks: Bookmark[];
    onShowCurrentMonth: () => void;
    onChangeCalendarMonth: (offset: number) => void;
    onOpenCalendarDate: (date: string) => void;
    onManageBookmarks: () => void;
  } = $props();

  function bookmarkHost(value: string) {
    try {
      return new URL(value).hostname.replace(/^www\./, "");
    } catch {
      return value;
    }
  }

  function hideBrokenBookmarkFavicon(event: Event) {
    (event.currentTarget as HTMLImageElement).hidden = true;
  }

  function configuredSectionLabel() {
    const label = widget.config.label;
    return typeof label === "string" && label.trim()
      ? label.trim()
      : "Untitled section";
  }

  function configuredTitle(fallback: string) {
    const title = widget.config.title;
    return typeof title === "string" && title.trim() ? title.trim() : fallback;
  }

  function configuredClockStyle() {
    return widget.config.clock_style === "digital" ? "digital" : "analog";
  }

  function configuredDividerStyle() {
    return widget.config.line_style === "dashed" ||
      widget.config.line_style === "dotted"
      ? widget.config.line_style
      : "solid";
  }

</script>

{#if widget.kind === "welcome"}
  <section
    class="dashboard-welcome-widget"
    data-od-id={`dashboard-welcome-${widget.id}`}
  >
    <h2>welcome:{firstName}</h2>
    {#if widget.config.show_status !== false}
      <span>$ dashboard status --widgets</span>
    {/if}
  </section>
{:else if widget.kind === "local-time"}
  <section
    class="dashboard-local-widget utility-analog-clock"
    data-od-id={`dashboard-local-time-${widget.id}`}
  >
    <header class="dashboard-widget-heading">
      <h2 data-od-id={`dashboard-local-time-title-${widget.id}`}>
        {configuredTitle("Local time")}
      </h2>
    </header>
    <div
      class="utility-clock-list"
      aria-label="Saved local times"
      data-od-id="dashboard-local-times"
    >
      {#each clocks as clock, index (clock.timezone)}
        <div
          class={["utility-clock-row", `is-${configuredClockStyle()}`]}
          data-od-id={`dashboard-local-time-${index + 1}`}
        >
          {#if configuredClockStyle() === "analog"}
            <div
              class="analog-clock"
              role="img"
              aria-label={`${clock.label} in ${clock.timezone}`}
            >
              {#each clockMarks as mark (mark)}
                <i
                  class="analog-clock-mark"
                  style:--mark-angle={`${mark * 30}deg`}
                ></i>
              {/each}
              <i
                class="analog-clock-hand is-hour"
                style:--hand-angle={`${clock.hourAngle}deg`}
              ></i>
              <i
                class="analog-clock-hand is-minute"
                style:--hand-angle={`${clock.minuteAngle}deg`}
              ></i>
              <i
                class="analog-clock-hand is-second"
                style:--hand-angle={`${clock.secondAngle}deg`}
              ></i>
              <i class="analog-clock-pin"></i>
            </div>
          {/if}
          <span class="utility-clock-copy">
            <strong>{clock.label}</strong>
            <small title={clock.timezone}>{clock.timezone}</small>
          </span>
        </div>
      {/each}
    </div>
  </section>
{:else if widget.kind === "calendar-overview"}
  <section
    class="dashboard-local-widget utility-calendar"
    data-od-id={`dashboard-calendar-${widget.id}`}
  >
    <header class="dashboard-widget-heading">
      <h2 data-od-id={`dashboard-calendar-title-${widget.id}`}>
        {configuredTitle("Calendar")}
      </h2>
    </header>
    <div class="utility-calendar-date">
      <div class="utility-calendar-date-copy">
        <strong>{calendarMonthLabel}</strong>
        <span>{dateLabel}</span>
      </div>
      <div
        class="utility-calendar-navigation"
        role="group"
        aria-label="Navigate dashboard calendar months"
        data-od-id="dashboard-calendar-navigation"
      >
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          aria-label="Show current month"
          onclick={onShowCurrentMonth}
          data-od-id="dashboard-calendar-today"
        >
          <CalendarDays size={15} strokeWidth={1.8} aria-hidden="true" />
        </button>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          aria-label="Previous month"
          onclick={() => onChangeCalendarMonth(-1)}
          data-od-id="dashboard-calendar-previous"
        >
          <ChevronLeft size={15} strokeWidth={1.8} aria-hidden="true" />
        </button>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          aria-label="Next month"
          onclick={() => onChangeCalendarMonth(1)}
          data-od-id="dashboard-calendar-next"
        >
          <ChevronRight size={15} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </div>
    </div>
    <div
      class="utility-calendar-grid"
      aria-label={`${calendarMonthLabel} calendar`}
      data-od-id="dashboard-calendar-month"
    >
      {#each calendarWeekdays as weekday, index (`${weekday}-${index}`)}
        <span class="utility-calendar-weekday" aria-hidden="true"
          >{weekday}</span
        >
      {/each}
      {#each calendarDays as day (day.key)}
        {@const eventSummary = calendarEventsByDate[day.key]}
        <button
          class={[
            "utility-calendar-day",
            !day.currentMonth && "is-outside",
            day.today && "is-today",
          ]}
          type="button"
          onclick={() => onOpenCalendarDate(day.key)}
          aria-label={`${day.key}, ${eventSummary?.count ?? 0} ${eventSummary?.count === 1 ? "calendar item" : "calendar items"}`}
          aria-current={day.today ? "date" : undefined}
          data-od-id={`dashboard-calendar-day-${day.key}`}
        >
          <time datetime={day.key}>{day.day}</time>
          {#if eventSummary && widget.config.show_event_markers !== false}
            <span class="utility-calendar-event-dots" aria-hidden="true">
              {#each eventSummary.colors as color (color)}
                <i style:--event-color={color}></i>
              {/each}
            </span>
          {/if}
        </button>
      {/each}
    </div>
  </section>
{:else if widget.kind === "bookmarks"}
  <section
    class="dashboard-local-widget utility-bookmarks"
    data-od-id={`dashboard-bookmarks-${widget.id}`}
  >
    <header class="dashboard-widget-heading utility-bookmarks-head">
      <h2 data-od-id={`dashboard-bookmarks-title-${widget.id}`}>
        {configuredTitle("Bookmarks")}
      </h2>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Manage bookmarks"
        onclick={onManageBookmarks}
        data-od-id="manage-dashboard-bookmarks"
      >
        <Plus size={16} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </header>
    {#if bookmarks.length > 0}
      <div
        class="utility-bookmark-list overlay-scroll-region"
        aria-label="Saved bookmarks"
      >
        {#each bookmarks as bookmark (bookmark.id)}
          <!-- eslint-disable svelte/no-navigation-without-resolve -- user-saved external destination -->
          <a
            class="utility-bookmark-row"
            href={bookmark.url}
            target="_blank"
            rel="noreferrer"
            data-od-id={`dashboard-bookmark-${bookmark.id}`}
          >
            <span class="bookmark-favicon" aria-hidden="true">
              <BookmarkIcon size={15} strokeWidth={1.8} />
              {#if bookmark.has_favicon}
                <img
                  src={bookmarkFaviconUrl(bookmark.id)}
                  alt=""
                  onerror={hideBrokenBookmarkFavicon}
                />
              {/if}
            </span>
            <span class="utility-bookmark-copy">
              <strong>{bookmark.title}</strong>
              {#if widget.config.show_hostnames !== false}
                <small>{bookmarkHost(bookmark.url)}</small>
              {/if}
            </span>
            <ArrowRight size={14} strokeWidth={1.8} aria-hidden="true" />
          </a>
          <!-- eslint-enable svelte/no-navigation-without-resolve -->
        {/each}
      </div>
    {:else}
      <button
        class="dashboard-bookmark-empty"
        type="button"
        onclick={onManageBookmarks}
        data-od-id="add-first-dashboard-bookmark"
      >
        <strong>No saved links</strong>
        <span>Add a bookmark to keep it close.</span>
      </button>
    {/if}
  </section>
{:else if widget.kind === "section-header"}
  <div
    class="dashboard-section-widget"
    data-od-id={`dashboard-section-header-${widget.id}`}
  >
    <strong data-od-id={`dashboard-section-label-${widget.id}`}
      >{configuredSectionLabel()}</strong
    >
  </div>
{:else if widget.kind === "divider"}
  <div
    class={["dashboard-line-widget", `is-${configuredDividerStyle()}`]}
    role="separator"
    aria-label="Dashboard divider"
    data-od-id={`dashboard-line-divider-${widget.id}`}
  >
    <span></span>
  </div>
{/if}

<style>
  .dashboard-local-widget {
    min-width: 0;
    min-height: 100%;
    display: flex;
    flex-direction: column;
  }

  .dashboard-widget-heading {
    min-width: 0;
    min-height: 32px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .dashboard-widget-heading h2 {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .dashboard-welcome-widget {
    min-height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 8px;
  }

  .dashboard-welcome-widget h2 {
    max-width: 20ch;
    font-family: var(--font-mono);
    font-size: clamp(30px, 3.6vw, 50px);
    font-weight: 520;
    letter-spacing: -0.045em;
    line-height: 1;
    text-wrap: balance;
  }

  .dashboard-welcome-widget > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.02em;
  }

  .dashboard-bookmark-empty {
    min-height: 116px;
    display: grid;
    place-content: center;
    gap: 5px;
    border: 1px dashed var(--border);
    background: transparent;
    color: var(--fg);
    text-align: center;
  }

  .dashboard-bookmark-empty:hover {
    border-color: var(--fg);
    background: var(--fg-soft);
  }

  .dashboard-bookmark-empty span {
    color: var(--muted);
    font-size: 11px;
  }

  .dashboard-section-widget {
    min-height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-items: center;
  }

  .dashboard-section-widget strong {
    min-width: 0;
    color: var(--fg);
    font: 590 17px/1.2 var(--font-mono);
    text-wrap: balance;
  }

  .dashboard-line-widget {
    min-height: 100%;
    display: grid;
    align-items: center;
  }

  .dashboard-line-widget span {
    width: 100%;
    height: 1px;
    display: block;
    background: var(--border);
  }

  .dashboard-line-widget.is-dashed span {
    height: 0;
    border-top: 1px dashed var(--border);
    background: transparent;
  }

  .dashboard-line-widget.is-dotted span {
    height: 0;
    border-top: 2px dotted var(--border);
    background: transparent;
  }

  .utility-clock-row.is-digital {
    grid-template-columns: minmax(0, 1fr);
    padding-block: 12px;
  }

  .utility-clock-row.is-digital .utility-clock-copy {
    gap: 4px;
  }

  .utility-clock-row.is-digital .utility-clock-copy strong {
    font-size: clamp(24px, 8cqi, 40px);
    letter-spacing: -0.05em;
  }

  :global(.widget-kind-welcome .widget-content),
  :global(.widget-kind-local-time .widget-content),
  :global(.widget-kind-calendar-overview .widget-content),
  :global(.widget-kind-bookmarks .widget-content),
  :global(.widget-kind-section-header .widget-content),
  :global(.widget-kind-divider .widget-content) {
    height: 100%;
  }

  :global(.widget-kind-section-header .widget-content) {
    padding-block: 8px;
  }

  :global(.widget-kind-divider) {
    border-color: transparent;
    background: transparent;
  }

  :global(.widget-kind-divider .widget-content) {
    padding: 0 8px;
    overflow: hidden;
  }

  :global(.widget-kind-divider.is-editing) {
    border-color: var(--accent);
  }

</style>

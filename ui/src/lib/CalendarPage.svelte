<script lang="ts">
  import CalendarPlus from "lucide-svelte/icons/calendar-plus";
  import ChevronLeft from "lucide-svelte/icons/chevron-left";
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import MapPin from "lucide-svelte/icons/map-pin";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import {
    createCalendarSubscription,
    deleteCalendarSubscription,
    fetchCalendar,
    refreshCalendarSubscription,
    type CalendarColor,
    type CalendarEvent,
    type CalendarResponse,
    type CalendarSubscription,
  } from "$lib/api";

  interface CalendarDay {
    key: string;
    date: Date;
    day: number;
    inMonth: boolean;
    today: boolean;
  }

  const defaultColor: CalendarColor = "#2DD4BF";
  const colorOptions: Array<{ value: CalendarColor; label: string }> = [
    { value: "#2DD4BF", label: "Signal teal" },
    { value: "#60A5FA", label: "Terminal blue" },
    { value: "#A78BFA", label: "Violet" },
    { value: "#FB7185", label: "Rose" },
    { value: "#FB923C", label: "Orange" },
    { value: "#FBBF24", label: "Amber" },
    { value: "#A3E635", label: "Lime" },
    { value: "#94A3B8", label: "Slate" },
  ];
  const weekdays = ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
  const birthdaySubscriptionId = "contacts-birthdays";

  let calendar = $state.raw<CalendarResponse>({ subscriptions: [], events: [] });
  let loading = $state(true);
  let pageError = $state("");
  let cursor = $state(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  let selectedDate = $state(dateKey(new Date()));
  let hiddenCalendars = $state<string[]>([]);
  let dialog = $state<HTMLDialogElement>();
  let urlInput = $state<HTMLInputElement>();
  let calendarUrl = $state("");
  let calendarColor = $state<CalendarColor>(defaultColor);
  let colorHexInput = $state<string>(defaultColor);
  let colorHue = $state(173);
  let colorSaturation = $state(67);
  let colorLightness = $state(50);
  let colorError = $state("");
  let formError = $state("");
  let saving = $state(false);
  let busyId = $state("");
  let deleteId = $state("");

  let monthLabel = $derived(
    new Intl.DateTimeFormat("en", { month: "long", year: "numeric" }).format(cursor),
  );
  let days = $derived.by(() => buildMonth(cursor));
  let visibleEvents = $derived(
    calendar.events.filter((event) => !hiddenCalendars.includes(event.subscription_id)),
  );
  let selectedEvents = $derived(
    visibleEvents.filter((event) => eventDateKey(event) === selectedDate),
  );
  let birthdayCount = $derived(
    calendar.events.filter(
      (event) => event.subscription_id === birthdaySubscriptionId,
    ).length,
  );

  onMount(() => {
    void loadCalendar();
  });

  async function loadCalendar() {
    loading = true;
    pageError = "";
    try {
      calendar = await fetchCalendar();
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to load calendars";
    } finally {
      loading = false;
    }
  }

  function captureDialog(node: HTMLDialogElement) {
    dialog = node;
    return () => {
      dialog = undefined;
    };
  }

  function captureUrlInput(node: HTMLInputElement) {
    urlInput = node;
    return () => {
      urlInput = undefined;
    };
  }

  async function openAddCalendar() {
    calendarUrl = "";
    chooseColor(defaultColor);
    formError = "";
    dialog?.showModal();
    await tick();
    urlInput?.focus();
  }

  async function subscribe(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    if (colorError) {
      formError = "Choose a valid calendar color before subscribing.";
      return;
    }
    saving = true;
    formError = "";
    try {
      calendar = await createCalendarSubscription(calendarUrl.trim(), calendarColor);
      dialog?.close();
    } catch (reason: unknown) {
      formError = reason instanceof Error ? reason.message : "Unable to subscribe";
    } finally {
      saving = false;
    }
  }

  async function refresh(subscription: CalendarSubscription) {
    if (busyId) return;
    busyId = subscription.id;
    pageError = "";
    try {
      calendar = await refreshCalendarSubscription(subscription.id);
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to refresh calendar";
      calendar = await fetchCalendar().catch(() => calendar);
    } finally {
      busyId = "";
    }
  }

  async function remove(subscription: CalendarSubscription) {
    if (busyId) return;
    if (deleteId !== subscription.id) {
      deleteId = subscription.id;
      return;
    }
    busyId = subscription.id;
    pageError = "";
    try {
      await deleteCalendarSubscription(subscription.id);
      hiddenCalendars = hiddenCalendars.filter((id) => id !== subscription.id);
      calendar = await fetchCalendar();
      deleteId = "";
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to remove calendar";
    } finally {
      busyId = "";
    }
  }

  function toggleCalendar(id: string) {
    hiddenCalendars = hiddenCalendars.includes(id)
      ? hiddenCalendars.filter((candidate) => candidate !== id)
      : [...hiddenCalendars, id];
  }

  function changeMonth(offset: number) {
    cursor = new Date(cursor.getFullYear(), cursor.getMonth() + offset, 1);
  }

  function goToday() {
    const today = new Date();
    cursor = new Date(today.getFullYear(), today.getMonth(), 1);
    selectedDate = dateKey(today);
  }

  function eventsOn(key: string) {
    return visibleEvents.filter((event) => eventDateKey(event) === key);
  }

  function chooseColor(value: CalendarColor) {
    calendarColor = value.toUpperCase() as CalendarColor;
    colorHexInput = calendarColor;
    colorError = "";
    const hsl = hexToHsl(calendarColor);
    colorHue = hsl.hue;
    colorSaturation = hsl.saturation;
    colorLightness = hsl.lightness;
  }

  function updateColorFromSliders(
    channel: "hue" | "saturation" | "lightness",
    event: Event,
  ) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    if (channel === "hue") colorHue = value;
    else if (channel === "saturation") colorSaturation = value;
    else colorLightness = value;
    calendarColor = hslToHex(colorHue, colorSaturation, colorLightness);
    colorHexInput = calendarColor;
    colorError = "";
  }

  function updateHexColor(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    colorHexInput = input.value.toUpperCase();
    if (/^#[0-9A-F]{6}$/.test(colorHexInput)) {
      chooseColor(colorHexInput as CalendarColor);
    } else {
      colorError = "Enter a six-digit hex color such as #2DD4BF.";
    }
  }

  function hexToHsl(value: CalendarColor) {
    const red = Number.parseInt(value.slice(1, 3), 16) / 255;
    const green = Number.parseInt(value.slice(3, 5), 16) / 255;
    const blue = Number.parseInt(value.slice(5, 7), 16) / 255;
    const maximum = Math.max(red, green, blue);
    const minimum = Math.min(red, green, blue);
    const delta = maximum - minimum;
    let hue = 0;
    if (delta !== 0) {
      if (maximum === red) hue = 60 * (((green - blue) / delta) % 6);
      else if (maximum === green) hue = 60 * ((blue - red) / delta + 2);
      else hue = 60 * ((red - green) / delta + 4);
    }
    if (hue < 0) hue += 360;
    const lightness = (maximum + minimum) / 2;
    const saturation = delta === 0 ? 0 : delta / (1 - Math.abs(2 * lightness - 1));
    return {
      hue: Math.round(hue),
      saturation: Math.round(saturation * 100),
      lightness: Math.round(lightness * 100),
    };
  }

  function hslToHex(hue: number, saturation: number, lightness: number): CalendarColor {
    const normalizedSaturation = saturation / 100;
    const normalizedLightness = lightness / 100;
    const chroma =
      (1 - Math.abs(2 * normalizedLightness - 1)) * normalizedSaturation;
    const component = chroma * (1 - Math.abs(((hue / 60) % 2) - 1));
    const offset = normalizedLightness - chroma / 2;
    let red = 0;
    let green = 0;
    let blue = 0;
    if (hue < 60) [red, green] = [chroma, component];
    else if (hue < 120) [red, green] = [component, chroma];
    else if (hue < 180) [green, blue] = [chroma, component];
    else if (hue < 240) [green, blue] = [component, chroma];
    else if (hue < 300) [red, blue] = [component, chroma];
    else [red, blue] = [chroma, component];
    const channel = (value: number) =>
      Math.round((value + offset) * 255)
        .toString(16)
        .padStart(2, "0")
        .toUpperCase();
    return `#${channel(red)}${channel(green)}${channel(blue)}`;
  }

  function eventDateKey(event: CalendarEvent) {
    if (event.all_day) return event.start_at.slice(0, 10);
    const date = new Date(event.start_at);
    return Number.isNaN(date.valueOf()) ? event.start_at.slice(0, 10) : dateKey(date);
  }

  function eventTime(event: CalendarEvent) {
    if (event.all_day) return "All day";
    const date = new Date(event.start_at);
    return Number.isNaN(date.valueOf())
      ? event.start_at
      : new Intl.DateTimeFormat("en", { hour: "numeric", minute: "2-digit" }).format(date);
  }

  function selectedDateLabel() {
    const date = new Date(`${selectedDate}T12:00:00`);
    return new Intl.DateTimeFormat("en", {
      weekday: "long",
      month: "long",
      day: "numeric",
    }).format(date);
  }

  function dateKey(date: Date) {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function buildMonth(month: Date): CalendarDay[] {
    const first = new Date(month.getFullYear(), month.getMonth(), 1);
    const mondayOffset = (first.getDay() + 6) % 7;
    const start = new Date(first.getFullYear(), first.getMonth(), 1 - mondayOffset);
    const today = dateKey(new Date());
    return Array.from({ length: 42 }, (_, index) => {
      const date = new Date(start.getFullYear(), start.getMonth(), start.getDate() + index);
      const key = dateKey(date);
      return {
        key,
        date,
        day: date.getDate(),
        inMonth: date.getMonth() === month.getMonth(),
        today: key === today,
      };
    });
  }
</script>

<section class="calendar-page product-page" data-od-id="calendar-page">
  <header class="calendar-header page-header">
    <div>
      <h2>$ calendar --month</h2>
      <p>Bring public iCalendar sources into one private view.</p>
    </div>
    <button class="ui-button ui-button--primary calendar-primary" type="button" onclick={openAddCalendar}>
      <Plus size={16} strokeWidth={1.8} aria-hidden="true" /> Add calendar
    </button>
  </header>

  {#if pageError}<p class="calendar-error" role="alert">{pageError}</p>{/if}

  <div class="calendar-layout">
    <section class="month-panel" aria-label="Calendar month">
      <header class="month-toolbar">
        <div>
          <button type="button" aria-label="Previous month" onclick={() => changeMonth(-1)}><ChevronLeft size={18} /></button>
          <button type="button" aria-label="Next month" onclick={() => changeMonth(1)}><ChevronRight size={18} /></button>
          <button class="ui-button ui-button--secondary today-button" type="button" onclick={goToday}>Today</button>
        </div>
        <h3>{monthLabel}</h3>
      </header>

      <div class="weekday-row" aria-hidden="true">
        {#each weekdays as weekday (weekday)}<span>{weekday}</span>{/each}
      </div>
      <div class="month-grid">
        {#each days as day (day.key)}
          {@const dayEvents = eventsOn(day.key)}
          <button
            class:outside={!day.inMonth}
            class:today={day.today}
            class:selected={selectedDate === day.key}
            type="button"
            onclick={() => (selectedDate = day.key)}
            aria-label={`${day.date.toDateString()}, ${dayEvents.length} events`}
          >
            <span class="day-number">{day.day}</span>
            <span class="day-events">
              {#each dayEvents.slice(0, 3) as event (event.id)}
                <span class="event-pill" style:--calendar-color={event.calendar_color}>{event.title}</span>
              {/each}
              {#if dayEvents.length > 3}<small>+{dayEvents.length - 3} more</small>{/if}
            </span>
          </button>
        {/each}
      </div>
    </section>

    <aside class="calendar-sidebar">
      <section class="day-agenda">
        <span>[ DAY.AGENDA ]</span>
        <h3>{selectedDateLabel()}</h3>
        <div class="agenda-list">
          {#if loading}
            <p>Loading calendar…</p>
          {:else}
            {#each selectedEvents as event (event.id)}
              <article class="agenda-event" style:--calendar-color={event.calendar_color}>
                <div><time>{eventTime(event)}</time><small>{event.calendar_name}</small></div>
                <h4>{event.title}</h4>
                {#if event.location}<p><MapPin size={13} aria-hidden="true" /> {event.location}</p>{/if}
                {#if event.description}<p>{event.description}</p>{/if}
                {#if event.url}<a href={event.url} target="_blank" rel="noreferrer">Open event <ExternalLink size={13} /></a>{/if}
              </article>
            {:else}
              <p class="agenda-empty">No events scheduled for this day.</p>
            {/each}
          {/if}
        </div>
      </section>

      <section class="source-panel">
        <span>[ SOURCES ]</span>
        <h3>Subscribed calendars</h3>
        <div class="source-list">
          {#if birthdayCount}
            <article data-od-id="calendar-source-birthdays">
              <button
                class="source-toggle"
                class:muted={hiddenCalendars.includes(birthdaySubscriptionId)}
                style:--calendar-color="#FB7185"
                type="button"
                aria-label={`${hiddenCalendars.includes(birthdaySubscriptionId) ? "Show" : "Hide"} contact birthdays`}
                onclick={() => toggleCalendar(birthdaySubscriptionId)}
              ></button>
              <div>
                <strong>Birthdays</strong>
                <small>Annual events from Contacts</small>
              </div>
              <span class="source-static-mark">LOCAL</span>
            </article>
          {/if}
          {#each calendar.subscriptions as subscription (subscription.id)}
            <article>
              <button
                class="source-toggle"
                class:muted={hiddenCalendars.includes(subscription.id)}
                style:--calendar-color={subscription.color}
                type="button"
                aria-label={`${hiddenCalendars.includes(subscription.id) ? "Show" : "Hide"} ${subscription.name}`}
                onclick={() => toggleCalendar(subscription.id)}
              ></button>
              <div><strong>{subscription.name}</strong><small>{subscription.last_error ?? `${calendar.events.filter((event) => event.subscription_id === subscription.id).length} events`}</small></div>
              <button type="button" aria-label={`Refresh ${subscription.name}`} disabled={Boolean(busyId)} onclick={() => refresh(subscription)}>
                <RefreshCw class={busyId === subscription.id ? "spinning" : undefined} size={14} />
              </button>
              <button class="ui-button ui-button--danger ui-button--icon" class:confirm={deleteId === subscription.id} type="button" aria-label={`Remove ${subscription.name}`} disabled={Boolean(busyId)} onclick={() => remove(subscription)}>
                <Trash2 size={14} />
              </button>
            </article>
          {:else}
            <div class="source-empty"><CalendarPlus size={24} /><p>No calendars subscribed yet.</p></div>
          {/each}
        </div>
      </section>
    </aside>
  </div>

  <dialog class="settings-dialog calendar-dialog" {@attach captureDialog} onclick={(event) => event.target === dialog && dialog?.close()}>
    <header><div><span>[ CALENDAR.ADD ]</span><h2>Subscribe to .ics</h2></div><button class="ui-button ui-button--ghost ui-button--icon" type="button" aria-label="Close" onclick={() => dialog?.close()}><X size={18} /></button></header>
    <form onsubmit={subscribe}>
      <label for="calendar-url">Calendar URL</label>
      <input id="calendar-url" type="url" bind:value={calendarUrl} {@attach captureUrlInput} placeholder="https://example.com/calendar.ics" maxlength="2048" required />
      <small>Use a public HTTPS link that returns an RFC 5545 .ics calendar.</small>
      <fieldset class="calendar-color-picker" style:--calendar-color={calendarColor}>
        <legend>Calendar color</legend>
        <div class="color-picker-preview">
          <span aria-hidden="true"></span>
          <div><strong>Event color</strong><small>Presets or any six-digit hex value</small></div>
          <label for="calendar-color-hex">HEX</label>
          <input id="calendar-color-hex" class:invalid={Boolean(colorError)} type="text" value={colorHexInput} oninput={updateHexColor} maxlength="7" spellcheck="false" autocomplete="off" aria-describedby="calendar-color-help" />
        </div>
        <div class="color-presets" aria-label="Color presets">
          {#each colorOptions as option (option.value)}
            <button
              class:selected={calendarColor === option.value}
              style:--swatch-color={option.value}
              type="button"
              aria-label={option.label}
              aria-pressed={calendarColor === option.value}
              title={option.label}
              onclick={() => chooseColor(option.value)}
            ><span></span></button>
          {/each}
        </div>
        <div class="color-channels">
          <label for="calendar-color-hue"><span>Hue</span><output>{colorHue}°</output></label>
          <input id="calendar-color-hue" class="hue-channel" type="range" min="0" max="359" value={colorHue} oninput={(event) => updateColorFromSliders("hue", event)} />
          <label for="calendar-color-saturation"><span>Saturation</span><output>{colorSaturation}%</output></label>
          <input id="calendar-color-saturation" class="saturation-channel" style:--picker-hue={`${colorHue}deg`} type="range" min="0" max="100" value={colorSaturation} oninput={(event) => updateColorFromSliders("saturation", event)} />
          <label for="calendar-color-lightness"><span>Lightness</span><output>{colorLightness}%</output></label>
          <input id="calendar-color-lightness" class="lightness-channel" style:--picker-hue={`${colorHue}deg`} style:--picker-saturation={`${colorSaturation}%`} type="range" min="0" max="100" value={colorLightness} oninput={(event) => updateColorFromSliders("lightness", event)} />
        </div>
        <small id="calendar-color-help">{colorError || "Color is saved with this calendar source."}</small>
      </fieldset>
      {#if formError}<p class="calendar-form-error" role="alert">{formError}</p>{/if}
      <footer><button class="ui-button ui-button--secondary" type="button" onclick={() => dialog?.close()}>Cancel</button><button class="ui-button ui-button--primary calendar-primary" type="submit" disabled={saving}>{saving ? "Fetching…" : "Subscribe"}</button></footer>
    </form>
  </dialog>
</section>

<style>
  .calendar-page { display: grid; gap: 18px; padding: clamp(24px, 3vw, 42px); min-width: 0; }
  .calendar-header { display: flex; align-items: end; justify-content: space-between; gap: 24px; padding-bottom: 18px; border-bottom: 1px solid var(--border); }
  .calendar-sidebar section > span, .calendar-dialog header span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .09em; }
  .calendar-header h2 { margin: 8px 0 0; font-family: var(--font-mono); font-size: clamp(26px, 3vw, 42px); font-weight: 540; letter-spacing: -.04em; }
  .calendar-header p { margin: 7px 0 0; color: var(--muted); font-family: var(--font-mono); font-size: 11px; }
  button, input { font: inherit; }
  button { color: inherit; }
  .calendar-primary { display: inline-flex; min-height: 42px; align-items: center; gap: 8px; border: 1px solid var(--fg); background: var(--fg); color: var(--bg); padding: 0 16px; font-family: var(--font-mono); font-size: 11px; letter-spacing: .04em; }
  .calendar-primary:hover { background: transparent; color: var(--fg); }
  .calendar-error, .calendar-form-error { margin: 0; border: 1px solid oklch(60% .16 25 / .5); background: oklch(20% .04 25 / .75); padding: 10px 12px; color: oklch(82% .09 25); font-family: var(--font-mono); font-size: 11px; }
  .calendar-layout { display: grid; grid-template-columns: minmax(0, 1fr) minmax(260px, 320px); gap: 18px; min-height: 0; }
  .month-panel, .calendar-sidebar section { border: 1px solid var(--border); background: color-mix(in oklch, var(--page-surface, var(--surface)) 92%, transparent); }
  .month-toolbar { display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; min-height: 62px; padding: 0 16px; border-bottom: 1px solid var(--border); }
  .month-toolbar > div { display: flex; gap: 6px; }
  .month-toolbar button { min-width: 36px; height: 36px; border: 1px solid var(--border); background: transparent; }
  .month-toolbar button:hover { border-color: var(--fg); }
  .month-toolbar .today-button { width: auto; padding: 0 12px; font-family: var(--font-mono); font-size: 10px; text-transform: uppercase; letter-spacing: .08em; }
  .month-toolbar h3 { grid-column: 2; margin: 0; font-family: var(--font-mono); font-size: 15px; font-weight: 550; }
  .weekday-row, .month-grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); }
  .weekday-row { border-bottom: 1px solid var(--border); }
  .weekday-row span { padding: 9px 8px; color: var(--muted); text-align: right; font-family: var(--font-mono); font-size: 9px; letter-spacing: .08em; }
  .month-grid > button { position: relative; min-width: 0; min-height: 112px; overflow: hidden; border: 0; border-right: 1px solid var(--border); border-bottom: 1px solid var(--border); background: transparent; padding: 8px; text-align: left; }
  .month-grid > button:nth-child(7n) { border-right: 0; }
  .month-grid > button:hover, .month-grid > button.selected { background: color-mix(in oklch, var(--fg) 6%, transparent); }
  .month-grid > button.outside { color: color-mix(in oklch, var(--muted) 55%, transparent); }
  .day-number { display: grid; width: 24px; height: 24px; place-items: center; margin-left: auto; font-family: var(--font-mono); font-size: 10px; }
  .today .day-number { background: var(--fg); color: var(--bg); }
  .day-events { display: grid; gap: 4px; margin-top: 4px; }
  .event-pill { overflow: hidden; border-left: 2px solid var(--calendar-color); padding: 3px 5px; text-overflow: ellipsis; white-space: nowrap; color: var(--fg); background: color-mix(in oklch, var(--calendar-color) 14%, transparent); font-family: var(--font-mono); font-size: 9px; }
  .day-events small { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .calendar-sidebar { display: grid; align-content: start; gap: 18px; }
  .calendar-sidebar section { padding: 16px; }
  .calendar-sidebar h3 { margin: 7px 0 15px; font-family: var(--font-mono); font-size: 14px; font-weight: 550; }
  .agenda-list { display: grid; gap: 8px; max-height: 420px; overflow: auto; }
  .agenda-event { border: 1px solid var(--border); border-top: 2px solid var(--calendar-color); padding: 12px; color: var(--fg); }
  .agenda-event > div { display: flex; justify-content: space-between; gap: 10px; color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .agenda-event h4 { margin: 9px 0 5px; font-size: 13px; }
  .agenda-event p { display: flex; gap: 5px; margin: 5px 0 0; color: var(--muted); font-size: 11px; line-height: 1.5; }
  .agenda-event a { display: inline-flex; gap: 5px; margin-top: 9px; color: var(--fg); font-family: var(--font-mono); font-size: 10px; }
  .agenda-empty, .source-empty, .agenda-list > p { color: var(--muted); font-family: var(--font-mono); font-size: 11px; }
  .source-list { display: grid; gap: 7px; }
  .source-list article { display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; align-items: center; gap: 8px; border-top: 1px solid var(--border); padding-top: 9px; }
  .source-list article button { display: grid; width: 30px; height: 30px; place-items: center; border: 1px solid transparent; background: transparent; }
  .source-list article button:hover { border-color: var(--border); }
  .source-list .source-toggle { width: 10px; height: 10px; min-height: 10px; border: 0; border-radius: 50%; background: var(--calendar-color); padding: 0; }
  .source-toggle.muted { background: transparent; box-shadow: inset 0 0 0 1px var(--calendar-color); opacity: .45; }
  .source-list strong, .source-list small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--font-mono); }
  .source-list strong { font-size: 11px; font-weight: 550; }
  .source-list small { margin-top: 3px; color: var(--muted); font-size: 9px; }
  .source-static-mark { grid-column: span 2; justify-self: end; color: var(--muted); font-family: var(--font-mono); font-size: 8px; letter-spacing: .08em; }
  .source-list button.confirm { border-color: oklch(62% .19 25); color: oklch(72% .16 25); }
  .source-empty { display: grid; justify-items: start; gap: 8px; padding: 16px 0 4px; }
  :global(.spinning) { animation: calendar-spin .8s linear infinite; }
  @keyframes calendar-spin { to { transform: rotate(360deg); } }
  .calendar-dialog { width: min(520px, calc(100vw - 32px)); border: 1px solid var(--border); background: var(--page-surface, var(--surface)); color: var(--fg); padding: 0; }
  .calendar-dialog::backdrop { background: oklch(5% 0 0 / .7); backdrop-filter: blur(5px); }
  .calendar-dialog header { display: flex; align-items: center; justify-content: space-between; padding: 18px 20px; border-bottom: 1px solid var(--border); }
  .calendar-dialog h2 { margin: 6px 0 0; font-family: var(--font-mono); font-size: 20px; font-weight: 550; }
  .calendar-dialog header button { width: 36px; height: 36px; border: 1px solid var(--border); background: transparent; }
  .calendar-dialog form { display: grid; gap: 10px; padding: 20px; }
  .calendar-dialog label, .calendar-dialog legend { font-family: var(--font-mono); font-size: 10px; letter-spacing: .04em; }
  .calendar-dialog input[type="url"] { min-height: 44px; border: 1px solid var(--border); background: var(--bg); color: var(--fg); padding: 0 12px; }
  .calendar-dialog small { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .calendar-dialog fieldset { margin: 8px 0 0; border: 1px solid var(--border); padding: 14px; }
  .calendar-color-picker { display: grid; gap: 14px; }
  .color-picker-preview { display: grid; grid-template-columns: auto minmax(0, 1fr) auto 92px; align-items: center; gap: 10px; }
  .color-picker-preview > span { width: 34px; height: 34px; border: 1px solid color-mix(in oklch, var(--calendar-color) 70%, var(--fg)); background: var(--calendar-color); box-shadow: inset 0 0 0 3px var(--surface); }
  .color-picker-preview strong, .color-picker-preview small { display: block; font-family: var(--font-mono); }
  .color-picker-preview strong { font-size: 11px; font-weight: 550; }
  .color-picker-preview small { margin-top: 3px; }
  .color-picker-preview label { color: var(--muted); }
  .color-picker-preview input { min-width: 0; min-height: 36px; border: 1px solid var(--border); background: var(--bg); color: var(--fg); padding: 0 9px; font-family: var(--font-mono); font-size: 11px; text-transform: uppercase; }
  .color-picker-preview input.invalid { border-color: oklch(62% .19 25); }
  .color-presets { display: grid; grid-template-columns: repeat(8, minmax(0, 1fr)); gap: 7px; }
  .color-presets button { display: grid; min-width: 0; height: 30px; place-items: center; border: 1px solid var(--border); background: transparent; padding: 3px; }
  .color-presets button:hover, .color-presets button.selected { border-color: var(--fg); background: color-mix(in oklch, var(--fg) 7%, transparent); }
  .color-presets button.selected { box-shadow: inset 0 -2px 0 var(--fg); }
  .color-presets button span { width: 100%; height: 100%; background: var(--swatch-color); }
  .color-channels { display: grid; grid-template-columns: 84px minmax(0, 1fr); align-items: center; gap: 8px 12px; border-top: 1px solid var(--border); padding-top: 13px; }
  .color-channels label { display: flex; justify-content: space-between; gap: 8px; color: var(--muted); }
  .color-channels output { color: var(--fg); }
  .color-channels input[type="range"] { width: 100%; height: 18px; margin: 0; appearance: none; background: transparent; }
  .color-channels input[type="range"]::-webkit-slider-runnable-track { height: 6px; border: 1px solid color-mix(in oklch, var(--fg) 18%, var(--border)); background: var(--channel-background); }
  .color-channels input[type="range"]::-moz-range-track { height: 6px; border: 1px solid color-mix(in oklch, var(--fg) 18%, var(--border)); background: var(--channel-background); }
  .color-channels input[type="range"]::-webkit-slider-thumb { width: 12px; height: 18px; margin-top: -7px; appearance: none; border: 2px solid var(--surface); border-radius: 0; background: var(--fg); box-shadow: 0 0 0 1px var(--fg); }
  .color-channels input[type="range"]::-moz-range-thumb { width: 10px; height: 16px; border: 2px solid var(--surface); border-radius: 0; background: var(--fg); box-shadow: 0 0 0 1px var(--fg); }
  .hue-channel { --channel-background: linear-gradient(90deg, hsl(0 75% 55%), hsl(60 75% 55%), hsl(120 75% 45%), hsl(180 75% 45%), hsl(240 75% 60%), hsl(300 75% 55%), hsl(360 75% 55%)); }
  .saturation-channel { --channel-background: linear-gradient(90deg, hsl(var(--picker-hue) 0% 50%), hsl(var(--picker-hue) 100% 50%)); }
  .lightness-channel { --channel-background: linear-gradient(90deg, hsl(var(--picker-hue) var(--picker-saturation) 0%), hsl(var(--picker-hue) var(--picker-saturation) 50%), hsl(var(--picker-hue) var(--picker-saturation) 100%)); }
  .calendar-color-picker > small { min-height: 14px; color: var(--muted); }
  .calendar-dialog footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
  .calendar-dialog footer > button:not(.calendar-primary) { min-height: 42px; border: 1px solid var(--border); background: transparent; padding: 0 16px; }
  :focus-visible { outline: 2px solid var(--fg); outline-offset: 2px; }
  @media (max-width: 1050px) { .calendar-layout { grid-template-columns: 1fr; } .calendar-sidebar { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @media (max-width: 720px) { .calendar-header { align-items: stretch; flex-direction: column; } .calendar-primary { justify-content: center; } .calendar-sidebar { grid-template-columns: 1fr; } .month-grid > button { min-height: 72px; padding: 4px; } .event-pill { width: 6px; height: 6px; border: 0; border-radius: 50%; background: var(--calendar-color); padding: 0; font-size: 0; } .day-events { display: flex; } .day-events small { display: none; } .weekday-row span { text-align: center; } .color-picker-preview { grid-template-columns: auto minmax(0, 1fr); } .color-picker-preview > label { grid-column: 1; } .color-picker-preview input { grid-column: 2; } .color-presets { grid-template-columns: repeat(4, minmax(0, 1fr)); } .color-channels { grid-template-columns: 1fr; } }
  @media (prefers-reduced-motion: reduce) { :global(.spinning) { animation: none; } }
</style>

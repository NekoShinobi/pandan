<script lang="ts">
  import ChevronLeft from "lucide-svelte/icons/chevron-left";
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import Ellipsis from "lucide-svelte/icons/ellipsis";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import MapPin from "lucide-svelte/icons/map-pin";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick, untrack } from "svelte";
  import PandanColorPicker from "$lib/components/PandanColorPicker.svelte";
  import { motionPopover } from "$lib/motion.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    PAYMENT_CALENDAR_COLOR,
    PAYMENT_CALENDAR_SOURCE_ID,
    paymentDateKeysBetween,
  } from "$lib/paymentSchedule";
  import {
    createCalendarSubscription,
    deleteCalendarSubscription,
    fetchCalendar,
    fetchPaymentSubscriptions,
    refreshCalendarSubscription,
    updateCalendarSubscription,
    type CalendarColor,
    type CalendarDisplayMode,
    type CalendarEvent,
    type CalendarResponse,
    type CalendarSubscription,
    type PaymentSubscription,
    type Task,
    type UserSettings,
  } from "$lib/api";

  interface CalendarDay {
    key: string;
    date: Date;
    day: number;
    inMonth: boolean;
    today: boolean;
    weekday: string | null;
  }

  interface CalendarItem {
    id: string;
    sourceId: string;
    sourceName: string;
    color: string;
    title: string;
    description: string;
    location: string;
    url: string;
    dateKey: string;
    timeLabel: string;
    kind: "event" | "task" | "birthday" | "payment";
    completed: boolean;
    task?: Task;
    contactId?: string;
  }

  let {
    tasks = [],
    onEditTask = () => {},
    onOpenContact = () => {},
    initialDate = null,
    onInitialDateHandled = () => {},
    weekStart = "sunday",
  }: {
    tasks?: Task[];
    onEditTask?: (task: Task) => void;
    onOpenContact?: (contactId: string) => void;
    initialDate?: string | null;
    onInitialDateHandled?: () => void;
    weekStart?: UserSettings["calendar_week_start"];
  } = $props();

  const defaultColor: CalendarColor = "#2DD4BF";
  const birthdaySubscriptionId = "contacts-birthdays";
  const taskSourceId = "tasks-due";
  const taskColor = "var(--accent)";

  const initialCalendarDate =
    untrack(() => parseDateKey(initialDate)) ?? new Date();

  let calendar = $state.raw<CalendarResponse>({
    subscriptions: [],
    events: [],
  });
  let paymentSubscriptions = $state.raw<PaymentSubscription[]>([]);
  let loading = $state(true);
  let pageError = $state("");
  let cursor = $state(
    new Date(
      initialCalendarDate.getFullYear(),
      initialCalendarDate.getMonth(),
      1,
    ),
  );
  let selectedDate = $state(dateKey(initialCalendarDate));
  let hiddenCalendars = $state<string[]>([]);
  let dialog = $state<HTMLDialogElement>();
  let nameInput = $state<HTMLInputElement>();
  let urlInput = $state<HTMLInputElement>();
  let editingSubscription = $state.raw<CalendarSubscription | null>(null);
  let calendarName = $state("");
  let calendarUrl = $state("");
  let calendarColor = $state<CalendarColor>(defaultColor);
  let calendarDisplayMode = $state<CalendarDisplayMode>("full");
  let colorError = $state("");
  let formError = $state("");
  let saving = $state(false);
  let busyId = $state("");
  let busyKind = $state<"refresh" | "delete" | "">("");
  let deleteId = $state("");
  let sourceMenuId = $state("");

  let monthLabel = $derived(
    new Intl.DateTimeFormat("en", { month: "long", year: "numeric" }).format(
      cursor,
    ),
  );
  let days = $derived.by(() => buildMonth(cursor, weekStart));
  let paymentItems = $derived.by((): CalendarItem[] => {
    const firstDay = days[0]?.key;
    const lastDay = days[days.length - 1]?.key;
    if (!firstDay || !lastDay) return [];
    return paymentSubscriptions.flatMap((subscription) =>
      paymentDateKeysBetween(subscription, firstDay, lastDay).map(
        (paymentDate) => ({
          id: `subscription-payment-${subscription.id}-${paymentDate}`,
          sourceId: PAYMENT_CALENDAR_SOURCE_ID,
          sourceName: "Subscriptions",
          color: PAYMENT_CALENDAR_COLOR,
          title: subscription.service,
          description: subscription.description || subscription.frequency,
          location: "",
          url: "",
          dateKey: paymentDate,
          timeLabel: "Payment due",
          kind: "payment" as const,
          completed: false,
        }),
      ),
    );
  });
  let calendarItems = $derived.by((): CalendarItem[] => [
    ...tasks
      .filter((task) => task.due_date !== null)
      .map((task) => ({
        id: `task-${task.id}`,
        sourceId: taskSourceId,
        sourceName: "Tasks",
        color: taskColor,
        title: task.title,
        description: task.description,
        location: "",
        url: "",
        dateKey: task.due_date ?? "",
        timeLabel: "Due",
        kind: "task" as const,
        completed: task.completed,
        task,
      })),
    ...paymentItems,
    ...calendar.events.map((event) => ({
      id: event.id,
      sourceId: event.subscription_id,
      sourceName: event.calendar_name,
      color: event.calendar_color,
      title: event.title,
      description: event.description,
      location: event.location,
      url: event.url,
      dateKey: eventDateKey(event),
      timeLabel: eventTime(event),
      kind:
        event.subscription_id === birthdaySubscriptionId
          ? ("birthday" as const)
          : ("event" as const),
      completed: false,
      contactId: birthdayContactId(event),
    })),
  ]);
  let visibleEvents = $derived(
    calendarItems.filter((item) => !hiddenCalendars.includes(item.sourceId)),
  );
  let selectedEvents = $derived(
    visibleEvents.filter((item) => item.dateKey === selectedDate),
  );
  let dotCalendarIds = $derived.by(
    () =>
      new Set(
        calendar.subscriptions
          .filter((subscription) => subscription.display_mode === "dot")
          .map((subscription) => subscription.id),
      ),
  );
  let birthdayCount = $derived(
    calendar.events.filter(
      (event) => event.subscription_id === birthdaySubscriptionId,
    ).length,
  );
  let taskDueCount = $derived(
    tasks.filter((task) => task.due_date !== null).length,
  );

  onMount(() => {
    if (initialDate) onInitialDateHandled();
    void loadCalendar();
  });

  async function loadCalendar() {
    loading = true;
    pageError = "";
    try {
      const [nextCalendar, nextPaymentSubscriptions] = await Promise.all([
        fetchCalendar(),
        fetchPaymentSubscriptions().catch(() => []),
      ]);
      calendar = nextCalendar;
      paymentSubscriptions = nextPaymentSubscriptions;
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error ? reason.message : "Unable to load calendars";
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

  function captureNameInput(node: HTMLInputElement) {
    nameInput = node;
    return () => {
      nameInput = undefined;
    };
  }

  async function openAddCalendar() {
    editingSubscription = null;
    calendarName = "";
    calendarUrl = "";
    calendarDisplayMode = "full";
    chooseColor(defaultColor);
    formError = "";
    dialog?.showModal();
    await tick();
    urlInput?.focus();
  }

  async function openEditCalendar(subscription: CalendarSubscription) {
    if (busyId) return;
    sourceMenuId = "";
    editingSubscription = subscription;
    calendarName = subscription.name;
    calendarUrl = subscription.url;
    calendarDisplayMode = subscription.display_mode;
    chooseColor(subscription.color);
    formError = "";
    deleteId = "";
    dialog?.showModal();
    await tick();
    nameInput?.focus();
  }

  async function saveCalendar(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    if (colorError) {
      formError = "Choose a valid calendar color before saving.";
      return;
    }
    if (editingSubscription && !calendarName.trim()) {
      formError = "Enter a calendar name.";
      return;
    }
    saving = true;
    formError = "";
    try {
      calendar = editingSubscription
        ? await updateCalendarSubscription(editingSubscription.id, {
            url: calendarUrl.trim(),
            name: calendarName.trim(),
            color: calendarColor,
            display_mode: calendarDisplayMode,
          })
        : await createCalendarSubscription(calendarUrl.trim(), calendarColor);
      dialog?.close();
    } catch (reason: unknown) {
      formError =
        reason instanceof Error
          ? reason.message
          : editingSubscription
            ? "Unable to update calendar"
            : "Unable to subscribe";
    } finally {
      saving = false;
    }
  }

  async function refresh(subscription: CalendarSubscription) {
    if (busyId) return;
    sourceMenuId = "";
    deleteId = "";
    busyId = subscription.id;
    busyKind = "refresh";
    pageError = "";
    try {
      calendar = await refreshCalendarSubscription(subscription.id);
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error ? reason.message : "Unable to refresh calendar";
      calendar = await fetchCalendar().catch(() => calendar);
    } finally {
      busyId = "";
      busyKind = "";
    }
  }

  async function remove(subscription: CalendarSubscription) {
    if (busyId) return;
    if (deleteId !== subscription.id) {
      deleteId = subscription.id;
      return;
    }
    sourceMenuId = "";
    busyId = subscription.id;
    busyKind = "delete";
    pageError = "";
    try {
      await deleteCalendarSubscription(subscription.id);
      hiddenCalendars = hiddenCalendars.filter((id) => id !== subscription.id);
      calendar = await fetchCalendar();
      deleteId = "";
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error ? reason.message : "Unable to remove calendar";
    } finally {
      busyId = "";
      busyKind = "";
    }
  }

  function toggleSourceMenu(subscriptionId: string) {
    if (busyId) return;
    sourceMenuId = sourceMenuId === subscriptionId ? "" : subscriptionId;
    deleteId = "";
  }

  function closeSourceMenuOnFocusOut(
    event: FocusEvent,
    subscriptionId: string,
  ) {
    const anchor = event.currentTarget;
    const nextTarget = event.relatedTarget;
    if (
      anchor instanceof HTMLElement &&
      nextTarget instanceof Node &&
      anchor.contains(nextTarget)
    ) {
      return;
    }
    if (sourceMenuId === subscriptionId) {
      sourceMenuId = "";
      deleteId = "";
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || !sourceMenuId) return;
    const activeMenuId = sourceMenuId;
    sourceMenuId = "";
    deleteId = "";
    void tick().then(() => {
      document
        .getElementById(`calendar-source-menu-trigger-${activeMenuId}`)
        ?.focus();
    });
  }

  function handleWindowPointerdown(event: PointerEvent) {
    const target = event.target;
    if (!sourceMenuId) return;
    if (
      target instanceof Element &&
      target.closest(`[data-calendar-source-menu-root="${sourceMenuId}"]`)
    ) {
      return;
    }
    sourceMenuId = "";
    deleteId = "";
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
    return visibleEvents.filter((item) => item.dateKey === key);
  }

  function birthdayContactId(event: CalendarEvent) {
    if (event.subscription_id !== birthdaySubscriptionId) return undefined;
    return /^contact-birthday-(.+)-\d{4}$/.exec(event.id)?.[1];
  }

  function openCalendarItem(item: CalendarItem) {
    if (item.kind === "task" && item.task) {
      onEditTask(item.task);
    } else if (item.kind === "birthday" && item.contactId) {
      onOpenContact(item.contactId);
    }
  }

  function chooseColor(value: CalendarColor) {
    calendarColor = value.toUpperCase() as CalendarColor;
    colorError = "";
  }

  function eventDateKey(event: CalendarEvent) {
    if (event.all_day) return event.start_at.slice(0, 10);
    const date = new Date(event.start_at);
    return Number.isNaN(date.valueOf())
      ? event.start_at.slice(0, 10)
      : dateKey(date);
  }

  function eventTime(event: CalendarEvent) {
    if (event.all_day) return "All day";
    const date = new Date(event.start_at);
    return Number.isNaN(date.valueOf())
      ? event.start_at
      : new Intl.DateTimeFormat("en", {
          hour: "numeric",
          minute: "2-digit",
        }).format(date);
  }

  function selectedDateLabel() {
    const date = new Date(`${selectedDate}T12:00:00`);
    return new Intl.DateTimeFormat("en", {
      weekday: "long",
      month: "long",
      day: "numeric",
    }).format(date);
  }

  function parseDateKey(value: string | null | undefined) {
    if (!value || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return null;
    const date = new Date(`${value}T12:00:00`);
    return Number.isNaN(date.valueOf()) || dateKey(date) !== value
      ? null
      : date;
  }

  function dateKey(date: Date) {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function buildMonth(
    month: Date,
    firstWeekday: UserSettings["calendar_week_start"],
  ): CalendarDay[] {
    const first = new Date(month.getFullYear(), month.getMonth(), 1);
    const offset =
      firstWeekday === "sunday" ? first.getDay() : (first.getDay() + 6) % 7;
    const start = new Date(first.getFullYear(), first.getMonth(), 1 - offset);
    const today = dateKey(new Date());
    const weekdayLabels =
      firstWeekday === "sunday"
        ? ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"]
        : ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
    return Array.from({ length: 42 }, (_, index) => {
      const date = new Date(
        start.getFullYear(),
        start.getMonth(),
        start.getDate() + index,
      );
      const key = dateKey(date);
      return {
        key,
        date,
        day: date.getDate(),
        inMonth: date.getMonth() === month.getMonth(),
        today: key === today,
        weekday: index < 7 ? weekdayLabels[index] : null,
      };
    });
  }
</script>

{#snippet agendaEventSummary(event: CalendarItem)}
  <div><time>{event.timeLabel}</time><small>{event.sourceName}</small></div>
  <h4>{event.title}</h4>
  {#if event.location}<p>
      <MapPin size={13} aria-hidden="true" />
      {event.location}
    </p>{/if}
  {#if event.description}<p>{event.description}</p>{/if}
{/snippet}

<svelte:window
  onkeydown={handleWindowKeydown}
  onpointerdown={handleWindowPointerdown}
/>

<section class="calendar-page product-page" data-od-id="calendar-page">
  <header class="calendar-header page-header">
    <div>
      <TypedHeading text="$ calendar --month" odId="calendar-heading" />
      <p>
        See tasks, subscription payments, birthdays, and iCalendar sources in
        one private view.
      </p>
    </div>
    <button
      class="ui-button ui-button--primary calendar-primary"
      type="button"
      onclick={openAddCalendar}
    >
      <Plus size={16} strokeWidth={1.8} aria-hidden="true" /> Add calendar
    </button>
  </header>

  {#if pageError}<p class="calendar-error" role="alert">{pageError}</p>{/if}

  <div class="calendar-layout">
    <section class="month-panel" aria-label="Calendar month">
      <header class="month-toolbar">
        <div class="month-heading" data-od-id="calendar-month-heading">
          <h3>{monthLabel}</h3>
        </div>
        <div
          class="month-navigation"
          role="group"
          aria-label="Navigate calendar months"
          data-od-id="calendar-month-navigation"
        >
          <button
            class="ui-button ui-button--secondary today-button"
            type="button"
            onclick={goToday}
            data-od-id="calendar-today">Today</button
          >
          <button
            class="ui-button ui-button--ghost ui-button--icon month-shift"
            type="button"
            aria-label="Previous month"
            onclick={() => changeMonth(-1)}
            data-od-id="calendar-previous-month"
            ><ChevronLeft
              size={17}
              strokeWidth={1.8}
              aria-hidden="true"
            /></button
          >
          <button
            class="ui-button ui-button--ghost ui-button--icon month-shift"
            type="button"
            aria-label="Next month"
            onclick={() => changeMonth(1)}
            data-od-id="calendar-next-month"
            ><ChevronRight
              size={17}
              strokeWidth={1.8}
              aria-hidden="true"
            /></button
          >
        </div>
      </header>
      <div class="month-grid">
        {#each days as day (day.key)}
          {@const dayEvents = eventsOn(day.key)}
          {@const listedDayEvents = dayEvents.filter(
            (event) => !dotCalendarIds.has(event.sourceId),
          )}
          {@const dottedDayEvents = dayEvents.filter((event) =>
            dotCalendarIds.has(event.sourceId),
          )}
          <button
            class:outside={!day.inMonth}
            class:today={day.today}
            class:selected={selectedDate === day.key}
            type="button"
            onclick={() => (selectedDate = day.key)}
            aria-label={`${day.date.toDateString()}, ${dayEvents.length} calendar items`}
          >
            <span class="day-number">{day.day}</span>
            {#if day.weekday}<span class="day-weekday" aria-hidden="true"
                >{day.weekday}</span
              >{/if}
            <span class="day-events overlay-scroll-region">
              {#each listedDayEvents as event (event.id)}
                <span
                  class="event-pill"
                  class:completed={event.completed}
                  style:--calendar-color={event.color}>{event.title}</span
                >
              {/each}
              {#if dottedDayEvents.length > 0}
                <span
                  class="event-dots"
                  aria-label={`${dottedDayEvents.length} dot-only calendar ${dottedDayEvents.length === 1 ? "event" : "events"}`}
                >
                  {#each dottedDayEvents as event (event.id)}
                    <span
                      class="event-dot"
                      style:--calendar-color={event.color}
                      title={`${event.sourceName}: ${event.title}`}
                      aria-hidden="true"
                    ></span>
                  {/each}
                </span>
              {/if}
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
          {#if loading}<p>Loading subscribed calendars…</p>{/if}
          {#each selectedEvents as event (event.id)}
            {#if (event.kind === "task" && event.task) || (event.kind === "birthday" && event.contactId)}
              <button
                class="agenda-event agenda-event-action"
                class:completed={event.completed}
                style:--calendar-color={event.color}
                type="button"
                onclick={() => openCalendarItem(event)}
                data-od-id={`calendar-open-${event.kind}-${event.id}`}
              >
                {@render agendaEventSummary(event)}
                <span class="agenda-event-action-label">
                  {event.kind === "task" ? "Open task" : "Open contact"}
                  <ChevronRight
                    size={13}
                    strokeWidth={1.8}
                    aria-hidden="true"
                  />
                </span>
              </button>
            {:else}
              <article
                class="agenda-event"
                class:completed={event.completed}
                style:--calendar-color={event.color}
              >
                {@render agendaEventSummary(event)}
                {#if event.url}<a
                    href={event.url}
                    target="_blank"
                    rel="noreferrer">Open event <ExternalLink size={13} /></a
                  >{/if}
              </article>
            {/if}
          {:else}
            {#if !loading}<p class="agenda-empty">
                No events or tasks due this day.
              </p>{/if}
          {/each}
        </div>
      </section>

      <section class="source-panel">
        <span>[ SOURCES ]</span>
        <h3>Calendars &amp; local dates</h3>
        <div class="source-list">
          <article data-od-id="calendar-source-tasks">
            <button
              class="source-toggle"
              class:muted={hiddenCalendars.includes(taskSourceId)}
              style:--calendar-color={taskColor}
              type="button"
              aria-label={`${hiddenCalendars.includes(taskSourceId) ? "Show" : "Hide"} task due dates`}
              onclick={() => toggleCalendar(taskSourceId)}
            ></button>
            <div>
              <strong>Tasks</strong><small
                >{taskDueCount}
                {taskDueCount === 1 ? "dated task" : "dated tasks"}</small
              >
            </div>
            <span class="source-static-mark">LOCAL</span>
          </article>
          {#if paymentSubscriptions.length > 0}
            <article data-od-id="calendar-source-subscription-payments">
              <button
                class="source-toggle"
                class:muted={hiddenCalendars.includes(
                  PAYMENT_CALENDAR_SOURCE_ID,
                )}
                style:--calendar-color={PAYMENT_CALENDAR_COLOR}
                type="button"
                aria-label={`${hiddenCalendars.includes(PAYMENT_CALENDAR_SOURCE_ID) ? "Show" : "Hide"} subscription payment dates`}
                onclick={() => toggleCalendar(PAYMENT_CALENDAR_SOURCE_ID)}
              ></button>
              <div>
                <strong>Subscriptions</strong>
                <small>
                  {paymentSubscriptions.length}
                  {paymentSubscriptions.length === 1 ? "service" : "services"}
                </small>
              </div>
              <span class="source-static-mark">LOCAL</span>
            </article>
          {/if}
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
              <div>
                <strong>{subscription.name}</strong><small
                  >{subscription.last_error ??
                    `${calendar.events.filter((event) => event.subscription_id === subscription.id).length} events`}</small
                >
              </div>
              <div
                class={[
                  "source-action-menu",
                  sourceMenuId === subscription.id && "is-open",
                ]}
                data-calendar-source-menu-root={subscription.id}
                onfocusout={(event) =>
                  closeSourceMenuOnFocusOut(event, subscription.id)}
              >
                <button
                  class="source-action-trigger"
                  id={`calendar-source-menu-trigger-${subscription.id}`}
                  type="button"
                  aria-label={busyId === subscription.id
                    ? busyKind === "refresh"
                      ? `Refreshing ${subscription.name}`
                      : `Deleting ${subscription.name}`
                    : `More actions for ${subscription.name}`}
                  aria-haspopup="menu"
                  aria-expanded={sourceMenuId === subscription.id}
                  aria-controls={`calendar-source-menu-${subscription.id}`}
                  disabled={Boolean(busyId)}
                  onclick={() => toggleSourceMenu(subscription.id)}
                  data-od-id={`calendar-actions-${subscription.id}`}
                >
                  {#if busyId === subscription.id}
                    <RefreshCw
                      class="spinning"
                      size={16}
                      strokeWidth={1.8}
                      aria-hidden="true"
                    />
                  {:else}
                    <Ellipsis size={18} strokeWidth={1.8} aria-hidden="true" />
                  {/if}
                </button>
                <div
                  class="source-action-popover"
                  id={`calendar-source-menu-${subscription.id}`}
                  role="menu"
                  aria-label={`${subscription.name} actions`}
                  aria-hidden={sourceMenuId !== subscription.id}
                  inert={sourceMenuId !== subscription.id}
                  data-od-id={`calendar-menu-${subscription.id}`}
                  {@attach motionPopover(sourceMenuId === subscription.id, {
                    closedY: -6,
                  })}
                >
                  <button
                    type="button"
                    role="menuitem"
                    onclick={() => openEditCalendar(subscription)}
                    data-od-id={`calendar-edit-${subscription.id}`}
                  >
                    <Pencil size={15} strokeWidth={1.8} aria-hidden="true" />
                    Edit
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onclick={() => refresh(subscription)}
                    data-od-id={`calendar-refresh-${subscription.id}`}
                  >
                    <RefreshCw size={15} strokeWidth={1.8} aria-hidden="true" />
                    Refresh
                  </button>
                  <button
                    class={[
                      "source-menu-delete",
                      deleteId === subscription.id && "is-armed",
                    ]}
                    type="button"
                    role="menuitem"
                    aria-label={deleteId === subscription.id
                      ? `Confirm removal of ${subscription.name}`
                      : `Delete ${subscription.name}`}
                    onclick={() => remove(subscription)}
                    data-od-id={`calendar-delete-${subscription.id}`}
                  >
                    <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
                    {deleteId === subscription.id ? "Confirm delete" : "Delete"}
                  </button>
                </div>
              </div>
            </article>
          {/each}
        </div>
      </section>
    </aside>
  </div>

  <dialog
    class="settings-dialog calendar-dialog"
    {@attach captureDialog}
    onclick={(event) => event.target === dialog && dialog?.close()}
  >
    <header>
      <div>
        <span
          >{editingSubscription
            ? "[ CALENDAR.EDIT ]"
            : "[ CALENDAR.ADD ]"}</span
        >
        <h2>{editingSubscription ? "Edit calendar" : "Subscribe to .ics"}</h2>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close"
        onclick={() => dialog?.close()}><X size={18} /></button
      >
    </header>
    <form onsubmit={saveCalendar}>
      {#if editingSubscription}
        <label for="calendar-name">Calendar name</label>
        <input
          id="calendar-name"
          class="calendar-text-input"
          type="text"
          bind:value={calendarName}
          {@attach captureNameInput}
          maxlength="120"
          autocomplete="off"
          required
        />
        <small>This name stays in Pandan when the source refreshes.</small>
      {/if}
      <label for="calendar-url">Calendar URL</label>
      <input
        id="calendar-url"
        class="calendar-text-input"
        type="url"
        bind:value={calendarUrl}
        {@attach captureUrlInput}
        placeholder="https://example.com/calendar.ics"
        maxlength="2048"
        required
      />
      <small
        >{editingSubscription
          ? "Changing the URL validates and replaces the cached event snapshot."
          : "Use a public HTTPS link that returns an RFC 5545 .ics calendar."}</small
      >
      <PandanColorPicker
        id="calendar-color"
        label="Calendar color"
        value={calendarColor}
        helpText="Color is saved with this calendar source."
        onchange={(value) => chooseColor(value as CalendarColor)}
        onvaliditychange={(valid) => {
          colorError = valid ? "" : "Enter a valid calendar color.";
        }}
      />
      {#if editingSubscription}
        <fieldset class="calendar-display-editor">
          <legend>Month view</legend>
          <button
            class="ui-toggle-button calendar-dialog-toggle"
            type="button"
            aria-pressed={calendarDisplayMode === "full"}
            aria-label={`Full calendar listing: ${calendarDisplayMode === "full" ? "on" : "off"}`}
            disabled={saving}
            onclick={() =>
              (calendarDisplayMode =
                calendarDisplayMode === "full" ? "dot" : "full")}
            data-od-id="calendar-editor-display-mode"
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>
              <strong>Full listing</strong>
              <small>
                {calendarDisplayMode === "full"
                  ? "Show event titles"
                  : "Show color dots only"}
              </small>
            </span>
          </button>
        </fieldset>
      {/if}
      {#if formError}<p class="calendar-form-error" role="alert">
          {formError}
        </p>{/if}
      <footer>
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={() => dialog?.close()}>Cancel</button
        ><button
          class="ui-button ui-button--primary calendar-primary"
          type="submit"
          disabled={saving}
          >{saving
            ? editingSubscription
              ? "Saving…"
              : "Fetching…"
            : editingSubscription
              ? "Save changes"
              : "Subscribe"}</button
        >
      </footer>
    </form>
  </dialog>
</section>

<style>
  .calendar-page {
    display: grid;
    gap: 18px;
    padding: clamp(24px, 3vw, 42px);
    min-width: 0;
  }
  .calendar-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  .calendar-sidebar section > span,
  .calendar-dialog header span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
  }
  .calendar-header p {
    margin: 7px 0 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  button,
  input {
    font: inherit;
  }
  button {
    color: inherit;
  }
  .calendar-primary {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--fg);
    background: var(--fg);
    color: var(--bg);
    padding: 0 16px;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.04em;
  }
  .calendar-primary:hover {
    background: transparent;
    color: var(--fg);
  }
  .calendar-error,
  .calendar-form-error {
    margin: 0;
    border: 1px solid oklch(60% 0.16 25 / 0.5);
    background: oklch(20% 0.04 25 / 0.75);
    padding: 10px 12px;
    color: oklch(82% 0.09 25);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .calendar-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(260px, 320px);
    gap: 18px;
    min-height: 0;
  }
  .month-panel,
  .calendar-sidebar section {
    border: 1px solid var(--border);
    background: color-mix(
      in oklch,
      var(--page-surface, var(--surface)) 92%,
      transparent
    );
  }
  .month-toolbar {
    display: flex;
    min-height: 74px;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  .month-heading {
    min-width: 0;
  }
  .month-heading h3 {
    margin: 0;
    font-family: var(--font-mono);
    font-size: clamp(24px, 2vw, 32px);
    font-weight: 580;
    line-height: 1.1;
    letter-spacing: -0.025em;
    text-transform: uppercase;
    text-wrap: balance;
  }
  .month-navigation {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .month-navigation .today-button {
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .month-navigation .month-shift {
    flex: 0 0 44px;
    padding: 0;
  }
  .month-grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
  }
  .month-grid > button {
    position: relative;
    min-width: 0;
    min-height: 112px;
    overflow: hidden;
    border: 0;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: transparent;
    padding: 8px;
    text-align: left;
  }
  .month-grid > button:nth-child(7n) {
    border-right: 0;
  }
  .month-grid > button:hover,
  .month-grid > button.selected {
    background: color-mix(in oklch, var(--fg) 6%, transparent);
  }
  .month-grid > button.outside {
    color: color-mix(in oklch, var(--muted) 55%, transparent);
  }
  .day-number {
    position: absolute;
    top: 8px;
    left: 8px;
    display: grid;
    width: 24px;
    height: 24px;
    place-items: center;
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .day-weekday {
    position: absolute;
    top: 11px;
    right: 10px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 620;
    letter-spacing: 0.08em;
  }
  .today .day-number {
    background: var(--accent);
    color: var(--button-on-accent);
    font-weight: 700;
  }
  .day-events {
    display: grid;
    gap: 4px;
    width: 100%;
    max-height: 68px;
    margin-top: 32px;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
  }
  .event-pill {
    overflow: hidden;
    border-left: 2px solid var(--calendar-color);
    padding: 3px 5px;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--fg);
    background: color-mix(in oklch, var(--calendar-color) 14%, transparent);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .event-pill.completed {
    text-decoration: line-through;
  }
  .event-dots {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 2px 1px;
  }
  .event-dot {
    width: 7px;
    height: 7px;
    flex: 0 0 7px;
    border-radius: 50%;
    background: var(--calendar-color);
  }
  .calendar-sidebar {
    display: grid;
    align-content: start;
    gap: 18px;
  }
  .calendar-sidebar section {
    padding: 16px;
  }
  .calendar-sidebar h3 {
    margin: 7px 0 15px;
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 550;
  }
  .agenda-list {
    display: grid;
    gap: 8px;
    max-height: 420px;
    overflow: auto;
  }
  .agenda-event {
    border: 1px solid var(--border);
    border-top: 2px solid var(--calendar-color);
    padding: 12px;
    color: var(--fg);
  }
  .agenda-event > div {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .agenda-event h4 {
    margin: 9px 0 5px;
    font-size: 13px;
  }
  .agenda-event.completed h4 {
    text-decoration: line-through;
  }
  .agenda-event p {
    display: flex;
    gap: 5px;
    margin: 5px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.5;
  }
  .agenda-event a {
    display: inline-flex;
    gap: 5px;
    margin-top: 9px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .agenda-event-action {
    width: 100%;
    min-height: 44px;
    background: transparent;
    text-align: left;
  }
  .agenda-event-action:hover {
    border-color: var(--fg);
    background: color-mix(in oklch, var(--fg) 6%, transparent);
  }
  .agenda-event-action-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 9px;
    font-family: var(--font-mono);
    font-size: 10px;
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .agenda-empty,
  .agenda-list > p {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .source-list {
    display: grid;
    gap: 7px;
  }
  .source-list article {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    border-top: 1px solid var(--border);
    padding-top: 9px;
  }
  .source-list article > button,
  .source-action-menu > button {
    display: grid;
    width: 44px;
    height: 44px;
    place-items: center;
    border: 1px solid transparent;
    background: transparent;
  }
  .source-list article > button:hover,
  .source-action-menu > button:hover {
    border-color: var(--border);
  }
  .source-list .source-toggle {
    position: relative;
    width: 44px;
    height: 44px;
    min-height: 44px;
    border: 0;
    background: transparent;
    padding: 0;
  }
  .source-toggle::before {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--calendar-color);
    content: "";
  }
  .source-toggle.muted::before {
    background: transparent;
    box-shadow: inset 0 0 0 1px var(--calendar-color);
  }
  .source-action-menu {
    position: relative;
    z-index: 3;
    display: grid;
    place-items: center;
  }
  .source-action-menu.is-open {
    z-index: 12;
  }
  .source-action-trigger[aria-expanded="true"] {
    border-color: var(--fg);
    background: var(--fg-soft);
  }
  .source-action-popover {
    position: absolute;
    z-index: 10;
    top: calc(100% + 4px);
    right: 0;
    width: 184px;
    border: 1px solid var(--border);
    background: var(--bg);
    padding: 6px;
  }
  .source-action-popover button {
    display: flex;
    width: 100%;
    min-height: 44px;
    align-items: center;
    gap: 9px;
    border: 1px solid transparent;
    background: transparent;
    padding: 0 10px;
    color: var(--fg);
    text-align: left;
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .source-action-popover button:hover {
    border-color: var(--border);
    background: var(--fg-soft);
    color: var(--fg);
  }
  .source-action-popover .source-menu-delete {
    color: var(--danger);
  }
  .source-action-popover .source-menu-delete:hover {
    border-color: color-mix(in oklch, var(--danger) 55%, var(--border));
    background: color-mix(in oklch, var(--danger) 12%, transparent);
    color: var(--danger);
  }
  .source-action-popover .source-menu-delete.is-armed {
    border-color: var(--danger);
    background: var(--danger);
    color: var(--bg);
  }
  .source-list strong,
  .source-list small {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
  }
  .source-list strong {
    font-size: 11px;
    font-weight: 550;
  }
  .source-list small {
    margin-top: 3px;
    color: var(--muted);
    font-size: 9px;
  }
  .source-static-mark {
    grid-column: 3 / -1;
    justify-self: end;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 8px;
    letter-spacing: 0.08em;
  }
  .calendar-dialog {
    width: min(520px, calc(100vw - 32px));
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--fg);
    padding: 0;
  }
  .calendar-dialog::backdrop {
    background: oklch(5% 0 0 / 0.7);
    backdrop-filter: blur(5px);
  }
  .calendar-dialog header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 20px;
    border-bottom: 1px solid var(--border);
  }
  .calendar-dialog h2 {
    margin: 6px 0 0;
    font-family: var(--font-mono);
    font-size: 20px;
    font-weight: 550;
  }
  .calendar-dialog header button {
    width: 44px;
    height: 44px;
    border: 1px solid var(--border);
    background: transparent;
  }
  .calendar-dialog form {
    display: grid;
    gap: 10px;
    padding: 20px;
  }
  .calendar-dialog label,
  .calendar-dialog legend {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.04em;
  }
  .calendar-dialog .calendar-text-input {
    min-height: 44px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    padding: 0 12px;
  }
  .calendar-dialog small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .calendar-dialog fieldset {
    margin: 8px 0 0;
    border: 1px solid var(--border);
    padding: 14px;
  }
  .calendar-display-editor {
    display: grid;
    gap: 4px;
  }
  .calendar-dialog .calendar-dialog-toggle {
    width: 100%;
    height: auto;
    justify-content: flex-start;
    padding: 4px 0;
  }
  .calendar-dialog-toggle strong,
  .calendar-dialog-toggle small {
    display: block;
  }
  .calendar-dialog-toggle strong {
    color: var(--fg);
    font-size: 11px;
    font-weight: 550;
  }
  .calendar-dialog-toggle small {
    margin-top: 3px;
  }
  .calendar-dialog footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }
  .calendar-dialog footer > button:not(.calendar-primary) {
    min-height: 44px;
    border: 1px solid var(--border);
    background: transparent;
    padding: 0 16px;
  }
  :focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }
  @media (max-width: 1050px) {
    .calendar-layout {
      grid-template-columns: 1fr;
    }
    .calendar-sidebar {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (max-width: 720px) {
    .calendar-header {
      align-items: stretch;
      flex-direction: column;
    }
    .calendar-primary {
      justify-content: center;
    }
    .calendar-sidebar {
      grid-template-columns: 1fr;
    }
    .month-toolbar {
      align-items: stretch;
      flex-direction: column;
      gap: 12px;
    }
    .month-navigation {
      width: 100%;
    }
    .month-navigation .today-button {
      flex: 1;
    }
    .month-grid > button {
      min-height: 72px;
      padding: 4px;
    }
    .day-number {
      top: 6px;
      left: 5px;
    }
    .day-weekday {
      top: 9px;
      right: 6px;
      font-size: 8px;
    }
    .day-events {
      max-height: 36px;
    }
  }
</style>

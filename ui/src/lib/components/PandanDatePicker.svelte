<script lang="ts">
  import CalendarDays from "lucide-svelte/icons/calendar-days";
  import ChevronLeft from "lucide-svelte/icons/chevron-left";
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import ZoomIn from "lucide-svelte/icons/zoom-in";
  import ZoomOut from "lucide-svelte/icons/zoom-out";
  import { tick } from "svelte";
  import { motionPopover } from "$lib/motion.svelte";

  type CalendarPickerView = "days" | "months" | "years";

  type CalendarSelectorDay = {
    key: string;
    day: number;
    inMonth: boolean;
    isToday: boolean;
    isSelected: boolean;
  };

  type Props = {
    id: string;
    value?: string;
    ariaLabel: string;
    name?: string;
    required?: boolean;
    disabled?: boolean;
    compact?: boolean;
    odId?: string;
    onchange?: (value: string) => void;
  };

  let {
    id,
    value = $bindable(""),
    ariaLabel,
    name,
    required = false,
    disabled = false,
    compact = false,
    odId,
    onchange,
  }: Props = $props();

  const calendarWeekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const calendarMonths = [
    { short: "JAN", label: "January" },
    { short: "FEB", label: "February" },
    { short: "MAR", label: "March" },
    { short: "APR", label: "April" },
    { short: "MAY", label: "May" },
    { short: "JUN", label: "June" },
    { short: "JUL", label: "July" },
    { short: "AUG", label: "August" },
    { short: "SEP", label: "September" },
    { short: "OCT", label: "October" },
    { short: "NOV", label: "November" },
    { short: "DEC", label: "December" },
  ] as const;
  const calendarMonthFormatter = new Intl.DateTimeFormat("en", {
    month: "short",
    year: "numeric",
  });
  const calendarCurrentDate = new Date();
  const calendarCurrentYear = calendarCurrentDate.getFullYear();
  const calendarCurrentMonth = calendarCurrentDate.getMonth();
  const calendarYearBatchSize = 48;
  const calendarYearMinimum = 1000;
  const calendarYearMaximum = 9999;

  let trigger = $state<HTMLButtonElement>();
  let popover = $state<HTMLElement>();
  let yearScroll = $state<HTMLElement>();
  let calendarOpen = $state(false);
  let calendarPickerView = $state<CalendarPickerView>("days");
  let calendarViewYear = $state(calendarCurrentYear);
  let calendarViewMonth = $state(calendarCurrentMonth);
  let popoverLeft = $state(12);
  let popoverTop = $state(12);
  let yearOptions = $state.raw<number[]>([]);
  let extendingYears = false;

  let popoverId = $derived(`${id}-calendar`);
  let calendarMonthLabel = $derived(
    calendarMonthFormatter.format(
      new Date(calendarViewYear, calendarViewMonth, 1, 12),
    ),
  );
  let calendarHeaderLabel = $derived(
    calendarPickerView === "days"
      ? calendarMonthLabel
      : calendarPickerView === "months"
        ? String(calendarViewYear)
        : "Year",
  );
  let selectedCalendarParts = $derived.by(() => {
    const selected = parseDateKey(value);
    return selected
      ? { year: selected.getFullYear(), month: selected.getMonth() }
      : null;
  });
  let calendarSelectorDays = $derived.by(() =>
    buildCalendarSelectorDays(calendarViewYear, calendarViewMonth, value),
  );
  let selectedDateLabel = $derived(value ? formatDate(value) : "Choose a date");

  function captureTrigger(node: HTMLButtonElement) {
    trigger = node;
    return () => {
      trigger = undefined;
    };
  }

  function capturePopover(node: HTMLElement) {
    popover = node;
    return () => {
      if (node.matches(":popover-open")) node.hidePopover();
      popover = undefined;
    };
  }

  function captureYearScroll(node: HTMLElement) {
    yearScroll = node;
    return () => {
      yearScroll = undefined;
    };
  }

  function captureScrollDismiss(node: HTMLElement) {
    const ownerDocument = node.ownerDocument;
    const handleScroll = (event: Event) => {
      const target = event.target;
      if (
        !calendarOpen ||
        (target instanceof Node && popover?.contains(target))
      ) {
        return;
      }
      closeDateSelector(false);
    };
    ownerDocument.addEventListener("scroll", handleScroll, true);
    return () => {
      ownerDocument.removeEventListener("scroll", handleScroll, true);
    };
  }

  async function openDateSelector() {
    if (disabled) return;
    const selected = parseDateKey(value) ?? new Date();
    calendarViewYear = selected.getFullYear();
    calendarViewMonth = selected.getMonth();
    calendarPickerView = "days";
    if (popover && !popover.matches(":popover-open")) popover.showPopover();
    positionDateSelector();
    calendarOpen = true;
    await tick();
    document
      .getElementById(`${id}-calendar-day-${toDateKey(selected)}`)
      ?.focus();
  }

  function closeDateSelector(restoreFocus: boolean) {
    calendarOpen = false;
    if (!restoreFocus) return;
    void tick().then(() => trigger?.focus());
  }

  function toggleDateSelector() {
    if (calendarOpen) {
      closeDateSelector(false);
      return;
    }
    void openDateSelector();
  }

  function handlePopoverToggle(event: Event) {
    const toggleEvent = event as ToggleEvent;
    if (toggleEvent.newState === "closed") calendarOpen = false;
  }

  function hidePopoverAfterExit() {
    if (popover?.matches(":popover-open")) popover.hidePopover();
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || !calendarOpen) return;
    event.preventDefault();
    closeDateSelector(true);
  }

  function handleWindowPointerdown(event: PointerEvent) {
    if (!calendarOpen) return;
    const target = event.target;
    if (
      target instanceof Element &&
      target.closest(`[data-pandan-date-picker="${id}"]`)
    ) {
      return;
    }
    closeDateSelector(false);
  }

  function positionDateSelector() {
    if (!trigger || !popover) return;
    const viewportInset = 12;
    const gap = 8;
    const triggerRect = trigger.getBoundingClientRect();
    const popoverRect = popover.getBoundingClientRect();
    const pickerWidth = popoverRect.width || 360;
    const pickerHeight = popoverRect.height || 430;
    popoverLeft = Math.min(
      Math.max(viewportInset, triggerRect.left),
      Math.max(viewportInset, window.innerWidth - pickerWidth - viewportInset),
    );
    const belowTop = triggerRect.bottom + gap;
    const aboveTop = triggerRect.top - pickerHeight - gap;
    popoverTop =
      belowTop + pickerHeight <= window.innerHeight - viewportInset
        ? belowTop
        : Math.max(viewportInset, aboveTop);
  }

  function changeCalendarPeriod(offset: number) {
    if (calendarPickerView === "days") {
      const nextMonth = new Date(
        calendarViewYear,
        calendarViewMonth + offset,
        1,
        12,
      );
      calendarViewYear = nextMonth.getFullYear();
      calendarViewMonth = nextMonth.getMonth();
      return;
    }
    calendarViewYear += offset;
  }

  async function toggleCalendarZoom() {
    const nextView: CalendarPickerView =
      calendarPickerView === "days"
        ? "months"
        : calendarPickerView === "months"
          ? "years"
          : "months";
    if (nextView === "years") {
      yearOptions = buildCalendarYearWindow(calendarViewYear);
    }
    calendarPickerView = nextView;
    await tick();
    positionDateSelector();
    if (calendarPickerView === "years") {
      centerCalendarYear(calendarViewYear);
      document
        .getElementById(`${id}-calendar-year-${calendarViewYear}`)
        ?.focus();
      return;
    }
    document
      .getElementById(`${id}-calendar-month-${calendarViewMonth}`)
      ?.focus();
  }

  async function selectCalendarMonth(month: number) {
    calendarViewMonth = month;
    calendarPickerView = "days";
    await tick();
    positionDateSelector();
    const selected = parseDateKey(value);
    const target =
      selected &&
      selected.getFullYear() === calendarViewYear &&
      selected.getMonth() === month
        ? selected
        : new Date(calendarViewYear, month, 1, 12);
    document.getElementById(`${id}-calendar-day-${toDateKey(target)}`)?.focus();
  }

  async function selectCalendarYear(year: number) {
    calendarViewYear = year;
    calendarPickerView = "months";
    await tick();
    positionDateSelector();
    document
      .getElementById(`${id}-calendar-month-${calendarViewMonth}`)
      ?.focus();
  }

  function chooseDate(key: string) {
    value = key;
    onchange?.(key);
    closeDateSelector(true);
  }

  function selectToday() {
    const today = new Date();
    value = toDateKey(today);
    calendarViewYear = today.getFullYear();
    calendarViewMonth = today.getMonth();
    onchange?.(value);
    closeDateSelector(true);
  }

  function clearDate() {
    value = "";
    onchange?.(value);
    closeDateSelector(true);
  }

  function buildCalendarYearWindow(centerYear: number) {
    const start = Math.max(
      calendarYearMinimum,
      centerYear - calendarYearBatchSize,
    );
    const end = Math.min(
      calendarYearMaximum,
      centerYear + calendarYearBatchSize,
    );
    return Array.from({ length: end - start + 1 }, (_, index) => start + index);
  }

  function centerCalendarYear(year: number) {
    const target = document.getElementById(`${id}-calendar-year-${year}`);
    if (!target || !yearScroll) return;
    const scrollRect = yearScroll.getBoundingClientRect();
    const targetRect = target.getBoundingClientRect();
    const targetTop = targetRect.top - scrollRect.top + yearScroll.scrollTop;
    yearScroll.scrollTop =
      targetTop - (yearScroll.clientHeight - target.offsetHeight) / 2;
  }

  async function handleCalendarYearScroll(event: Event) {
    const scroll = event.currentTarget;
    if (!(scroll instanceof HTMLElement) || extendingYears) return;
    const firstYear = yearOptions[0];
    const lastYear = yearOptions.at(-1);
    if (firstYear === undefined || lastYear === undefined) return;
    const threshold = scroll.clientHeight * 0.75;

    if (scroll.scrollTop <= threshold && firstYear > calendarYearMinimum) {
      extendingYears = true;
      const previousHeight = scroll.scrollHeight;
      const start = Math.max(
        calendarYearMinimum,
        firstYear - calendarYearBatchSize,
      );
      const prepended = Array.from(
        { length: firstYear - start },
        (_, index) => start + index,
      );
      yearOptions = [...prepended, ...yearOptions];
      await tick();
      scroll.scrollTop += scroll.scrollHeight - previousHeight;
      extendingYears = false;
      return;
    }

    if (
      scroll.scrollHeight - scroll.scrollTop - scroll.clientHeight <=
        threshold &&
      lastYear < calendarYearMaximum
    ) {
      extendingYears = true;
      const end = Math.min(
        calendarYearMaximum,
        lastYear + calendarYearBatchSize,
      );
      const appended = Array.from(
        { length: end - lastYear },
        (_, index) => lastYear + index + 1,
      );
      yearOptions = [...yearOptions, ...appended];
      await tick();
      extendingYears = false;
    }
  }

  function handleCalendarDayKeydown(
    event: KeyboardEvent,
    day: CalendarSelectorDay,
  ) {
    const date = parseDateKey(day.key);
    if (!date) return;
    const dayOffsets: Record<string, number> = {
      ArrowLeft: -1,
      ArrowRight: 1,
      ArrowUp: -7,
      ArrowDown: 7,
      Home: -date.getDay(),
      End: 6 - date.getDay(),
    };
    if (event.key in dayOffsets) {
      event.preventDefault();
      focusCalendarDate(
        new Date(
          date.getFullYear(),
          date.getMonth(),
          date.getDate() + dayOffsets[event.key],
          12,
        ),
      );
      return;
    }
    if (event.key === "PageUp" || event.key === "PageDown") {
      event.preventDefault();
      const monthOffset = event.key === "PageUp" ? -1 : 1;
      const targetMonth = new Date(
        date.getFullYear(),
        date.getMonth() + monthOffset,
        1,
        12,
      );
      const lastDay = new Date(
        targetMonth.getFullYear(),
        targetMonth.getMonth() + 1,
        0,
        12,
      ).getDate();
      focusCalendarDate(
        new Date(
          targetMonth.getFullYear(),
          targetMonth.getMonth(),
          Math.min(date.getDate(), lastDay),
          12,
        ),
      );
    }
  }

  function focusCalendarDate(date: Date) {
    calendarViewYear = date.getFullYear();
    calendarViewMonth = date.getMonth();
    void tick().then(() => {
      document.getElementById(`${id}-calendar-day-${toDateKey(date)}`)?.focus();
    });
  }

  function handleInvalid(event: Event) {
    event.preventDefault();
    trigger?.focus();
    void openDateSelector();
  }

  function buildCalendarSelectorDays(
    year: number,
    month: number,
    selectedKey: string,
  ): CalendarSelectorDay[] {
    const firstOfMonth = new Date(year, month, 1, 12);
    const todayKey = toDateKey(new Date());
    return Array.from({ length: 42 }, (_, index) => {
      const date = new Date(year, month, 1 - firstOfMonth.getDay() + index, 12);
      const key = toDateKey(date);
      return {
        key,
        day: date.getDate(),
        inMonth: date.getMonth() === month,
        isToday: key === todayKey,
        isSelected: key === selectedKey,
      };
    });
  }

  function parseDateKey(dateValue: string): Date | null {
    const match = dateValue.match(/^(\d{4})-(\d{2})-(\d{2})$/);
    if (!match) return null;
    const year = Number(match[1]);
    const month = Number(match[2]) - 1;
    const day = Number(match[3]);
    const date = new Date(year, month, day, 12);
    return date.getFullYear() === year &&
      date.getMonth() === month &&
      date.getDate() === day
      ? date
      : null;
  }

  function toDateKey(date: Date) {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function formatDate(dateValue: string) {
    const date = parseDateKey(dateValue);
    return date
      ? new Intl.DateTimeFormat("en", {
          month: "short",
          day: "numeric",
          year: "numeric",
        }).format(date)
      : dateValue;
  }
</script>

<svelte:window
  onkeydown={handleWindowKeydown}
  onpointerdown={handleWindowPointerdown}
  onresize={() => calendarOpen && positionDateSelector()}
/>

<div
  class={["pandan-date-picker", compact && "is-compact"]}
  data-pandan-date-picker={id}
  data-od-id={odId ?? `${id}-date-picker`}
  {@attach captureScrollDismiss}
>
  <button
    class="pandan-date-trigger"
    {id}
    type="button"
    aria-label={ariaLabel}
    aria-haspopup="dialog"
    aria-expanded={calendarOpen}
    aria-controls={popoverId}
    {disabled}
    data-od-id={`${odId ?? id}-trigger`}
    onclick={toggleDateSelector}
    {@attach captureTrigger}
  >
    <span class="pandan-date-trigger-value">
      <span class="pandan-date-icon" aria-hidden="true">
        <CalendarDays size={17} strokeWidth={1.7} />
      </span>
      <strong>{selectedDateLabel}</strong>
    </span>
    {#if !compact}
      <span class="pandan-date-trigger-action">
        {calendarOpen ? "Close" : "Choose date"}
      </span>
    {/if}
  </button>

  <input
    class="pandan-date-validation"
    id={`${id}-value`}
    type="text"
    {name}
    {required}
    {disabled}
    pattern={"\\d{4}-\\d{2}-\\d{2}"}
    tabindex="-1"
    aria-hidden="true"
    {value}
    oninvalid={handleInvalid}
  />

  <div
    class="pandan-date-popover"
    id={popoverId}
    popover="manual"
    role="dialog"
    aria-label={ariaLabel}
    aria-hidden={!calendarOpen}
    inert={!calendarOpen}
    style:--date-popover-left={`${popoverLeft}px`}
    style:--date-popover-top={`${popoverTop}px`}
    data-pandan-date-picker={id}
    data-od-id={`${odId ?? id}-calendar`}
    ontoggle={handlePopoverToggle}
    {@attach capturePopover}
    {@attach motionPopover(calendarOpen, {
      closedY: -6,
      onExitComplete: hidePopoverAfterExit,
    })}
  >
    <div class="pandan-date-panel">
      <header>
        <button
          class="pandan-date-zoom"
          type="button"
          aria-label={calendarPickerView === "days"
            ? `Choose a month or year, currently ${calendarHeaderLabel}`
            : calendarPickerView === "months"
              ? `Choose a year, currently ${calendarHeaderLabel}`
              : "Return to month selection"}
          data-od-id={`${odId ?? id}-zoom`}
          onclick={toggleCalendarZoom}
        >
          <span class="pandan-date-zoom-label">
            <h3>{calendarHeaderLabel}</h3>
            <span class="pandan-date-zoom-icon" aria-hidden="true">
              {#if calendarPickerView === "years"}
                <ZoomIn size={15} strokeWidth={1.8} />
              {:else}
                <ZoomOut size={15} strokeWidth={1.8} />
              {/if}
            </span>
          </span>
        </button>
        {#if calendarPickerView !== "years"}
          <div
            class="pandan-date-navigation"
            role="group"
            aria-label={`Navigate ${calendarPickerView === "days" ? "months" : "years"}`}
          >
            <button
              type="button"
              aria-label={`Previous ${calendarPickerView === "days" ? "month" : "year"}`}
              data-od-id={`${odId ?? id}-previous-period`}
              onclick={() => changeCalendarPeriod(-1)}
            >
              <ChevronLeft size={17} strokeWidth={1.8} aria-hidden="true" />
            </button>
            <button
              type="button"
              aria-label={`Next ${calendarPickerView === "days" ? "month" : "year"}`}
              data-od-id={`${odId ?? id}-next-period`}
              onclick={() => changeCalendarPeriod(1)}
            >
              <ChevronRight size={17} strokeWidth={1.8} aria-hidden="true" />
            </button>
          </div>
        {/if}
      </header>

      {#if calendarPickerView === "days"}
        <div class="pandan-date-weekdays" aria-hidden="true">
          {#each calendarWeekdays as weekday (weekday)}
            <span>{weekday}</span>
          {/each}
        </div>
        <div
          class="pandan-date-grid"
          role="grid"
          aria-label={calendarMonthLabel}
        >
          {#each calendarSelectorDays as day (day.key)}
            <button
              class={[
                !day.inMonth && "is-outside",
                day.isToday && "is-today",
                day.isSelected && "is-selected",
              ]}
              id={`${id}-calendar-day-${day.key}`}
              type="button"
              role="gridcell"
              tabindex={day.isSelected ||
              (!calendarSelectorDays.some(
                (candidate) => candidate.isSelected,
              ) &&
                day.inMonth &&
                day.day === 1)
                ? 0
                : -1}
              aria-label={formatDate(day.key)}
              aria-current={day.isToday ? "date" : undefined}
              aria-selected={day.isSelected}
              data-od-id={`${odId ?? id}-date-${day.key}`}
              onclick={() => chooseDate(day.key)}
              onkeydown={(event) => handleCalendarDayKeydown(event, day)}
            >
              <span>{String(day.day).padStart(2, "0")}</span>
            </button>
          {/each}
        </div>
      {:else if calendarPickerView === "months"}
        <div
          class="pandan-date-period-grid"
          role="grid"
          aria-label={`Choose a month in ${calendarViewYear}`}
        >
          {#each calendarMonths as month, monthIndex (month.short)}
            <button
              class={[
                selectedCalendarParts?.year === calendarViewYear &&
                  selectedCalendarParts.month === monthIndex &&
                  "is-selected",
                calendarCurrentYear === calendarViewYear &&
                  calendarCurrentMonth === monthIndex &&
                  "is-current",
              ]}
              id={`${id}-calendar-month-${monthIndex}`}
              type="button"
              role="gridcell"
              aria-label={`${month.label} ${calendarViewYear}`}
              aria-selected={selectedCalendarParts?.year === calendarViewYear &&
                selectedCalendarParts.month === monthIndex}
              data-od-id={`${odId ?? id}-month-${monthIndex + 1}`}
              onclick={() => selectCalendarMonth(monthIndex)}
            >
              <span>{month.short}</span>
            </button>
          {/each}
        </div>
      {:else}
        <div
          class="pandan-date-year-scroll"
          role="region"
          aria-label="Choose a year"
          data-od-id={`${odId ?? id}-year-scroll`}
          onscroll={handleCalendarYearScroll}
          {@attach captureYearScroll}
        >
          <div
            class="pandan-date-period-grid is-year-grid"
            role="grid"
            aria-label="Years"
          >
            {#each yearOptions as year (year)}
              <button
                class={[
                  selectedCalendarParts?.year === year && "is-selected",
                  calendarCurrentYear === year && "is-current",
                ]}
                id={`${id}-calendar-year-${year}`}
                type="button"
                role="gridcell"
                aria-label={`Select ${year}`}
                aria-selected={selectedCalendarParts?.year === year}
                data-od-id={`${odId ?? id}-year-${year}`}
                onclick={() => selectCalendarYear(year)}
              >
                <span>{year}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <footer>
        <span>Selected: <strong>{selectedDateLabel}</strong></span>
        <div class="pandan-date-footer-actions">
          {#if !required && value}
            <button
              type="button"
              data-od-id={`${odId ?? id}-clear`}
              onclick={clearDate}>Clear</button
            >
          {/if}
          <button
            type="button"
            data-od-id={`${odId ?? id}-today`}
            onclick={selectToday}>Today</button
          >
        </div>
      </footer>
    </div>
  </div>
</div>

<style>
  .pandan-date-picker {
    position: relative;
    min-width: 0;
    width: 100%;
  }
  .pandan-date-trigger {
    display: flex;
    width: 100%;
    min-height: 58px;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    padding: 8px 12px 8px 9px;
    text-align: left;
  }
  .pandan-date-trigger:hover,
  .pandan-date-trigger[aria-expanded="true"] {
    border-color: var(--fg);
    background: color-mix(in oklch, var(--fg) 5%, var(--bg));
  }
  .pandan-date-trigger:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }
  .pandan-date-trigger-value {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 10px;
  }
  .pandan-date-icon {
    display: grid;
    width: 38px;
    height: 38px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--border);
  }
  .pandan-date-trigger-value strong {
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 550;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pandan-date-trigger-action {
    flex: 0 0 auto;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .pandan-date-picker.is-compact .pandan-date-trigger {
    min-height: 44px;
    padding: 3px 9px;
  }
  .pandan-date-picker.is-compact .pandan-date-icon {
    width: 34px;
    height: 34px;
  }
  .pandan-date-validation {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    border: 0;
    margin: 0;
    opacity: 0;
    padding: 0;
    pointer-events: none;
  }
  .pandan-date-popover {
    position: fixed;
    inset: auto;
    top: var(--date-popover-top);
    left: var(--date-popover-left);
    width: min(360px, calc(100vw - 24px));
    max-height: calc(100dvh - 24px);
    margin: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    border: 0;
    background: transparent;
    color: var(--fg);
    padding: 0;
  }
  .pandan-date-popover::backdrop {
    background: transparent;
  }
  .pandan-date-panel {
    border: 1px solid var(--border);
    background: color-mix(
      in oklch,
      var(--page-surface, var(--surface)) 94%,
      var(--bg)
    );
    box-shadow: 0 18px 54px color-mix(in oklch, var(--bg) 72%, transparent);
  }
  .pandan-date-panel > header {
    display: flex;
    min-height: 54px;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    border-bottom: 1px solid var(--border);
    padding: 5px 10px 5px 14px;
  }
  .pandan-date-zoom {
    display: flex;
    min-width: 0;
    min-height: 44px;
    flex: 1;
    align-items: center;
    justify-content: flex-start;
    gap: 12px;
    border: 0;
    background: transparent;
    color: var(--fg);
    padding: 0 6px 0 0;
    text-align: left;
  }
  .pandan-date-zoom:hover {
    background: color-mix(in oklch, var(--fg) 6%, transparent);
  }
  .pandan-date-zoom-label {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .pandan-date-zoom-icon {
    display: grid;
    flex: 0 0 auto;
    place-items: center;
    color: var(--muted);
  }
  .pandan-date-panel h3 {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 14px;
    font-weight: 550;
  }
  .pandan-date-navigation {
    display: flex;
    gap: 6px;
  }
  .pandan-date-navigation button,
  .pandan-date-panel > footer button {
    display: grid;
    min-width: 44px;
    min-height: 44px;
    place-items: center;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
  }
  .pandan-date-navigation button:hover,
  .pandan-date-panel > footer button:hover {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--bg);
  }
  .pandan-date-weekdays,
  .pandan-date-grid {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
  }
  .pandan-date-weekdays {
    min-height: 34px;
    align-items: center;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.06em;
    text-align: center;
    text-transform: uppercase;
  }
  .pandan-date-grid,
  .pandan-date-period-grid {
    border-left: 1px solid var(--border);
  }
  .pandan-date-grid button {
    min-width: 0;
    min-height: 44px;
    border: 0;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .pandan-date-grid button:hover,
  .pandan-date-period-grid button:hover {
    background: color-mix(in oklch, var(--fg) 8%, transparent);
  }
  .pandan-date-grid button.is-outside {
    color: var(--muted);
    opacity: 0.48;
  }
  .pandan-date-grid button.is-today,
  .pandan-date-period-grid button.is-current {
    box-shadow: inset 0 0 0 1px var(--fg);
  }
  .pandan-date-grid button.is-selected,
  .pandan-date-period-grid button.is-selected {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--bg);
    opacity: 1;
  }
  .pandan-date-grid button.is-selected:hover,
  .pandan-date-period-grid button.is-selected:hover {
    background: color-mix(in oklch, var(--accent) 86%, var(--bg));
    color: var(--bg);
  }
  .pandan-date-period-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .pandan-date-period-grid button {
    display: grid;
    min-width: 0;
    min-height: 56px;
    place-content: center;
    border: 0;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    text-align: center;
  }
  .pandan-date-period-grid button > span {
    font-size: 11px;
    letter-spacing: 0.05em;
  }
  .pandan-date-year-scroll {
    max-height: 264px;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
  }
  .pandan-date-period-grid.is-year-grid {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
  .pandan-date-period-grid.is-year-grid button {
    min-height: 52px;
  }
  .pandan-date-panel > footer {
    display: flex;
    min-height: 62px;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 10px 8px 14px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .pandan-date-panel > footer strong {
    color: var(--fg);
    font-weight: 550;
  }
  .pandan-date-footer-actions {
    display: flex;
    flex: 0 0 auto;
    gap: 6px;
  }
  .pandan-date-panel > footer button {
    width: auto;
    padding: 0 13px;
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  :focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }
  @supports not (scrollbar-gutter: stable) {
    .pandan-date-popover,
    .pandan-date-year-scroll {
      overflow-y: scroll;
    }
  }
  @media (max-width: 640px) {
    .pandan-date-popover {
      inset: auto 8px max(8px, env(safe-area-inset-bottom)) 8px;
      width: auto;
      max-height: calc(100dvh - 16px - env(safe-area-inset-bottom));
    }
    .pandan-date-trigger-action {
      display: none;
    }
  }
</style>

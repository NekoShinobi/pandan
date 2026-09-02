<script lang="ts">
  import ArrowRight from "lucide-svelte/icons/arrow-right";
  import Check from "lucide-svelte/icons/check";
  import DashboardLocalWidget from "$lib/DashboardLocalWidget.svelte";
  import DashboardMediaWidget from "$lib/DashboardMediaWidget.svelte";
  import IntegrationWidget from "$lib/IntegrationWidget.svelte";
  import WeatherWidget from "$lib/WeatherWidget.svelte";
  import type { Bookmark, DashboardWidget, Task, UserSettings } from "$lib/api";

  let {
    widget,
    editing,
    tasks,
    settings,
    completedCount,
    todayTasks,
    todayCompletedCount,
    todayTaskProgress,
    onToggleTask,
    onCreateTask,
    onClearCompleted,
    onStartFocus,
    onToast,
    onOpenCalendarDate,
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
    onManageBookmarks,
    onOpenContextMenu,
    onUpdateWidget,
    onRegisterEditor,
  }: {
    widget: DashboardWidget;
    editing: boolean;
    tasks: Task[];
    settings: UserSettings;
    completedCount: number;
    todayTasks: Task[];
    todayCompletedCount: number;
    todayTaskProgress: number;
    onToggleTask: (task: Task) => void;
    onCreateTask: (title: string) => Promise<void>;
    onClearCompleted: () => void;
    onStartFocus: (subject: string, minutes: number) => void;
    onToast: (message: string) => void;
    onOpenCalendarDate: (date: string) => void;
    firstName: string;
    clocks: Array<{
      timezone: string;
      hourAngle: number;
      minuteAngle: number;
      secondAngle: number;
      label: string;
    }>;
    clockMarks: number[];
    dateLabel: string;
    calendarMonthLabel: string;
    calendarWeekdays: readonly string[];
    calendarDays: Array<{
      key: string;
      day: number;
      currentMonth: boolean;
      today: boolean;
    }>;
    calendarEventsByDate: Record<
      string,
      { count: number; colors: string[] }
    >;
    bookmarks: Bookmark[];
    onShowCurrentMonth: () => void;
    onChangeCalendarMonth: (offset: number) => void;
    onManageBookmarks: () => void;
    onOpenContextMenu: (
      widget: DashboardWidget,
      title: string,
      x: number,
      y: number,
    ) => void;
    onUpdateWidget: (widget: DashboardWidget) => void;
    onRegisterEditor: (widgetId: string, editor?: () => void) => void;
  } = $props();

  const localWidgetKinds = new Set([
    "welcome",
    "local-time",
    "calendar-overview",
    "bookmarks",
    "section-header",
    "divider",
  ]);

  let newTaskTitle = $state("");
  let savingTask = $state(false);
  let focusGoal = $state("");
  let focusMinutes = $derived(
    Math.min(240, Math.max(1, Number(widget.config.default_minutes) || 25)),
  );
  let nextTask = $derived(tasks.find((task) => !task.completed));
  let showTaskPreview = $derived(widget.config.summary_style !== "progress");
  let showTaskPriorities = $derived(widget.config.show_priorities !== false);
  let visibleTaskCount = $derived(
    widget.size === "compact"
      ? 3
      : widget.size === "standard"
        ? 5
        : tasks.length,
  );
  let widgetTitle = $derived.by(() => {
    const customTitle = widget.config.title;
    if (typeof customTitle === "string" && customTitle.trim()) {
      return customTitle.trim();
    }
    return (
      {
      welcome: "Welcome",
      "local-time": "Local time",
      "calendar-overview": "Calendar overview",
      bookmarks: "Bookmarks",
      "section-header": "Category header",
      divider: "Line divider",
      "image-frame": "Image frame",
      "music-visualizer": "Music visualizer",
      weather: "Weather",
      "task-summary": "Today",
      focus: "Next focus",
      "task-list": "Tasks",
      "feed-list": "Feed",
      "feed-sources": "Sources",
      youtube: "YouTube",
      rss: "RSS feeds",
      reddit: "Reddit",
      stocks: "Stocks",
      calendar: "Calendar",
      clock: "Clock",
      iframe: "Custom frame",
      html: "Custom HTML",
      releases: "Releases",
      streams: "Channels",
      "bible-verse": "Bible Verse",
      }[widget.kind] ?? "Widget"
    );
  });

  async function submitTask(event: SubmitEvent) {
    event.preventDefault();
    const title = newTaskTitle.trim();
    if (!title || savingTask) return;
    savingTask = true;
    try {
      await onCreateTask(title);
      newTaskTitle = "";
    } finally {
      savingTask = false;
    }
  }

  function submitFocus(event: SubmitEvent) {
    event.preventDefault();
    const subject = focusGoal.trim() || nextTask?.title || "Deep work";
    const minutes = Math.min(240, Math.max(1, Math.round(focusMinutes || 1)));
    focusGoal = subject;
    focusMinutes = minutes;
    onStartFocus(subject, minutes);
  }

  function openWidgetContextMenu(event: MouseEvent) {
    if (!editing) return;
    event.preventDefault();
    onOpenContextMenu(
      widget,
      widgetTitle,
      event.clientX,
      event.clientY,
    );
  }

  function handleWidgetKeydown(event: KeyboardEvent) {
    const opensMenu =
      event.key === "Enter" ||
      event.key === " " ||
      event.key === "ContextMenu" ||
      (event.shiftKey && event.key === "F10");
    if (
      !editing ||
      !opensMenu
    ) {
      return;
    }
    event.preventDefault();
    const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
    onOpenContextMenu(widget, widgetTitle, bounds.left + 24, bounds.top + 24);
  }
</script>

<article
  class={[
    "widget",
    "widget-card",
    `widget-size-${widget.size}`,
    `widget-kind-${widget.kind}`,
    editing && "is-editing",
  ]}
  data-size={widget.size}
  role="listitem"
  data-od-id={`widget-${widget.id}`}
>
  {#if editing}
    <div
      class="widget-drag-surface"
      role="button"
      tabindex="0"
      aria-label={`${widgetTitle} widget. Drag to move. Press Enter for actions.`}
      oncontextmenu={openWidgetContextMenu}
      onkeydown={handleWidgetKeydown}
    ></div>
    <span
      class="widget-edit-label"
      data-od-id={`widget-edit-name-${widget.id}`}>{widgetTitle}</span
    >
  {/if}

  <div class="widget-content" inert={editing}>
    {#if localWidgetKinds.has(widget.kind)}
      <DashboardLocalWidget
        {widget}
        {firstName}
        {clocks}
        {clockMarks}
        {dateLabel}
        {calendarMonthLabel}
        {calendarWeekdays}
        {calendarDays}
        {calendarEventsByDate}
        {bookmarks}
        {onShowCurrentMonth}
        {onChangeCalendarMonth}
        {onOpenCalendarDate}
        {onManageBookmarks}
      />
    {:else if widget.kind === "image-frame" || widget.kind === "music-visualizer"}
      <DashboardMediaWidget {widget} />
    {:else if widget.kind === "weather"}
      <WeatherWidget
        {widget}
        {settings}
        onUpdate={onUpdateWidget}
        {onToast}
        onRegisterEditor={(editor) => onRegisterEditor(widget.id, editor)}
      />
    {:else if widget.kind === "task-summary"}
      <div class="widget-head">
        <h2>Today</h2>
        <span class="mono muted">{todayCompletedCount}/{todayTasks.length}</span
        >
      </div>
      <div class="completion-summary">
        <strong class="mono">{todayTaskProgress}%</strong>
        <span class="muted">complete</span>
      </div>
      {#if showTaskPreview}
        <div class="agenda-list compact-list adaptive-task-preview">
          {#each todayTasks.slice(0, visibleTaskCount) as task (task.id)}
            <div class="agenda-row">
              <span class={["status-dot", task.completed && "filled"]}></span>
              <span>{task.title}</span>
              <span class="time">{task.completed ? "done" : task.priority}</span>
            </div>
          {:else}
            <p class="empty-state">No tasks due today.</p>
          {/each}
        </div>
      {/if}
    {:else if widget.kind === "focus"}
      <form class="focus-widget-form" onsubmit={submitFocus}>
        <p class="widget-kicker">Next focus</p>
        <label class="focus-widget-goal" for={`focus-goal-${widget.id}`}>
          <span>Goal</span>
          <input
            id={`focus-goal-${widget.id}`}
            class="text-input"
            bind:value={focusGoal}
            placeholder={nextTask?.title ?? "What needs your attention?"}
            maxlength="120"
          />
        </label>
        <label class="focus-widget-duration" for={`focus-minutes-${widget.id}`}>
          <span>Timer</span>
          <span class="focus-widget-duration-control">
            <input
              id={`focus-minutes-${widget.id}`}
              type="number"
              bind:value={focusMinutes}
              min="1"
              max="240"
              step="1"
              inputmode="numeric"
              required
            />
            <small>min</small>
          </span>
        </label>
        <button
          class="ui-button ui-button--secondary secondary-btn focus-widget-submit"
          type="submit"
        >
          Start focus
          <ArrowRight size={17} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </form>
    {:else if widget.kind === "task-list"}
      <div class="widget-head">
        <h2>Tasks</h2>
        <button
          class="ui-button ui-button--danger"
          onclick={onClearCompleted}
          disabled={completedCount === 0}>Clear completed</button
        >
      </div>
      <div class="task-list">
        {#each tasks.slice(0, visibleTaskCount) as task (task.id)}
          <div
            class={["task-row", task.completed && "done"]}
            data-od-id={`task-row-${task.id}`}
          >
            <button
              class="task-check"
              aria-label={task.completed
                ? `Mark ${task.title} incomplete`
                : `Mark ${task.title} complete`}
              onclick={() => onToggleTask(task)}
            >
              <Check size={17} strokeWidth={2} aria-hidden="true" />
            </button>
            <span class="task-copy">{task.title}</span>
            {#if showTaskPriorities}
              <span class="priority"
                >{task.completed ? "done" : task.priority}</span
              >
            {/if}
          </div>
        {:else}
          <p class="empty-state roomy">No tasks yet. Add one below.</p>
        {/each}
      </div>
      <form class="add-task" onsubmit={submitTask}>
        <label class="sr-only" for={`new-task-${widget.id}`}>New task</label>
        <input
          class="text-input"
          id={`new-task-${widget.id}`}
          bind:value={newTaskTitle}
          placeholder="Add a task…"
          maxlength="120"
          required
        />
        <button
          class="ui-button ui-button--primary primary-btn"
          type="submit"
          disabled={savingTask}
        >
          {savingTask ? "Adding…" : "Add task"}
        </button>
      </form>
    {:else}
      <IntegrationWidget
        {widget}
        onUpdate={onUpdateWidget}
        {onToast}
        {onOpenCalendarDate}
        onRegisterEditor={(editor) => onRegisterEditor(widget.id, editor)}
      />
    {/if}
  </div>
</article>

<script lang="ts">
  import ArrowRight from "lucide-svelte/icons/arrow-right";
  import Check from "lucide-svelte/icons/check";
  import GripVertical from "lucide-svelte/icons/grip-vertical";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import IntegrationWidget from "$lib/IntegrationWidget.svelte";
  import WeatherWidget from "$lib/WeatherWidget.svelte";
  import type { DashboardWidget, FeedItem, Task, UserSettings } from "$lib/api";

  type FeedFilter = "All" | FeedItem["category"];

  let {
    widget,
    editing,
    tasks,
    feeds,
    settings,
    completedCount,
    taskProgress,
    todayTasks,
    todayCompletedCount,
    todayTaskProgress,
    savingLayout,
    onToggleTask,
    onCreateTask,
    onClearCompleted,
    onStartFocus,
    onToast,
    onOpenCalendarDate,
    onRemove,
    onUpdateWidget,
  }: {
    widget: DashboardWidget;
    editing: boolean;
    tasks: Task[];
    feeds: FeedItem[];
    settings: UserSettings;
    completedCount: number;
    taskProgress: number;
    todayTasks: Task[];
    todayCompletedCount: number;
    todayTaskProgress: number;
    savingLayout: boolean;
    onToggleTask: (task: Task) => void;
    onCreateTask: (title: string) => Promise<void>;
    onClearCompleted: () => void;
    onStartFocus: (subject: string, minutes: number) => void;
    onToast: (message: string) => void;
    onOpenCalendarDate: (date: string) => void;
    onRemove: (widget: DashboardWidget) => void;
    onUpdateWidget: (widget: DashboardWidget) => void;
  } = $props();

  const feedFilters: FeedFilter[] = ["All", "Design", "Technology", "Culture"];

  let activeFilter = $state<FeedFilter>("All");
  let newTaskTitle = $state("");
  let savingTask = $state(false);
  let focusGoal = $state("");
  let focusMinutes = $state(25);
  let filteredFeeds = $derived(
    activeFilter === "All"
      ? feeds
      : feeds.filter((feed) => feed.category === activeFilter),
  );
  let nextTask = $derived(tasks.find((task) => !task.completed));
  let feedSources = $derived.by(() => {
    const counts: Record<string, number> = {};
    for (const feed of feeds) {
      counts[feed.source] = (counts[feed.source] ?? 0) + 1;
    }
    return Object.entries(counts).map(([source, count]) => ({ source, count }));
  });
  let visibleTaskCount = $derived(
    widget.size === "compact"
      ? 3
      : widget.size === "standard"
        ? 5
        : tasks.length,
  );
  let visibleFeedCount = $derived(
    widget.size === "compact"
      ? 2
      : widget.size === "standard"
        ? 3
        : filteredFeeds.length,
  );
  let widgetTitle = $derived(
    {
      weather: "Weather",
      "task-summary": "Today",
      focus: "Next focus",
      "task-list": "Tasks",
      "task-progress": "Progress",
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
    }[widget.kind] ?? "Widget",
  );

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
  aria-label={editing
    ? `${widgetTitle} widget. Use the grip to move.`
    : undefined}
>
  {#if editing}
    <div class="widget-edit-bar" aria-label={`${widgetTitle} layout controls`}>
      <button
        class="widget-drag-handle"
        type="button"
        disabled={savingLayout}
        aria-label={`Move ${widgetTitle} widget`}
      >
        <GripVertical size={17} strokeWidth={1.8} aria-hidden="true" />
        <span>Move</span>
      </button>
      <span class="widget-resize-hint">Drag an edge to resize</span>
      <button
        class="ui-button ui-button--danger ui-button--icon widget-remove"
        type="button"
        disabled={savingLayout}
        aria-label={`Remove ${widgetTitle} widget`}
        onclick={() => onRemove(widget)}
      >
        <Trash2 size={17} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>
  {/if}

  <div class="widget-content" inert={editing}>
    {#if widget.kind === "weather"}
      <WeatherWidget {widget} {settings} onUpdate={onUpdateWidget} {onToast} />
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
            <span class="priority"
              >{task.completed ? "done" : task.priority}</span
            >
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
    {:else if widget.kind === "task-progress"}
      <p class="widget-kicker">Progress</p>
      <div class="task-count mono">{completedCount} / {tasks.length}</div>
      <p class="task-count-label">
        Tasks completed. Select any task checkbox to update progress.
      </p>
      <div class="progress-track progress-spaced">
        <div class="progress-fill" style:--progress={`${taskProgress}%`}></div>
      </div>
    {:else if widget.kind === "feed-list"}
      <div class="widget-head feed-widget-head">
        <div class="filter-row" aria-label="Feed filters">
          {#each feedFilters as filter (filter)}
            <button
              class="filter-btn"
              aria-pressed={activeFilter === filter}
              onclick={() => (activeFilter = filter)}>{filter}</button
            >
          {/each}
        </div>
        <button
          class="ui-button ui-button--secondary secondary-btn"
          onclick={() => onToast("Feeds are current")}>Refresh</button
        >
      </div>
      <div class="feed-list">
        {#each filteredFeeds.slice(0, visibleFeedCount) as feed (feed.id)}
          <article class="feed-row">
            <span class="feed-source">{feed.source}</span>
            <div>
              <h3>{feed.title}</h3>
              <p>{feed.summary}</p>
              <span class="reading-time mono"
                >{feed.reading_minutes} min read</span
              >
            </div>
            <button
              class="arrow-link"
              aria-label={`Open ${feed.title}`}
              onclick={() => onToast("Article preview coming next")}
              ><ArrowRight
                size={17}
                strokeWidth={1.8}
                aria-hidden="true"
              /></button
            >
          </article>
        {:else}
          <p class="empty-state roomy">No feed items in this category.</p>
        {/each}
      </div>
    {:else if widget.kind === "feed-sources"}
      <h2>Following</h2>
      <div class="source-list">
        {#each feedSources as source (source.source)}
          <div class="source-row">
            <span>{source.source}</span>
            <span class="mono muted">{source.count}</span>
          </div>
        {:else}
          <p class="empty-state">No sources are available.</p>
        {/each}
      </div>
    {:else}
      <IntegrationWidget
        {widget}
        onUpdate={onUpdateWidget}
        {onToast}
        {onOpenCalendarDate}
      />
    {/if}
  </div>
</article>

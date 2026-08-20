<script lang="ts">
  import { GridStack, type GridItemHTMLElement } from "gridstack";
  import "gridstack/dist/gridstack.min.css";
  import ArrowRight from "lucide-svelte/icons/arrow-right";
  import ArchiveIcon from "lucide-svelte/icons/archive";
  import Bell from "lucide-svelte/icons/bell";
  import BookOpen from "lucide-svelte/icons/book-open";
  import CalendarDays from "lucide-svelte/icons/calendar-days";
  import ChartCandlestick from "lucide-svelte/icons/chart-candlestick";
  import Check from "lucide-svelte/icons/check";
  import CheckSquare2 from "lucide-svelte/icons/square-check-big";
  import ChevronDown from "lucide-svelte/icons/chevron-down";
  import ChevronLeft from "lucide-svelte/icons/chevron-left";
  import Code2 from "lucide-svelte/icons/code-xml";
  import Columns3 from "lucide-svelte/icons/columns-3";
  import ContactRound from "lucide-svelte/icons/contact-round";
  import Ellipsis from "lucide-svelte/icons/ellipsis";
  import Home from "lucide-svelte/icons/house";
  import ImageIcon from "lucide-svelte/icons/image";
  import Menu from "lucide-svelte/icons/menu";
  import MessageSquareText from "lucide-svelte/icons/message-square-text";
  import PanelTop from "lucide-svelte/icons/panel-top";
  import Paperclip from "lucide-svelte/icons/paperclip";
  import Pause from "lucide-svelte/icons/pause";
  import Pencil from "lucide-svelte/icons/pencil";
  import Play from "lucide-svelte/icons/play";
  import Plus from "lucide-svelte/icons/plus";
  import Repeat2 from "lucide-svelte/icons/repeat-2";
  import RotateCcw from "lucide-svelte/icons/rotate-ccw";
  import ReceiptText from "lucide-svelte/icons/receipt-text";
  import Podcast from "lucide-svelte/icons/podcast";
  import RotateCw from "lucide-svelte/icons/rotate-cw";
  import Rss from "lucide-svelte/icons/rss";
  import Search from "lucide-svelte/icons/search";
  import Settings from "lucide-svelte/icons/settings";
  import SkipBack from "lucide-svelte/icons/skip-back";
  import SkipForward from "lucide-svelte/icons/skip-forward";
  import SlidersHorizontal from "lucide-svelte/icons/sliders-horizontal";
  import Star from "lucide-svelte/icons/star";
  import Tag from "lucide-svelte/icons/tag";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import Volume1 from "lucide-svelte/icons/volume-1";
  import Volume2 from "lucide-svelte/icons/volume-2";
  import VolumeOff from "lucide-svelte/icons/volume-off";
  import Wallpaper from "lucide-svelte/icons/wallpaper";
  import X from "lucide-svelte/icons/x";
  import Youtube from "lucide-svelte/icons/circle-play";
  import { onDestroy, onMount, tick } from "svelte";
  import { createViewSwap } from "$lib/viewSwap.svelte";
  import { MediaQuery, SvelteMap, SvelteSet } from "svelte/reactivity";
  import AnimatedList from "$lib/components/AnimatedList.svelte";
  import PrismaticBurst from "$lib/components/PrismaticBurst.svelte";
  import CalendarPage from "$lib/CalendarPage.svelte";
  import CodingPage from "$lib/CodingPage.svelte";
  import ContactsPage from "$lib/ContactsPage.svelte";
  import DashboardWidgetCard from "$lib/DashboardWidgetCard.svelte";
  import EmbeddedPage from "$lib/EmbeddedPage.svelte";
  import EmbeddedPagesSettings from "$lib/EmbeddedPagesSettings.svelte";
  import JournalPage from "$lib/JournalPage.svelte";
  import KanbanPage from "$lib/KanbanPage.svelte";
  import LinesPage from "$lib/LinesPage.svelte";
  import PodcastsPage from "$lib/PodcastsPage.svelte";
  import RssReaderPage from "$lib/RssReaderPage.svelte";
  import SidebarUtilities from "$lib/SidebarUtilities.svelte";
  import SubscriptionsPage from "$lib/SubscriptionsPage.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import WallsPage from "$lib/WallsPage.svelte";
  import YoutubePage from "$lib/YoutubePage.svelte";
  import {
    MAX_PLAYBACK_VOLUME,
    SKIP_BACK_SECONDS,
    SKIP_FORWARD_SECONDS,
    formatPlaybackTime,
    podcastPlayer,
  } from "$lib/podcastPlayer.svelte";
  import {
    archiveTask,
    clearCompletedTasks,
    createAdministrator,
    createDashboardWidget,
    createTask,
    deleteAvatar,
    deleteDashboardWidget,
    deleteManagedUser,
    deleteTask,
    deleteTaskAttachment,
    deleteUserContent,
    deleteWallpaper,
    fetchAuthenticationSettings,
    fetchArchivedTasks,
    fetchDashboard,
    fetchManagedUsers,
    loginAccount,
    logoutAccount,
    registerAccount,
    restoreTask,
    setTaskCompleted,
    taskAttachmentUrl,
    updateTask,
    updateAppearance,
    updateAuthenticationSettings,
    updateAvatar,
    updateDashboardWidgetLayout,
    updateManagedUserRole,
    updateUserSettings,
    updateWallpaper,
    uploadTaskAttachment,
    type AuthenticationConfig,
    type DashboardWidget,
    type EmbeddedPage as EmbeddedPageRecord,
    type EmbeddedPagesResponse,
    type FeedItem,
    type ManagedUser,
    type KanbanSection,
    type Task,
    type TaskAttachment,
    type TaskInput,
    type UserSettings,
    type UserContentScope,
    type WallpaperSlot,
    type WallSlot,
    type WidgetKind,
    type WidgetSize,
  } from "$lib/api";
  import type { PageData } from "./$types";

  type AuthMode = "login" | "register";
  type WallpaperDraft = {
    file: File | null;
    preview: string;
    reset: boolean;
  };
  type ProductPage =
    | "dashboard"
    | "tasks"
    | "kanban"
    | "contacts"
    | "calendar"
    | "rss"
    | "journal"
    | "lines"
    | "walls"
    | "youtube"
    | "podcasts"
    | "coding"
    | "subscriptions"
    | "trading";
  type ActivePage =
    | { kind: "builtin"; id: ProductPage }
    | { kind: "embedded"; id: string };
  type CommandGroup = "PAGES" | "ACTIONS" | "WEB";
  type CommandItem = {
    id: string;
    group: CommandGroup;
    label: string;
    hint: string;
    keywords: string;
    run: () => void;
  };
  type TaskView = "active" | "archived";
  type TaskDueGroup = {
    id: "today" | "this-week" | "next-week" | "later" | "never";
    label: string;
    range: string;
    tasks: Task[];
  };
  type DashboardCalendarDay = {
    key: string;
    day: number;
    currentMonth: boolean;
    today: boolean;
  };

  const appearanceWallpaperOptions: Array<{
    id: WallpaperSlot;
    code: string;
    title: string;
    description: string;
    adminOnly: boolean;
  }> = [
    {
      id: "login",
      code: "PUBLIC SURFACE",
      title: "Login",
      description: "The global pre-authentication image for every visitor.",
      adminOnly: true,
    },
  ];

  const userWallpaperOptions: Array<{
    id: WallpaperSlot;
    code: string;
    title: string;
    description: string;
  }> = [
    {
      id: "welcome",
      code: "MAIN",
      title: "Main background",
      description:
        "Used by the Welcome loading screen and throughout authenticated pages.",
    },
  ];

  const allWallpaperSlots: WallpaperSlot[] = [
    "dashboard",
    "welcome",
    "loading",
    "login",
  ];

  const destructiveContentActions: Array<{
    scope: UserContentScope;
    title: string;
    description: string;
  }> = [
    {
      scope: "contacts",
      title: "All Contacts",
      description: "Contacts, Profile Pictures, And CardDAV Sources.",
    },
    {
      scope: "tasks",
      title: "All Tasks",
      description:
        "Active And Archived Tasks, Subtasks, Labels, And Attachments.",
    },
    {
      scope: "calendar",
      title: "All Calendars",
      description: "Calendar Subscriptions And Their Cached Events.",
    },
    {
      scope: "rss",
      title: "All RSS Feeds",
      description: "Feed Subscriptions, Articles, Categories, And Read State.",
    },
    {
      scope: "journal",
      title: "All Journal Entries",
      description: "Every Journal Document And Nested Entry.",
    },
    {
      scope: "lines",
      title: "All Lines Posts",
      description:
        "Your Public And Private Posts, Replies, Reactions, And Attachments.",
    },
    {
      scope: "youtube",
      title: "All YouTube Data",
      description: "Your Channel Subscriptions, Groups, And Display Settings.",
    },
    {
      scope: "podcasts",
      title: "All Podcast Data",
      description:
        "Your Subscriptions, Play Queue, Saved Episodes, Progress, And Requests.",
    },
    {
      scope: "coding",
      title: "All Coding Projects",
      description: "Tracked Repositories And Saved Provider Credentials.",
    },
    {
      scope: "subscriptions",
      title: "All Paid Subscriptions",
      description: "Every Recurring Service And Its Cost History.",
    },
  ];

  let { data }: { data: PageData } = $props();

  const widgetCatalog: Array<{
    kind: WidgetKind;
    title: string;
    description: string;
    size: WidgetSize;
  }> = [
    {
      kind: "weather",
      title: "Weather",
      description: "Local conditions with an adaptive forecast.",
      size: "wide",
    },
    {
      kind: "bible-verse",
      title: "Bible Verse",
      description: "A new passage from the English Revised Version each day.",
      size: "standard",
    },
    {
      kind: "task-summary",
      title: "Today",
      description: "Tasks due today with a compact completion overview.",
      size: "compact",
    },
    {
      kind: "focus",
      title: "Next focus",
      description: "Set a goal and launch a custom focus timer.",
      size: "standard",
    },
    {
      kind: "task-list",
      title: "Task list",
      description: "Create, complete, and clear personal tasks.",
      size: "wide",
    },
    {
      kind: "task-progress",
      title: "Task progress",
      description: "A focused completion readout.",
      size: "compact",
    },
    {
      kind: "feed-list",
      title: "Feed",
      description: "Filter and read the curated feed.",
      size: "wide",
    },
    {
      kind: "feed-sources",
      title: "Feed sources",
      description: "A compact source breakdown.",
      size: "compact",
    },
    {
      kind: "youtube",
      title: "YouTube uploads",
      description: "Channel and playlist uploads using feed-based discovery.",
      size: "wide",
    },
    {
      kind: "rss",
      title: "RSS feeds",
      description: "Combine RSS and Atom sources into one reading list.",
      size: "wide",
    },
    {
      kind: "reddit",
      title: "Reddit",
      description: "Follow a subreddit with optional app authentication.",
      size: "standard",
    },
    {
      kind: "stocks",
      title: "Stock tickers",
      description: "Track equities, funds, and crypto market symbols.",
      size: "wide",
    },
    {
      kind: "calendar",
      title: "Calendar",
      description: "A personal date card with upcoming events.",
      size: "standard",
    },
    {
      kind: "clock",
      title: "World clock",
      description: "Show one or several IANA timezones.",
      size: "compact",
    },
    {
      kind: "iframe",
      title: "Custom iframe",
      description: "Embed a sandboxed HTTPS dashboard surface.",
      size: "full",
    },
    {
      kind: "html",
      title: "Custom HTML",
      description: "Render static HTML inside an isolated sandbox.",
      size: "standard",
    },
    {
      kind: "releases",
      title: "Code releases",
      description: "GitHub, GitLab, Codeberg, Gitea, and Forgejo releases.",
      size: "wide",
    },
    {
      kind: "streams",
      title: "Live channels",
      description: "Twitch or Kick channel availability.",
      size: "standard",
    },
  ];

  const clockMarks = Array.from({ length: 12 }, (_, index) => index);
  const dashboardCalendarWeekdays = ["M", "T", "W", "T", "F", "S", "S"];
  const focusDurations = [15, 25, 45] as const;
  const kanbanSubmenuItems: Array<{
    id: KanbanSection;
    label: string;
    description: string;
  }> = [
    {
      id: "boards",
      label: "Boards",
      description: "Open shared boards and move work through lists and cards.",
    },
    {
      id: "workspaces",
      label: "Workspaces",
      description: "Manage Kanban collaborators, roles, and workspace access.",
    },
    {
      id: "invitations",
      label: "Invitations",
      description: "Review and respond to workspace invitations.",
    },
  ];

  const searchEngines = [
    {
      id: "duckduckgo",
      label: "DuckDuckGo",
      url: "https://duckduckgo.com/?q=",
    },
    { id: "google", label: "Google", url: "https://www.google.com/search?q=" },
    { id: "bing", label: "Bing", url: "https://www.bing.com/search?q=" },
    { id: "brave", label: "Brave", url: "https://search.brave.com/search?q=" },
  ] as const;
  type SearchEngineId = (typeof searchEngines)[number]["id"];
  const searchEngineStorageKey = "pandan-search-engine";

  const productPages = [
    {
      id: "dashboard",
      label: "Dashboard",
      description: "Arrange widgets and see your day at a glance.",
      code: "01",
      icon: Home,
    },
    {
      id: "tasks",
      label: "Tasks",
      description: "Capture, organize, and complete personal to-dos.",
      code: "02",
      icon: CheckSquare2,
    },
    {
      id: "kanban",
      label: "Kanban",
      description: "Plan shared work across boards, lists, and cards.",
      code: "03",
      icon: Columns3,
    },
    {
      id: "contacts",
      label: "Contacts",
      description: "Keep people, contact details, and important dates together.",
      code: "04",
      icon: ContactRound,
    },
    {
      id: "calendar",
      label: "Calendar",
      description: "Review events and subscribed calendars in one schedule.",
      code: "05",
      icon: CalendarDays,
    },
    {
      id: "rss",
      label: "RSS",
      description: "Read and manage posts from your subscribed feeds.",
      code: "06",
      icon: Rss,
    },
    {
      id: "journal",
      label: "Journal",
      description: "Write private daily notes and longer entries.",
      code: "07",
      icon: BookOpen,
    },
    {
      id: "lines",
      label: "Lines",
      description: "Share short posts with people on this Pandan instance.",
      code: "08",
      icon: MessageSquareText,
    },
    {
      id: "walls",
      label: "Walls",
      description: "Browse shared wallpapers and submit your own.",
      code: "09",
      icon: Wallpaper,
    },
    {
      id: "youtube",
      label: "YouTube",
      description: "Follow channels and browse recent videos without distractions.",
      code: "10",
      icon: Youtube,
    },
    {
      id: "podcasts",
      label: "Podcasts",
      description: "Listen to shows this instance hosts, and ask for new ones.",
      code: "11",
      icon: Podcast,
    },
    {
      id: "coding",
      label: "Coding",
      description: "Track projects, repositories, and release activity.",
      code: "12",
      icon: Code2,
    },
    {
      id: "subscriptions",
      label: "Subscriptions",
      description: "Monitor recurring services, costs, and renewal dates.",
      code: "13",
      icon: ReceiptText,
    },
    {
      id: "trading",
      label: "Trading",
      description: "Plan watchlists, market notes, and trades.",
      code: "14",
      icon: ChartCandlestick,
    },
  ] as const;

  const placeholderPages = {
    trading: {
      description:
        "A focused market workspace for watchlists and trade planning.",
      primaryTitle: "Create a watchlist",
      primaryCopy:
        "Selected tickers, market notes, and position planning will be available here.",
      modules: ["Watchlist", "Market notes", "Trade plan"],
    },
  } as const;

  let activePage = $state<ActivePage>({ kind: "builtin", id: "dashboard" });
  let activeSection = $derived(
    activePage.kind === "builtin" ? activePage.id : null,
  );
  /** Incremented on every Lines sidebar activation, to send the page back to its timeline. */
  let linesHomeToken = $state(0);
  let contactDetailId = $state<string | null>(null);
  let calendarDetailDate = $state<string | null>(null);
  let kanbanSection = $state<KanbanSection>("boards");
  let kanbanMenuOpen = $state(false);
  let sidebarOpen = $state(false);
  let sidebarCollapsed = $state(false);
  let sidebarElement = $state<HTMLElement>();
  let sidebarHint = $state<{
    title: string;
    description: string;
    top: number;
    source: HTMLElement;
  } | null>(null);
  let initialLoadingPending = $state(true);
  let welcomeLeaving = $state(false);
  let dashboard = $derived(data.dashboard);
  let activeEmbeddedPage = $derived.by<EmbeddedPageRecord | null>(() => {
    if (activePage.kind !== "embedded") return null;
    return (
      dashboard?.embedded_pages.global.find(
        (page) => page.id === activePage.id,
      ) ??
      dashboard?.embedded_pages.personal.find(
        (page) => page.id === activePage.id,
      ) ??
      null
    );
  });
  let setupRequired = $derived(data.setup.required);
  let tasks = $derived<Task[]>(dashboard?.tasks ?? []);
  let archivedTasks = $state.raw<Task[]>([]);
  let taskView = $state<TaskView>("active");
  let taskViewTarget = $state<TaskView>("active");
  const taskViewSwap = createViewSwap();
  let taskLabelFilter = $state("");
  let archivedTasksLoaded = $state(false);
  let loadingArchivedTasks = $state(false);
  let archivedTasksError = $state("");
  let feeds = $derived<FeedItem[]>(dashboard?.feeds ?? []);
  let widgets = $derived<DashboardWidget[]>(dashboard?.widgets ?? []);
  let savingLayout = $state(false);
  let layoutEditing = $state(false);
  let addingWidgetKind = $state<WidgetKind | "">("");
  let draggedWidgetId = $state("");
  let toastMessage = $state("");
  let currentTime = $state(new Date());
  let focusSubject = $state("");
  let focusDurationMinutes = $state(25);
  let focusRemainingSeconds = $state(25 * 60);
  let focusRunning = $state(false);
  let focusLeaving = $state(false);
  let focusSettingsOpen = $state(false);
  let burstIntensity = $state(1.7);
  let burstSpeed = $state(0.34);
  let burstDistort = $state(0.35);
  let burstHoverDampness = $state(0.2);
  let burstRayCount = $state(18);
  let burstPaused = $state(false);
  let authConfig = $derived<AuthenticationConfig>(data.auth);
  let authMode = $derived<AuthMode>(
    authConfig.password_login_enabled ? "login" : "register",
  );
  let authEmail = $state("");
  let authPassword = $state("");
  let authDisplayName = $state("");
  let authError = $state("");
  let authenticating = $state(false);
  let loadingScreenReady = $state(false);
  let settingsDisplayName = $state("");
  let settingsLocation = $state("");
  let settingsTimezone = $state("");
  let settingsTemperatureUnit =
    $state<UserSettings["temperature_unit"]>("celsius");
  let settingsLinesDefaultVisibility =
    $state<UserSettings["lines_default_visibility"]>("private");
  // The shell owns the single audio element so playback survives section changes.
  let podcastAudio = $state<HTMLAudioElement>();
  let podcastVolumeOpen = $state(false);
  let podcastVolumeControl = $state<HTMLDivElement>();
  let avatarRevision = $state(Date.now());
  let avatarAvailable = $state(true);
  let avatarFile = $state<File | null>(null);
  let avatarPreview = $state("");
  let avatarReset = $state(false);
  let wallpaperRevisions = $state<Record<WallpaperSlot, number>>({
    dashboard: 0,
    welcome: 0,
    loading: 0,
    login: 0,
  });
  let settingsError = $state("");
  let savingSettings = $state(false);
  let managedUsers = $state.raw<ManagedUser[]>([]);
  let loadingUsers = $state(false);
  let mutatingUserId = $state("");
  let pendingRemovalId = $state("");
  let adminError = $state("");
  let passwordLoginEnabled = $state(true);
  let passwordRegistrationEnabled = $state(true);
  let oidcRegistrationEnabled = $state(true);
  let savingAuthenticationSettings = $state(false);
  let commandDialog = $state<HTMLDialogElement>();
  let commandSearchInput = $state<HTMLInputElement>();
  let commandQuery = $state("");
  let commandIndex = $state(0);
  let searchEngine = $state<SearchEngineId>("duckduckgo");
  let settingsDialog = $state<HTMLDialogElement>();
  let embeddedPagesSettingsOpen = $state(false);
  let settingsScrollContainer = $state<HTMLDivElement>();
  let destructiveDialog = $state<HTMLDialogElement>();
  let adminDialog = $state<HTMLDialogElement>();
  let widgetLibraryDialog = $state<HTMLDialogElement>();
  let appearanceDialog = $state<HTMLDialogElement>();
  let pendingContentDeletion = $state<UserContentScope | null>(null);
  let deletingContentScope = $state<UserContentScope | null>(null);
  let destructiveError = $state("");
  let taskEditorDialog = $state<HTMLDialogElement>();
  let focusDialog = $state<HTMLDialogElement>();
  let wallpaperDrafts = $state<Record<WallpaperSlot, WallpaperDraft>>({
    dashboard: { file: null, preview: "", reset: false },
    welcome: { file: null, preview: "", reset: false },
    loading: { file: null, preview: "", reset: false },
    login: { file: null, preview: "", reset: false },
  });
  let backgroundBlur = $state(0);
  let backgroundBrightness = $state(78);
  let backgroundContrast = $state(108);
  let backgroundSaturation = $state(72);
  let appearanceError = $state("");
  let savingAppearance = $state(false);
  let editingTaskId = $state<string | null>(null);
  let taskName = $state("");
  let taskDescription = $state("");
  let taskPriority = $state<Task["priority"]>("none");
  let taskDueDate = $state("");
  let taskLabels = $state("");
  let taskRepeatRule = $state<Task["repeat_rule"]>("none");
  let taskRepeatInterval = $state(1);
  let taskRepeatUnit = $state<Task["repeat_unit"]>("days");
  let taskRescheduleFrom = $state<Task["reschedule_from"]>("due_date");
  let taskSubtasks = $state<
    Array<{ id?: string; title: string; completed: boolean }>
  >([]);
  let taskAttachments = $state.raw<TaskAttachment[]>([]);
  let pendingTaskFiles = $state.raw<File[]>([]);
  let taskEditorError = $state("");
  let savingTask = $state(false);
  let taskActionId = $state("");
  let subtaskActionId = $state("");
  let pendingTaskDeleteId = $state("");
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let clockTimer: ReturnType<typeof setInterval> | undefined;
  let focusTimer: ReturnType<typeof setInterval> | undefined;
  let focusExitTimer: ReturnType<typeof setTimeout> | undefined;
  let welcomeRemoveTimer: ReturnType<typeof setTimeout> | undefined;
  let loadingAnimationMinimumTimer: ReturnType<typeof setTimeout> | undefined;
  let dragSnapshot: DashboardWidget[] = [];
  const gridInstances = new SvelteMap<number, GridStack>();
  const expandedTaskIds = new SvelteSet<string>();
  const mobileNavigation = new MediaQuery("max-width: 720px", false);

  let completedCount = $derived(tasks.filter((task) => task.completed).length);
  let taskProgress = $derived(
    tasks.length === 0 ? 0 : Math.round((completedCount / tasks.length) * 100),
  );
  let todayTasks = $derived(
    tasks.filter(
      (task) =>
        task.due_date !== null &&
        taskDayDistance(
          task.due_date,
          currentTime,
          dashboard?.settings.timezone || "UTC",
        ) === 0,
    ),
  );
  let todayCompletedCount = $derived(
    todayTasks.filter((task) => task.completed).length,
  );
  let todayTaskProgress = $derived(
    todayTasks.length === 0
      ? 0
      : Math.round((todayCompletedCount / todayTasks.length) * 100),
  );
  let taskLabelOptions = $derived.by(() => {
    const visibleTasks = taskView === "active" ? tasks : archivedTasks;
    const labels = visibleTasks
      .flatMap((task) =>
        task.labels.map((label) => label.trim()).filter(Boolean),
      )
      .filter((label, index, allLabels) => allLabels.indexOf(label) === index);
    if (taskLabelFilter && !labels.includes(taskLabelFilter)) {
      labels.push(taskLabelFilter);
    }
    return labels.sort((left, right) => left.localeCompare(right));
  });
  let filteredActiveTasks = $derived(
    taskLabelFilter
      ? tasks.filter((task) => task.labels.includes(taskLabelFilter))
      : tasks,
  );
  let filteredArchivedTasks = $derived(
    taskLabelFilter
      ? archivedTasks.filter((task) => task.labels.includes(taskLabelFilter))
      : archivedTasks,
  );
  let taskDueGroups = $derived(
    groupTasksByDueDate(
      filteredActiveTasks,
      currentTime,
      dashboard?.settings.timezone || "UTC",
    ),
  );
  let profileInitials = $derived(
    dashboard?.settings.display_name
      .split(/\s+/)
      .slice(0, 2)
      .map((part) => part[0]?.toUpperCase())
      .join("") || "ME",
  );
  let administratorCount = $derived(
    managedUsers.filter((user) => user.role === "administrator").length,
  );
  let passwordAccessEnabled = $derived(
    authConfig.password_login_enabled ||
      authConfig.password_registration_enabled,
  );
  let activeSectionLabel = $derived(
    activeEmbeddedPage?.title ??
      productPages.find((item) => item.id === activeSection)?.label ??
      "Dashboard",
  );
  let firstName = $derived(
    dashboard?.settings.display_name.trim().split(/\s+/)[0] || "there",
  );
  let loadingWelcomeName = $state("");
  let loadingWelcomeDisplayName = $derived(
    loadingWelcomeName ||
      dashboard?.settings.display_name.trim().split(/\s+/)[0] ||
      "there",
  );
  let loadingOverlayVisible = $derived(
    (Boolean(dashboard) && initialLoadingPending) ||
      (authenticating && loadingScreenReady),
  );
  let placeholderPage = $derived(
    activeSection && activeSection in placeholderPages
      ? placeholderPages[activeSection as keyof typeof placeholderPages]
      : null,
  );
  let commandItems = $derived.by<CommandItem[]>(() => {
    const items: CommandItem[] = productPages.map((page) => ({
      id: `page:${page.id}`,
      group: "PAGES",
      label: page.label,
      hint: page.code,
      keywords: `${page.label} ${page.code} ${page.description}`,
      run: () => openProductPage(page.id),
    }));

    for (const submenuItem of kanbanSubmenuItems) {
      items.push({
        id: `kanban:${submenuItem.id}`,
        group: "PAGES",
        label: `Kanban / ${submenuItem.label}`,
        hint: "03",
        keywords: `kanban ${submenuItem.label} ${submenuItem.description}`,
        run: () => openKanbanSection(submenuItem.id),
      });
    }

    for (const page of dashboard?.embedded_pages.global ?? []) {
      items.push({
        id: `embedded:global:${page.id}`,
        group: "PAGES",
        label: page.title,
        hint: "GLOBAL · CUSTOM",
        keywords: `${page.title} ${page.description} global custom embedded`,
        run: () => openEmbeddedPage(page.id),
      });
    }
    for (const page of dashboard?.embedded_pages.personal ?? []) {
      items.push({
        id: `embedded:personal:${page.id}`,
        group: "PAGES",
        label: page.title,
        hint: "PERSONAL · CUSTOM",
        keywords: `${page.title} ${page.description} personal custom embedded`,
        run: () => openEmbeddedPage(page.id),
      });
    }

    items.push(
      {
        id: "action:new-task",
        group: "ACTIONS",
        label: "New task",
        hint: "+",
        keywords: "new task create todo add",
        run: () => {
          openProductPage("tasks");
          openTaskEditor();
        },
      },
      {
        id: "action:focus",
        group: "ACTIONS",
        label: "Start focus session",
        hint: "+",
        keywords: "focus timer pomodoro session deep work",
        run: startFocusSession,
      },
      {
        id: "action:add-widget",
        group: "ACTIONS",
        label: "Add dashboard widget",
        hint: "+",
        keywords: "add widget dashboard library",
        run: () => {
          openProductPage("dashboard");
          openWidgetLibrary();
        },
      },
      {
        id: "action:edit-layout",
        group: "ACTIONS",
        label: layoutEditing ? "Exit dashboard edit mode" : "Edit dashboard layout",
        hint: "+",
        keywords: "edit layout dashboard arrange move resize grid",
        run: () => {
          openProductPage("dashboard");
          void toggleLayoutEditing();
        },
      },
      {
        id: "action:settings",
        group: "ACTIONS",
        label: "Account settings",
        hint: "+",
        keywords: "settings account preferences profile",
        run: openSettings,
      },
      {
        id: "action:sign-out",
        group: "ACTIONS",
        label: "Sign out",
        hint: "+",
        keywords: "sign out log out logout leave",
        run: () => void signOut(),
      },
    );

    return items;
  });

  let commandResults = $derived.by<CommandItem[]>(() => {
    const query = commandQuery.trim();
    const needle = query.toLowerCase();
    const matches = needle
      ? commandItems.filter((item) =>
          item.keywords.toLowerCase().includes(needle),
        )
      : commandItems;
    if (!query) return matches;
    const engine =
      searchEngines.find((option) => option.id === searchEngine) ??
      searchEngines[0];
    return [
      ...matches,
      {
        id: "web:search",
        group: "WEB",
        label: `Search “${query}” on ${engine.label}`,
        hint: "↗",
        keywords: query,
        run: () => {
          window.open(
            `${engine.url}${encodeURIComponent(query)}`,
            "_blank",
            "noopener,noreferrer",
          );
        },
      },
    ];
  });

  let hasLocalCommandMatches = $derived(
    commandResults.some((item) => item.group !== "WEB"),
  );

  let commandGroups = $derived.by(() => {
    const order: CommandGroup[] = ["PAGES", "ACTIONS", "WEB"];
    return order
      .map((group) => ({
        group,
        items: commandResults.filter((item) => item.group === group),
      }))
      .filter((entry) => entry.items.length > 0);
  });
  let dashboardClock = $derived(
    clockDisplay(currentTime, dashboard?.settings.timezone || "UTC"),
  );
  let dashboardTimezone = $derived(
    normalizeTimezone(dashboard?.settings.timezone || "UTC"),
  );
  let dashboardCalendarDate = $derived(
    dateInTimezone(currentTime, dashboardTimezone),
  );
  let dateLabel = $derived(
    new Intl.DateTimeFormat("en", {
      timeZone: dashboardTimezone,
      weekday: "short",
      month: "short",
      day: "numeric",
    }).format(currentTime),
  );
  let timeLabel = $derived(
    new Intl.DateTimeFormat("en", {
      timeZone: dashboardTimezone,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    }).format(currentTime),
  );
  let timezoneLabel = $derived(
    new Intl.DateTimeFormat("en", {
      timeZone: dashboardTimezone,
      timeZoneName: "short",
    })
      .formatToParts(currentTime)
      .find((part) => part.type === "timeZoneName")?.value ?? dashboardTimezone,
  );
  let dashboardCalendarMonthLabel = $derived(
    new Intl.DateTimeFormat("en", {
      timeZone: dashboardTimezone,
      month: "long",
      year: "numeric",
    }).format(currentTime),
  );
  let dashboardCalendarDays = $derived(
    buildDashboardCalendarMonth(dashboardCalendarDate),
  );
  let focusTimeLabel = $derived(formatFocusTime(focusRemainingSeconds));
  let focusProgress = $derived(
    focusDurationMinutes === 0
      ? 0
      : Math.max(
          0,
          Math.min(
            100,
            ((focusDurationMinutes * 60 - focusRemainingSeconds) /
              (focusDurationMinutes * 60)) *
              100,
          ),
        ),
  );
  let focusSessionStatus = $derived(
    focusRemainingSeconds <= 0
      ? "COMPLETE"
      : focusRunning
        ? "IN FOCUS"
        : "PAUSED",
  );

  onMount(() => {
    sidebarCollapsed =
      localStorage.getItem("pandan-sidebar-collapsed") === "true";
    const savedPage = localStorage.getItem("pandan-active-section");
    const savedBuiltin = savedPage?.startsWith("builtin:")
      ? savedPage.slice("builtin:".length)
      : savedPage;
    const savedEmbedded = savedPage?.startsWith("embedded:")
      ? savedPage.slice("embedded:".length)
      : null;
    if (productPages.some((item) => item.id === savedBuiltin)) {
      activePage = { kind: "builtin", id: savedBuiltin as ProductPage };
      kanbanMenuOpen = savedBuiltin === "kanban";
    } else if (
      savedEmbedded &&
      [
        ...(dashboard?.embedded_pages.global ?? []),
        ...(dashboard?.embedded_pages.personal ?? []),
      ].some((page) => page.id === savedEmbedded)
    ) {
      activePage = { kind: "embedded", id: savedEmbedded };
    }
    const savedEngine = localStorage.getItem(searchEngineStorageKey);
    if (savedEngine && searchEngines.some((item) => item.id === savedEngine)) {
      searchEngine = savedEngine as SearchEngineId;
    }
    clockTimer = setInterval(() => (currentTime = new Date()), 1_000);
    const savedFocusDuration = Number(
      localStorage.getItem("pandan-focus-duration"),
    );
    if (
      Number.isInteger(savedFocusDuration) &&
      savedFocusDuration >= 1 &&
      savedFocusDuration <= 240
    ) {
      focusDurationMinutes = savedFocusDuration;
      focusRemainingSeconds = savedFocusDuration * 60;
    }
    const currentUrl = new URL(window.location.href);
    const oidcError = currentUrl.searchParams.get("auth_error");
    if (oidcError) {
      authError =
        oidcError === "oidc_access_denied"
          ? "Single sign-on was cancelled."
          : oidcError === "oidc_registration_disabled"
            ? "New accounts cannot be created with single sign-on."
            : "Single sign-on could not be completed. Please try again.";
      currentUrl.searchParams.delete("auth_error");
      window.history.replaceState(
        {},
        "",
        `${currentUrl.pathname}${currentUrl.search}${currentUrl.hash}`,
      );
    }
    if (dashboard) {
      refreshPrivateWallpaperRevisions();
      resetAppearanceDraft();
      void showInitialLoadingScreen();
    } else {
      initialLoadingPending = false;
    }
  });

  onDestroy(() => {
    clearAvatarDraft();
    clearWallpaperDrafts();
    clearTimeout(toastTimer);
    clearTimeout(welcomeRemoveTimer);
    clearTimeout(loadingAnimationMinimumTimer);
    clearInterval(clockTimer);
    clearInterval(focusTimer);
    clearTimeout(focusExitTimer);
    taskViewSwap.cancel();
  });

  function clockDisplay(date: Date, timezone: string) {
    let formatter: Intl.DateTimeFormat;
    try {
      formatter = new Intl.DateTimeFormat("en", {
        timeZone: timezone,
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
        hour12: false,
      });
    } catch {
      return clockDisplay(date, "UTC");
    }
    const parts = formatter.formatToParts(date);
    const value = (type: Intl.DateTimeFormatPartTypes) =>
      Number(parts.find((part) => part.type === type)?.value ?? 0);
    const hour = value("hour") % 12;
    const minute = value("minute");
    const second = value("second");
    return {
      hourAngle: hour * 30 + minute * 0.5,
      minuteAngle: minute * 6 + second * 0.1,
      secondAngle: second * 6,
      label: new Intl.DateTimeFormat("en", {
        timeZone: timezone,
        hour: "2-digit",
        minute: "2-digit",
        hour12: false,
      }).format(date),
      zone: timezone.split("/").at(-1)?.replaceAll("_", " ") ?? timezone,
    };
  }

  function normalizeTimezone(timezone: string) {
    try {
      new Intl.DateTimeFormat("en", { timeZone: timezone }).format();
      return timezone;
    } catch {
      return "UTC";
    }
  }

  function dateInTimezone(date: Date, timezone: string) {
    const parts = new Intl.DateTimeFormat("en", {
      timeZone: timezone,
      year: "numeric",
      month: "numeric",
      day: "numeric",
    }).formatToParts(date);
    const value = (type: Intl.DateTimeFormatPartTypes) =>
      Number(parts.find((part) => part.type === type)?.value ?? 0);
    return new Date(value("year"), value("month") - 1, value("day"));
  }

  function dashboardCalendarDateKey(date: Date) {
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  function buildDashboardCalendarMonth(
    reference: Date,
  ): DashboardCalendarDay[] {
    const first = new Date(reference.getFullYear(), reference.getMonth(), 1);
    const mondayOffset = (first.getDay() + 6) % 7;
    const start = new Date(
      reference.getFullYear(),
      reference.getMonth(),
      1 - mondayOffset,
    );
    const today = dashboardCalendarDateKey(reference);

    return Array.from({ length: 42 }, (_, index) => {
      const date = new Date(
        start.getFullYear(),
        start.getMonth(),
        start.getDate() + index,
      );
      const key = dashboardCalendarDateKey(date);
      return {
        key,
        day: date.getDate(),
        currentMonth: date.getMonth() === reference.getMonth(),
        today: key === today,
      };
    });
  }

  function taskDayDistance(dueDate: string, reference: Date, timezone: string) {
    const [year, month, day] = dueDate.split("-").map(Number);
    const dueDay = Date.UTC(year, month - 1, day);
    let referenceParts: Intl.DateTimeFormatPart[];
    try {
      referenceParts = new Intl.DateTimeFormat("en", {
        timeZone: timezone,
        year: "numeric",
        month: "numeric",
        day: "numeric",
      }).formatToParts(reference);
    } catch {
      return taskDayDistance(dueDate, reference, "UTC");
    }
    const referenceValue = (type: Intl.DateTimeFormatPartTypes) =>
      Number(referenceParts.find((part) => part.type === type)?.value ?? 0);
    const referenceDay = Date.UTC(
      referenceValue("year"),
      referenceValue("month") - 1,
      referenceValue("day"),
    );
    return Math.round((dueDay - referenceDay) / 86_400_000);
  }

  function groupTasksByDueDate(
    items: Task[],
    reference: Date,
    timezone: string,
  ): TaskDueGroup[] {
    const groups: TaskDueGroup[] = [
      {
        id: "today",
        label: "Due today",
        range: "Today and earlier",
        tasks: [],
      },
      {
        id: "this-week",
        label: "Due in less than a week",
        range: "Tomorrow through day 6",
        tasks: [],
      },
      {
        id: "next-week",
        label: "Due next week",
        range: "7–13 days away",
        tasks: [],
      },
      {
        id: "later",
        label: "Due later",
        range: "14 or more days away",
        tasks: [],
      },
      {
        id: "never",
        label: "Never due",
        range: "No due date",
        tasks: [],
      },
    ];

    for (const task of items) {
      if (!task.due_date) {
        groups[4].tasks.push(task);
        continue;
      }
      const daysAway = taskDayDistance(task.due_date, reference, timezone);
      const groupIndex =
        daysAway <= 0 ? 0 : daysAway < 7 ? 1 : daysAway < 14 ? 2 : 3;
      groups[groupIndex].tasks.push(task);
    }

    return groups;
  }

  function formatFocusTime(totalSeconds: number) {
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
  }

  function stopFocusTimer() {
    focusRunning = false;
    clearInterval(focusTimer);
    focusTimer = undefined;
  }

  function setFocusDuration(minutes: number) {
    if (focusRunning) return;
    const duration = Math.min(240, Math.max(1, Math.round(minutes || 1)));
    focusDurationMinutes = duration;
    focusRemainingSeconds = duration * 60;
    localStorage.setItem("pandan-focus-duration", String(duration));
  }

  function toggleFocusTimer() {
    if (focusRunning) {
      stopFocusTimer();
      return;
    }
    if (focusRemainingSeconds <= 0) {
      focusRemainingSeconds = focusDurationMinutes * 60;
    }
    focusRunning = true;
    focusTimer = setInterval(() => {
      if (focusRemainingSeconds <= 1) {
        focusRemainingSeconds = 0;
        stopFocusTimer();
        showToast(
          focusSubject.trim()
            ? `Focus complete: ${focusSubject.trim()}`
            : "Focus session complete",
        );
        return;
      }
      focusRemainingSeconds -= 1;
    }, 1_000);
  }

  function resetFocusTimer() {
    stopFocusTimer();
    focusRemainingSeconds = focusDurationMinutes * 60;
  }

  function resetBurstControls() {
    burstIntensity = 1.7;
    burstSpeed = 0.34;
    burstDistort = 0.35;
    burstHoverDampness = 0.2;
    burstRayCount = 18;
    burstPaused = false;
  }

  function captureFocusDialog(node: HTMLDialogElement) {
    focusDialog = node;
    return () => {
      focusDialog = undefined;
    };
  }

  function startFocusSession() {
    focusSubject = focusSubject.trim() || "Deep work";
    focusRemainingSeconds = focusDurationMinutes * 60;
    focusLeaving = false;
    focusSettingsOpen = false;
    if (!focusDialog?.open) focusDialog?.showModal();
    toggleFocusTimer();
  }

  function startDashboardFocusSession(subject: string, minutes: number) {
    focusSubject = subject;
    setFocusDuration(minutes);
    startFocusSession();
  }

  function endFocusSession() {
    const sessionWasOpen = focusDialog?.open ?? false;
    if (!sessionWasOpen || focusLeaving) return;

    stopFocusTimer();
    const closeSession = () => {
      clearTimeout(focusExitTimer);
      focusExitTimer = undefined;
      focusDialog?.close();
      focusLeaving = false;
      focusSettingsOpen = false;
      focusRemainingSeconds = focusDurationMinutes * 60;
      showToast("Focus session ended");
    };

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      closeSession();
      return;
    }

    focusLeaving = true;
    focusExitTimer = setTimeout(closeSession, 420);
  }

  async function showInitialLoadingScreen() {
    authenticating = true;
    loadingScreenReady = true;
    setLoadingWelcomeName();
    welcomeLeaving = false;
    try {
      await tick();
      await Promise.all([prepareWelcomeWallpaper(), waitForLoadingScreen()]);
      await finishLoadingScreen();
    } finally {
      initialLoadingPending = false;
      loadingScreenReady = false;
      authenticating = false;
      welcomeLeaving = false;
    }
  }

  function setLoadingWelcomeName() {
    loadingWelcomeName =
      authDisplayName.trim().split(/\s+/)[0] ||
      dashboard?.settings.display_name.trim().split(/\s+/)[0] ||
      authEmail.split("@")[0] ||
      "there";
  }

  function waitForLoadingScreen() {
    clearTimeout(loadingAnimationMinimumTimer);
    return new Promise<void>((resolve) => {
      loadingAnimationMinimumTimer = setTimeout(resolve, 2_700);
    });
  }

  function finishLoadingScreen() {
    clearTimeout(welcomeRemoveTimer);
    welcomeLeaving = true;
    return new Promise<void>((resolve) => {
      welcomeRemoveTimer = setTimeout(resolve, 600);
    });
  }

  function preloadImage(source: string) {
    return new Promise<boolean>((resolve) => {
      const image = new Image();
      let settled = false;
      const finish = (loaded: boolean) => {
        if (settled) return;
        settled = true;
        image.onload = null;
        image.onerror = null;
        resolve(loaded);
      };
      image.decoding = "async";
      image.onload = () => {
        void image
          .decode()
          .catch(() => undefined)
          .finally(() => finish(true));
      };
      image.onerror = () => finish(false);
      image.src = source;
    });
  }

  async function prepareWelcomeWallpaper() {
    const fallback = "/wired-terminal-wallpaper.png";
    const source = wallpaperSource("welcome");
    const loaded = await preloadImage(source);
    if (!loaded && source !== fallback) {
      await preloadImage(fallback);
    }
  }

  function refreshPrivateWallpaperRevisions() {
    const revision = Date.now();
    wallpaperRevisions.dashboard = revision;
    wallpaperRevisions.welcome = revision;
    wallpaperRevisions.loading = revision;
  }

  function openWallsFromSettings() {
    settingsDialog?.close();
    openProductPage("walls");
  }

  async function handleWallApplied(slot: WallSlot) {
    // Applying a wall does not change the wallpaper URL, so without a fresh revision the
    // browser keeps serving the previous image and the background appears not to change.
    wallpaperRevisions[slot] = Date.now();
    if (slot === "login") {
      if (dashboard) {
        dashboard = {
          ...dashboard,
          appearance: { ...dashboard.appearance, has_login_wallpaper: true },
        };
      }
      showToast("Login screen updated");
      return;
    }
    if (dashboard) {
      dashboard = {
        ...dashboard,
        appearance: { ...dashboard.appearance, has_welcome_wallpaper: true },
      };
    }
    await prepareWelcomeWallpaper();
    showToast("Background updated");
  }

  $effect(() => {
    if (!podcastAudio) return;
    podcastPlayer.attach(podcastAudio);
    podcastPlayer.setPlaybackRate(dashboard?.settings.podcast_playback_rate ?? 1);
  });

  // The volume popover is a transient surface, so it closes on Escape or on a pointer
  // press anywhere outside it. The listeners only exist while it is open.
  $effect(() => {
    if (!podcastVolumeOpen) return;
    const dismiss = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && podcastVolumeControl?.contains(target)) return;
      podcastVolumeOpen = false;
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      podcastVolumeOpen = false;
    };
    globalThis.addEventListener("pointerdown", dismiss);
    globalThis.addEventListener("keydown", escape, true);
    return () => {
      globalThis.removeEventListener("pointerdown", dismiss);
      globalThis.removeEventListener("keydown", escape, true);
    };
  });

  // Closing the player must not leave its popover orphaned on screen.
  $effect(() => {
    if (!podcastPlayer.episode) podcastVolumeOpen = false;
  });

  // A position is written on an interval while playing; this catches the tail end
  // when the tab is closed or backgrounded between writes.
  $effect(() => {
    const flush = () => void podcastPlayer.flushNow();
    globalThis.addEventListener("pagehide", flush);
    document.addEventListener("visibilitychange", flush);
    return () => {
      globalThis.removeEventListener("pagehide", flush);
      document.removeEventListener("visibilitychange", flush);
    };
  });

  /** Applies a playback speed immediately and remembers it for the next session. */
  async function savePlaybackRate(rate: number) {
    podcastPlayer.setPlaybackRate(rate);
    if (!dashboard) return;
    try {
      const settings = await updateUserSettings({
        display_name: dashboard.settings.display_name,
        location: dashboard.settings.location,
        timezone: dashboard.settings.timezone,
        temperature_unit: dashboard.settings.temperature_unit,
        lines_default_visibility: dashboard.settings.lines_default_visibility,
        podcast_playback_rate: rate,
      });
      dashboard = { ...dashboard, settings };
    } catch {
      // Speed already applied locally; remembering it is a convenience, not a
      // reason to interrupt playback.
    }
  }

  function openProductPage(page: ProductPage) {
    // Lines keeps its own screen stack, so choosing it in the sidebar has to ask the
    // page for the timeline even when it is already the active section.
    if (page === "lines") linesHomeToken += 1;
    activePage = { kind: "builtin", id: page };
    if (page !== "contacts") contactDetailId = null;
    if (page !== "calendar") calendarDetailDate = null;
    sidebarOpen = false;
    pendingTaskDeleteId = "";
    localStorage.setItem("pandan-active-section", `builtin:${page}`);
  }

  function openEmbeddedPage(pageId: string) {
    activePage = { kind: "embedded", id: pageId };
    contactDetailId = null;
    calendarDetailDate = null;
    sidebarOpen = false;
    pendingTaskDeleteId = "";
    localStorage.setItem("pandan-active-section", `embedded:${pageId}`);
  }

  function openKanbanSection(nextSection: KanbanSection) {
    kanbanSection = nextSection;
    kanbanMenuOpen = true;
    openProductPage("kanban");
  }

  function openCalendarTask(task: Task) {
    openProductPage("tasks");
    openTaskEditor(task);
  }

  function openCalendarContact(contactId: string) {
    contactDetailId = contactId;
    openProductPage("contacts");
  }

  function openDashboardCalendarDate(date: string) {
    calendarDetailDate = date;
    openProductPage("calendar");
  }

  function toggleSidebar() {
    if (mobileNavigation.current) {
      sidebarOpen = !sidebarOpen;
      return;
    }
    sidebarOpen = false;
    sidebarCollapsed = !sidebarCollapsed;
    localStorage.setItem("pandan-sidebar-collapsed", String(sidebarCollapsed));
  }

  function captureSidebar(node: HTMLElement) {
    sidebarElement = node;
    return () => {
      sidebarElement = undefined;
      sidebarHint = null;
    };
  }

  function sidebarHintTarget(event: Event) {
    if (!(event.target instanceof Element)) return null;
    return event.target.closest<HTMLElement>("[data-sidebar-title]");
  }

  function showSidebarHint(event: Event) {
    const source = sidebarHintTarget(event);
    const title = source?.dataset.sidebarTitle;
    const description = source?.dataset.sidebarDescription;
    if (!source || !sidebarElement || !title || !description) return;

    const sidebarBounds = sidebarElement.getBoundingClientRect();
    const sourceBounds = source.getBoundingClientRect();
    const sourceCenter =
      sourceBounds.top - sidebarBounds.top + sourceBounds.height / 2;
    const edgeInset = 58;
    sidebarHint = {
      title,
      description,
      top: Math.max(
        edgeInset,
        Math.min(sidebarBounds.height - edgeInset, sourceCenter),
      ),
      source,
    };
  }

  function hideSidebarHint(event: PointerEvent | FocusEvent) {
    const source = sidebarHintTarget(event);
    const nextTarget = event.relatedTarget;
    if (source && nextTarget instanceof Node && source.contains(nextTarget)) {
      return;
    }
    if (sidebarHint?.source === source) sidebarHint = null;
  }

  function showToast(message: string) {
    toastMessage = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toastMessage = ""), 1800);
  }

  async function openCommand() {
    commandQuery = "";
    commandIndex = 0;
    commandDialog?.showModal();
    await tick();
    commandSearchInput?.focus();
  }

  function selectSearchEngine(event: Event) {
    searchEngine = (event.currentTarget as HTMLSelectElement)
      .value as SearchEngineId;
    localStorage.setItem(searchEngineStorageKey, searchEngine);
    commandSearchInput?.focus();
  }

  function captureCommandDialog(node: HTMLDialogElement) {
    commandDialog = node;
    return () => {
      commandDialog = undefined;
    };
  }

  function captureCommandSearchInput(node: HTMLInputElement) {
    commandSearchInput = node;
    return () => {
      commandSearchInput = undefined;
    };
  }

  function handleCommandSearchKeydown(event: KeyboardEvent) {
    if (commandResults.length === 0) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      commandIndex = (commandIndex + 1) % commandResults.length;
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      commandIndex =
        (commandIndex - 1 + commandResults.length) % commandResults.length;
      return;
    }
    if (event.key !== "Enter") return;
    event.preventDefault();
    runCommand(commandResults[commandIndex] ?? commandResults[0]);
  }

  function captureSettingsDialog(node: HTMLDialogElement) {
    settingsDialog = node;
    return () => {
      settingsDialog = undefined;
    };
  }

  function captureSettingsScrollContainer(node: HTMLDivElement) {
    settingsScrollContainer = node;
    return () => {
      settingsScrollContainer = undefined;
    };
  }

  function captureDestructiveDialog(node: HTMLDialogElement) {
    destructiveDialog = node;
    return () => {
      destructiveDialog = undefined;
    };
  }

  function captureAdminDialog(node: HTMLDialogElement) {
    adminDialog = node;
    return () => {
      adminDialog = undefined;
    };
  }

  function captureWidgetLibraryDialog(node: HTMLDialogElement) {
    widgetLibraryDialog = node;
    return () => {
      widgetLibraryDialog = undefined;
    };
  }

  function captureAppearanceDialog(node: HTMLDialogElement) {
    appearanceDialog = node;
    return () => {
      appearanceDialog = undefined;
    };
  }

  function captureTaskEditorDialog(node: HTMLDialogElement) {
    taskEditorDialog = node;
    return () => {
      taskEditorDialog = undefined;
    };
  }

  function resetTaskEditor() {
    editingTaskId = null;
    taskName = "";
    taskDescription = "";
    taskPriority = "none";
    taskDueDate = "";
    taskLabels = "";
    taskRepeatRule = "none";
    taskRepeatInterval = 1;
    taskRepeatUnit = "days";
    taskRescheduleFrom = "due_date";
    taskSubtasks = [];
    taskAttachments = [];
    pendingTaskFiles = [];
    taskEditorError = "";
  }

  function openTaskEditor(task?: Task) {
    resetTaskEditor();
    pendingTaskDeleteId = "";
    if (task) {
      editingTaskId = task.id;
      taskName = task.title;
      taskDescription = task.description;
      taskPriority = task.priority;
      taskDueDate = task.due_date ?? "";
      taskLabels = task.labels.join(", ");
      taskRepeatRule = task.repeat_rule;
      taskRepeatInterval = task.repeat_interval;
      taskRepeatUnit = task.repeat_unit;
      taskRescheduleFrom = task.reschedule_from;
      taskSubtasks = task.subtasks.map((subtask) => ({
        id: subtask.id,
        title: subtask.title,
        completed: subtask.completed,
      }));
      taskAttachments = task.attachments;
    }
    taskEditorDialog?.showModal();
  }

  function addSubtaskDraft() {
    taskSubtasks = [...taskSubtasks, { title: "", completed: false }];
  }

  function removeSubtaskDraft(index: number) {
    taskSubtasks = taskSubtasks.filter((_, itemIndex) => itemIndex !== index);
  }

  function selectTaskAttachments(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    pendingTaskFiles = Array.from(input.files ?? []);
  }

  async function saveTask(event: SubmitEvent) {
    event.preventDefault();
    if (savingTask) return;
    savingTask = true;
    taskEditorError = "";

    const input: TaskInput = {
      title: taskName,
      description: taskDescription,
      priority: taskPriority,
      due_date: taskDueDate || null,
      labels: taskLabels
        .split(",")
        .map((label) => label.trim())
        .filter(Boolean),
      subtasks: taskSubtasks
        .map((subtask) => ({ ...subtask, title: subtask.title.trim() }))
        .filter((subtask) => subtask.title),
      repeat_rule: taskRepeatRule,
      repeat_interval: taskRepeatInterval,
      repeat_unit: taskRepeatUnit,
      reschedule_from: taskRescheduleFrom,
    };

    try {
      let saved = editingTaskId
        ? await updateTask(editingTaskId, input)
        : await createTask(input);
      const uploaded = [];
      for (const file of pendingTaskFiles) {
        uploaded.push(await uploadTaskAttachment(saved.id, file));
      }
      if (uploaded.length) {
        saved = {
          ...saved,
          attachments: [...saved.attachments, ...uploaded],
        };
      }
      tasks = editingTaskId
        ? tasks.map((task) => (task.id === saved.id ? saved : task))
        : [...tasks, saved];
      taskEditorDialog?.close();
      showToast(editingTaskId ? "Task updated" : "Task created");
    } catch (reason: unknown) {
      taskEditorError =
        reason instanceof Error ? reason.message : "Unable to save task";
    } finally {
      savingTask = false;
    }
  }

  async function removeCurrentTask() {
    if (!editingTaskId || savingTask) return;
    if (!window.confirm("Delete this task and its attachments?")) return;
    savingTask = true;
    taskEditorError = "";
    try {
      await deleteTask(editingTaskId);
      tasks = tasks.filter((task) => task.id !== editingTaskId);
      taskEditorDialog?.close();
      showToast("Task deleted");
    } catch (reason: unknown) {
      taskEditorError =
        reason instanceof Error ? reason.message : "Unable to delete task";
    } finally {
      savingTask = false;
    }
  }

  function resetTaskArchiveView() {
    taskViewSwap.cancel();
    taskView = "active";
    taskViewTarget = "active";
    taskLabelFilter = "";
    archivedTasks = [];
    archivedTasksLoaded = false;
    loadingArchivedTasks = false;
    archivedTasksError = "";
    pendingTaskDeleteId = "";
  }

  async function loadArchivedTasks() {
    if (loadingArchivedTasks) return;
    loadingArchivedTasks = true;
    archivedTasksError = "";
    try {
      archivedTasks = await fetchArchivedTasks();
      archivedTasksLoaded = true;
    } catch (reason: unknown) {
      archivedTasksError =
        reason instanceof Error
          ? reason.message
          : "Unable to load archived tasks";
    } finally {
      loadingArchivedTasks = false;
    }
  }

  async function selectTaskView(view: TaskView) {
    if (view === taskViewTarget) return;
    taskViewTarget = view;
    await taskViewSwap.run({
      forward: view === "archived",
      pending:
        view === "archived" && !archivedTasksLoaded ? loadArchivedTasks() : null,
      commit: () => {
        taskView = view;
        taskLabelFilter = "";
        pendingTaskDeleteId = "";
      },
    });
  }

  async function archiveTaskFromList(task: Task) {
    if (taskActionId) return;
    const previousTasks = tasks;
    taskActionId = task.id;
    pendingTaskDeleteId = "";
    tasks = tasks.filter((item) => item.id !== task.id);
    try {
      await archiveTask(task.id);
      expandedTaskIds.delete(task.id);
      if (archivedTasksLoaded) {
        archivedTasks = [
          task,
          ...archivedTasks.filter((item) => item.id !== task.id),
        ];
      }
      showToast(`Archived ${task.title}`);
    } catch (reason: unknown) {
      tasks = previousTasks;
      showToast(
        reason instanceof Error ? reason.message : "Unable to archive task",
      );
    } finally {
      taskActionId = "";
    }
  }

  async function restoreTaskFromArchive(task: Task) {
    if (taskActionId) return;
    const previousArchivedTasks = archivedTasks;
    taskActionId = task.id;
    pendingTaskDeleteId = "";
    archivedTasks = archivedTasks.filter((item) => item.id !== task.id);
    try {
      await restoreTask(task.id);
      tasks = [...tasks.filter((item) => item.id !== task.id), task];
      expandedTaskIds.delete(task.id);
      showToast(`Restored ${task.title}`);
    } catch (reason: unknown) {
      archivedTasks = previousArchivedTasks;
      showToast(
        reason instanceof Error ? reason.message : "Unable to restore task",
      );
    } finally {
      taskActionId = "";
    }
  }

  async function deleteArchivedTaskFromList(task: Task) {
    if (taskActionId) return;
    if (pendingTaskDeleteId !== task.id) {
      pendingTaskDeleteId = task.id;
      showToast(`Select delete again to remove ${task.title}`);
      return;
    }

    const previousArchivedTasks = archivedTasks;
    taskActionId = task.id;
    archivedTasks = archivedTasks.filter((item) => item.id !== task.id);
    try {
      await deleteTask(task.id);
      expandedTaskIds.delete(task.id);
      showToast(`Deleted ${task.title}`);
    } catch (reason: unknown) {
      archivedTasks = previousArchivedTasks;
      showToast(
        reason instanceof Error ? reason.message : "Unable to delete task",
      );
    } finally {
      taskActionId = "";
      pendingTaskDeleteId = "";
    }
  }

  async function deleteTaskFromList(task: Task) {
    if (taskActionId) return;
    if (pendingTaskDeleteId !== task.id) {
      pendingTaskDeleteId = task.id;
      showToast(`Select delete again to remove ${task.title}`);
      return;
    }

    const previousTasks = tasks;
    taskActionId = task.id;
    tasks = tasks.filter((item) => item.id !== task.id);
    try {
      await deleteTask(task.id);
      showToast(`Deleted ${task.title}`);
    } catch (reason: unknown) {
      tasks = previousTasks;
      showToast(
        reason instanceof Error ? reason.message : "Unable to delete task",
      );
    } finally {
      taskActionId = "";
      pendingTaskDeleteId = "";
    }
  }

  async function removeTaskAttachment(attachment: TaskAttachment) {
    if (!editingTaskId || savingTask) return;
    taskEditorError = "";
    try {
      await deleteTaskAttachment(editingTaskId, attachment.id);
      taskAttachments = taskAttachments.filter(
        (item) => item.id !== attachment.id,
      );
      tasks = tasks.map((task) =>
        task.id === editingTaskId
          ? {
              ...task,
              attachments: task.attachments.filter(
                (item) => item.id !== attachment.id,
              ),
            }
          : task,
      );
    } catch (reason: unknown) {
      taskEditorError =
        reason instanceof Error
          ? reason.message
          : "Unable to remove attachment";
    }
  }

  function downloadTaskAttachment(attachment: TaskAttachment) {
    if (!editingTaskId) return;
    const link = document.createElement("a");
    link.href = taskAttachmentUrl(editingTaskId, attachment.id);
    link.download = attachment.file_name;
    link.click();
  }

  function openWidgetLibrary() {
    widgetLibraryDialog?.showModal();
  }

  async function toggleLayoutEditing() {
    if (draggedWidgetId || savingLayout) return;
    layoutEditing = !layoutEditing;
    if (layoutEditing) await tick();
    for (const grid of gridInstances.values()) {
      grid.enableMove(layoutEditing);
      grid.enableResize(layoutEditing);
    }
    showToast(layoutEditing ? "Layout editing enabled" : "Layout saved");
  }

  function dashboardWidgets() {
    return [...widgets].sort(
      (a, b) =>
        a.grid_y - b.grid_y || a.grid_x - b.grid_x || a.position - b.position,
    );
  }

  function normalizeWidgetLayout(items: DashboardWidget[]) {
    return items
      .sort(
        (a, b) =>
          a.grid_y - b.grid_y || a.grid_x - b.grid_x || a.position - b.position,
      )
      .map((widget, position) => ({ ...widget, position }));
  }

  function widgetSizeForWidth(width: number): WidgetSize {
    if (width <= 4) return "compact";
    if (width <= 6) return "standard";
    if (width <= 9) return "wide";
    return "full";
  }

  function gridAttributes(widget: DashboardWidget) {
    return {
      "gs-id": widget.id,
      "gs-x": widget.grid_x,
      "gs-y": widget.grid_y,
      "gs-w": widget.grid_w,
      "gs-h": widget.grid_h,
    };
  }

  function gridCoordinates(grid: GridStack, element: GridItemHTMLElement) {
    const node = element.gridstackNode;
    if (!node) return null;
    const columns = grid.getColumn();
    const ratio = 12 / columns;
    const grid_w = Math.max(1, Math.min(12, Math.round((node.w ?? 1) * ratio)));
    const grid_x = Math.max(
      0,
      Math.min(12 - grid_w, Math.round((node.x ?? 0) * ratio)),
    );
    return {
      grid_x,
      grid_y: Math.max(0, node.y ?? 0),
      grid_w,
      grid_h: Math.max(1, node.h ?? 1),
    };
  }

  function layoutFromGrid() {
    const next = widgets.map((widget) => ({ ...widget }));
    for (const grid of gridInstances.values()) {
      for (const element of grid.getGridItems()) {
        const id = element.dataset.widgetId;
        const coordinates = gridCoordinates(grid, element);
        const index = next.findIndex((widget) => widget.id === id);
        if (index < 0 || !coordinates) continue;
        next[index] = {
          ...next[index],
          ...coordinates,
          size: widgetSizeForWidth(coordinates.grid_w),
        };
      }
    }
    return normalizeWidgetLayout(next);
  }

  async function synchronizeGridFromState() {
    await tick();
    for (const grid of gridInstances.values()) {
      const gridElement = grid.el;
      for (const widget of dashboardWidgets()) {
        const element = gridElement.querySelector<GridItemHTMLElement>(
          `[data-widget-id="${widget.id}"]`,
        );
        if (!element) continue;
        if (!element.gridstackNode) grid.makeWidget(element);
        grid.update(element, {
          x: widget.grid_x,
          y: widget.grid_y,
          w: widget.grid_w,
          h: widget.grid_h,
        });
      }
      grid.enableMove(layoutEditing);
      grid.enableResize(layoutEditing);
    }
  }

  async function persistWidgetLayout(
    next: DashboardWidget[],
    previous: DashboardWidget[],
  ) {
    if (savingLayout) return;
    widgets = next;
    savingLayout = true;
    try {
      widgets = await updateDashboardWidgetLayout(
        next.map(
          ({
            id,
            workspace,
            position,
            size,
            grid_x,
            grid_y,
            grid_w,
            grid_h,
          }) => ({
            id,
            workspace,
            position,
            size,
            grid_x,
            grid_y,
            grid_w,
            grid_h,
          }),
        ),
      );
    } catch (reason: unknown) {
      widgets = previous;
      await synchronizeGridFromState();
      showToast(
        reason instanceof Error
          ? reason.message
          : "Widget layout was not saved",
      );
    } finally {
      savingLayout = false;
    }
  }

  function layoutsMatch(first: DashboardWidget[], second: DashboardWidget[]) {
    return first.every((widget) => {
      const candidate = second.find((item) => item.id === widget.id);
      return (
        candidate?.workspace === widget.workspace &&
        candidate.position === widget.position &&
        candidate.size === widget.size &&
        candidate.grid_x === widget.grid_x &&
        candidate.grid_y === widget.grid_y &&
        candidate.grid_w === widget.grid_w &&
        candidate.grid_h === widget.grid_h
      );
    });
  }

  function startGridDrag(element: GridItemHTMLElement) {
    if (!layoutEditing || savingLayout) return;
    draggedWidgetId = element.dataset.widgetId ?? "";
    dragSnapshot = widgets.map((item) => ({ ...item }));
  }

  function finishGridInteraction() {
    if (!draggedWidgetId) return;

    const previous = dragSnapshot.map((item) => ({ ...item }));
    const next = layoutFromGrid();
    widgets = next;
    if (!layoutsMatch(next, previous)) void persistWidgetLayout(next, previous);

    draggedWidgetId = "";
    dragSnapshot = [];
  }

  function finishGridResize() {
    const previous = widgets.map((item) => ({ ...item }));
    const next = layoutFromGrid();
    widgets = next;
    if (!layoutsMatch(next, previous)) void persistWidgetLayout(next, previous);
  }

  function gridAttachment(workspace: number) {
    return (node: HTMLElement) => {
      const grid = GridStack.init(
        {
          column: 12,
          columnOpts: {
            breakpoints: [
              { w: 720, c: 1, layout: "list" },
              { w: 1040, c: 6, layout: "moveScale" },
            ],
            layout: "moveScale",
          },
          cellHeight: 76,
          margin: 8,
          animate: true,
          float: false,
          disableDrag: true,
          disableResize: true,
          handle: ".widget-drag-handle",
          draggable: {
            handle: ".widget-drag-handle",
            appendTo: "body",
            helper: "clone",
            scroll: true,
          },
          resizable: { handles: "e,se,s,sw,w" },
        },
        node,
      );
      if (!grid)
        throw new Error("GridStack could not initialize this workspace");
      gridInstances.set(workspace, grid);
      grid.on("dragstart", (_event, element) => startGridDrag(element));
      grid.on("dragstop", finishGridInteraction);
      grid.on("resizestop", finishGridResize);
      return () => {
        grid.offAll();
        grid.destroy(false);
        gridInstances.delete(workspace);
      };
    };
  }

  async function addWidget(kind: WidgetKind, size: WidgetSize) {
    if (addingWidgetKind || savingLayout) return;
    addingWidgetKind = kind;
    try {
      const widget = await createDashboardWidget({
        kind,
        workspace: 0,
        size,
      });
      widgets = normalizeWidgetLayout([...widgets, widget]);
      await synchronizeGridFromState();
      widgetLibraryDialog?.close();
      showToast(
        `${widgetCatalog.find((item) => item.kind === kind)?.title ?? "Widget"} added`,
      );
    } catch (reason: unknown) {
      showToast(
        reason instanceof Error ? reason.message : "Unable to add widget",
      );
    } finally {
      addingWidgetKind = "";
    }
  }

  async function removeWidget(widget: DashboardWidget) {
    if (savingLayout) return;
    const previous = widgets.map((item) => ({ ...item }));
    const grid = gridInstances.get(0);
    const element = grid?.el.querySelector<GridItemHTMLElement>(
      `[data-widget-id="${widget.id}"]`,
    );
    if (grid && element) grid.removeWidget(element, false, false);
    widgets = normalizeWidgetLayout(
      widgets.filter((item) => item.id !== widget.id),
    );
    try {
      await deleteDashboardWidget(widget.id);
      showToast("Widget removed");
    } catch (reason: unknown) {
      widgets = previous;
      await synchronizeGridFromState();
      showToast(
        reason instanceof Error ? reason.message : "Unable to remove widget",
      );
    }
  }

  function updateWidgetInstance(updated: DashboardWidget) {
    widgets = widgets.map((widget) =>
      widget.id === updated.id ? updated : widget,
    );
  }

  function setAuthMode(mode: AuthMode) {
    if (
      (mode === "login" && !authConfig.password_login_enabled) ||
      (mode === "register" && !authConfig.password_registration_enabled)
    ) {
      return;
    }
    authMode = mode;
    authError = "";
  }

  async function authenticate(event: SubmitEvent) {
    event.preventDefault();
    if (authenticating) return;

    authenticating = true;
    initialLoadingPending = false;
    loadingScreenReady = false;
    authError = "";
    try {
      if (authMode === "register") {
        await registerAccount({
          email: authEmail,
          password: authPassword,
          display_name: authDisplayName,
        });
      } else {
        await loginAccount({ email: authEmail, password: authPassword });
      }
      refreshPrivateWallpaperRevisions();
      setLoadingWelcomeName();
      welcomeLeaving = false;
      loadingScreenReady = true;
      await tick();
      const loadingScreen = Promise.all([
        prepareWelcomeWallpaper(),
        waitForLoadingScreen(),
      ]);
      const [nextDashboard] = await Promise.all([
        fetchDashboard(),
        loadingScreen,
      ]);
      dashboard = nextDashboard;
      resetTaskArchiveView();
      avatarRevision = Date.now();
      avatarAvailable = true;
      resetAppearanceDraft();
      authPassword = "";
      activePage = { kind: "builtin", id: "dashboard" };
      localStorage.setItem("pandan-active-section", "builtin:dashboard");
      await finishLoadingScreen();
    } catch (reason: unknown) {
      authError =
        reason instanceof Error ? reason.message : "Unable to continue";
    } finally {
      loadingScreenReady = false;
      authenticating = false;
      welcomeLeaving = false;
    }
  }

  async function completeSetup(event: SubmitEvent) {
    event.preventDefault();
    if (authenticating) return;

    authenticating = true;
    initialLoadingPending = false;
    loadingScreenReady = false;
    authError = "";
    try {
      await createAdministrator({
        email: authEmail,
        password: authPassword,
        display_name: authDisplayName,
      });
      refreshPrivateWallpaperRevisions();
      setLoadingWelcomeName();
      welcomeLeaving = false;
      loadingScreenReady = true;
      await tick();
      const loadingScreen = Promise.all([
        prepareWelcomeWallpaper(),
        waitForLoadingScreen(),
      ]);
      const [nextDashboard] = await Promise.all([
        fetchDashboard(),
        loadingScreen,
      ]);
      dashboard = nextDashboard;
      resetTaskArchiveView();
      avatarRevision = Date.now();
      avatarAvailable = true;
      resetAppearanceDraft();
      setupRequired = false;
      authPassword = "";
      activePage = { kind: "builtin", id: "dashboard" };
      localStorage.setItem("pandan-active-section", "builtin:dashboard");
      await finishLoadingScreen();
    } catch (reason: unknown) {
      authError =
        reason instanceof Error ? reason.message : "Unable to complete setup";
    } finally {
      loadingScreenReady = false;
      authenticating = false;
      welcomeLeaving = false;
    }
  }

  function openSettings() {
    if (!dashboard) return;
    clearUserSettingsDrafts();
    settingsDisplayName = dashboard.settings.display_name;
    settingsLocation = dashboard.settings.location;
    settingsTimezone = dashboard.settings.timezone;
    settingsTemperatureUnit = dashboard.settings.temperature_unit;
    settingsLinesDefaultVisibility =
      dashboard.settings.lines_default_visibility;
    settingsError = "";
    settingsDialog?.showModal();
    if (settingsScrollContainer) settingsScrollContainer.scrollTop = 0;
  }

  function openEmbeddedPagesSettings() {
    if (!dashboard) return;
    settingsDialog?.close();
    embeddedPagesSettingsOpen = true;
  }

  async function closeEmbeddedPagesSettings(reopenSettings = false) {
    embeddedPagesSettingsOpen = false;
    if (reopenSettings) {
      await tick();
      openSettings();
    }
  }

  function applyEmbeddedPages(pages: EmbeddedPagesResponse) {
    if (!dashboard) return;
    dashboard = { ...dashboard, embedded_pages: pages };
  }

  function handleEmbeddedPageDeleted(pageId: string) {
    if (activePage.kind === "embedded" && activePage.id === pageId) {
      openProductPage("dashboard");
    }
  }

  function applySidebarSettings(settings: UserSettings) {
    if (!dashboard) return;
    dashboard = { ...dashboard, settings };
  }

  function avatarUrl() {
    return `/api/settings/avatar?v=${avatarRevision}`;
  }

  function avatarPreviewSource() {
    if (avatarReset) return "";
    return avatarPreview || (avatarAvailable ? avatarUrl() : "");
  }

  function clearAvatarDraft() {
    if (avatarPreview.startsWith("blob:")) {
      URL.revokeObjectURL(avatarPreview);
    }
    avatarFile = null;
    avatarPreview = "";
    avatarReset = false;
  }

  function selectAvatar(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (
      !["image/jpeg", "image/png", "image/webp", "image/avif"].includes(
        file.type,
      )
    ) {
      settingsError = "Choose a JPEG, PNG, WebP, or AVIF image.";
      input.value = "";
      return;
    }
    if (file.size > 10 * 1024 * 1024) {
      settingsError = "Avatar images must be 10 MB or smaller.";
      input.value = "";
      return;
    }
    if (avatarPreview.startsWith("blob:")) {
      URL.revokeObjectURL(avatarPreview);
    }
    avatarFile = file;
    avatarPreview = URL.createObjectURL(file);
    avatarReset = false;
    settingsError = "";
  }

  function resetAvatar() {
    if (avatarPreview.startsWith("blob:")) {
      URL.revokeObjectURL(avatarPreview);
    }
    avatarFile = null;
    avatarPreview = "";
    avatarReset = true;
    settingsError = "";
  }

  function wallpaperEndpoint(slot: WallpaperSlot) {
    return slot === "login"
      ? "/api/appearance/login-wallpaper"
      : `/api/settings/wallpapers/${slot}`;
  }

  function wallpaperSource(slot: WallpaperSlot) {
    const draft = wallpaperDrafts[slot];
    if (draft.reset) return "/wired-terminal-wallpaper.png";
    return (
      draft.preview ||
      `${wallpaperEndpoint(slot)}?v=${wallpaperRevisions[slot]}`
    );
  }

  function wallpaperBackground(slot: WallpaperSlot) {
    const source = wallpaperSource(slot);
    if (source === "/wired-terminal-wallpaper.png") {
      return 'url("/wired-terminal-wallpaper.png")';
    }
    return `url("${source}"), url("/wired-terminal-wallpaper.png")`;
  }

  function wallpaperHasCustom(slot: WallpaperSlot) {
    const appearance = dashboard?.appearance;
    if (!appearance) return false;
    if (slot === "dashboard") return appearance.has_dashboard_wallpaper;
    if (slot === "welcome") return appearance.has_welcome_wallpaper;
    if (slot === "loading") return appearance.has_loading_wallpaper;
    return appearance.has_login_wallpaper;
  }

  function wallpaperFileLabel(slot: WallpaperSlot) {
    const draft = wallpaperDrafts[slot];
    if (draft.file) return draft.file.name;
    if (draft.reset) return "Wired terminal default";
    return wallpaperHasCustom(slot) ? "Custom image" : "Wired terminal default";
  }

  function clearWallpaperDraft(slot: WallpaperSlot) {
    const draft = wallpaperDrafts[slot];
    if (draft.preview.startsWith("blob:")) {
      URL.revokeObjectURL(draft.preview);
    }
    wallpaperDrafts[slot] = { file: null, preview: "", reset: false };
  }

  function clearWallpaperDrafts(slots: WallpaperSlot[] = allWallpaperSlots) {
    for (const slot of slots) clearWallpaperDraft(slot);
  }

  function clearUserSettingsWallpaperDrafts() {
    clearWallpaperDrafts(userWallpaperOptions.map((option) => option.id));
  }

  function clearUserSettingsDrafts() {
    clearAvatarDraft();
    clearUserSettingsWallpaperDrafts();
  }

  function resetAppearanceDraft() {
    clearWallpaperDrafts(appearanceWallpaperOptions.map((option) => option.id));
    const appearance = dashboard?.appearance;
    backgroundBlur = appearance?.background_blur ?? 0;
    backgroundBrightness = appearance?.background_brightness ?? 78;
    backgroundContrast = appearance?.background_contrast ?? 108;
    backgroundSaturation = appearance?.background_saturation ?? 72;
    appearanceError = "";
  }

  function selectWallpaper(slot: WallpaperSlot, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (
      !["image/jpeg", "image/png", "image/webp", "image/avif"].includes(
        file.type,
      )
    ) {
      setWallpaperError(slot, "Choose a JPEG, PNG, WebP, or AVIF image.");
      input.value = "";
      return;
    }
    if (file.size > 30 * 1024 * 1024) {
      setWallpaperError(slot, "Wallpaper images must be 30 MB or smaller.");
      input.value = "";
      return;
    }
    const draft = wallpaperDrafts[slot];
    if (draft.preview.startsWith("blob:")) {
      URL.revokeObjectURL(draft.preview);
    }
    wallpaperDrafts[slot] = {
      file,
      preview: URL.createObjectURL(file),
      reset: false,
    };
    setWallpaperError(slot, "");
  }

  function resetWallpaper(slot: WallpaperSlot) {
    const draft = wallpaperDrafts[slot];
    if (draft.preview.startsWith("blob:")) {
      URL.revokeObjectURL(draft.preview);
    }
    wallpaperDrafts[slot] = { file: null, preview: "", reset: true };
    setWallpaperError(slot, "");
  }

  function setWallpaperError(slot: WallpaperSlot, message: string) {
    if (slot === "welcome" || slot === "loading") {
      settingsError = message;
    } else {
      appearanceError = message;
    }
  }

  function resetBackgroundFilters() {
    backgroundBlur = 0;
    backgroundBrightness = 78;
    backgroundContrast = 108;
    backgroundSaturation = 72;
  }

  function openAppearance() {
    if (!dashboard) return;
    resetAppearanceDraft();
    settingsDialog?.close();
    appearanceDialog?.showModal();
  }

  function openDestructiveActions() {
    destructiveError = "";
    pendingContentDeletion = null;
    settingsDialog?.close();
    destructiveDialog?.showModal();
  }

  async function closeDestructiveActions(reopenSettings = false) {
    destructiveError = "";
    pendingContentDeletion = null;
    destructiveDialog?.close();
    if (reopenSettings) {
      await tick();
      openSettings();
    }
  }

  async function removeContentArea(action: {
    scope: UserContentScope;
    title: string;
  }) {
    if (deletingContentScope) return;
    if (pendingContentDeletion !== action.scope) {
      pendingContentDeletion = action.scope;
      destructiveError = "";
      return;
    }
    deletingContentScope = action.scope;
    destructiveError = "";
    try {
      const result = await deleteUserContent(action.scope);
      if (action.scope === "tasks" && dashboard) {
        dashboard = { ...dashboard, tasks: [] };
        resetTaskArchiveView();
      }
      if (activeSection === action.scope) {
        openProductPage("dashboard");
      }
      pendingContentDeletion = null;
      showToast(
        result.deleted === 1
          ? `Deleted 1 record from ${action.title.toLowerCase()}`
          : `Deleted ${result.deleted} records from ${action.title.toLowerCase()}`,
      );
    } catch (reason: unknown) {
      destructiveError =
        reason instanceof Error
          ? reason.message
          : `Unable to delete ${action.title.toLowerCase()}`;
    } finally {
      deletingContentScope = null;
    }
  }

  async function closeAppearance(reopenSettings = false) {
    resetAppearanceDraft();
    appearanceDialog?.close();
    if (reopenSettings) {
      await tick();
      openSettings();
    }
  }

  async function saveAppearance(event: SubmitEvent) {
    event.preventDefault();
    if (!dashboard || savingAppearance) return;
    savingAppearance = true;
    appearanceError = "";
    try {
      for (const option of appearanceWallpaperOptions) {
        if (option.adminOnly && dashboard.user.role !== "administrator") {
          continue;
        }
        const draft = wallpaperDrafts[option.id];
        if (draft.file) {
          await updateWallpaper(option.id, draft.file);
        } else if (draft.reset) {
          await deleteWallpaper(option.id);
        }
        wallpaperRevisions[option.id] = Date.now();
      }
      const appearance = await updateAppearance({
        background_blur: backgroundBlur,
        background_brightness: backgroundBrightness,
        background_contrast: backgroundContrast,
        background_saturation: backgroundSaturation,
      });
      dashboard = {
        ...dashboard,
        appearance,
      };
      clearWallpaperDrafts(
        appearanceWallpaperOptions.map((option) => option.id),
      );
      await closeAppearance(true);
      showToast("Appearance saved");
    } catch (reason: unknown) {
      appearanceError =
        reason instanceof Error ? reason.message : "Unable to save appearance";
    } finally {
      savingAppearance = false;
    }
  }

  async function saveSettings(event: SubmitEvent) {
    event.preventDefault();
    if (!dashboard || savingSettings) return;

    savingSettings = true;
    settingsError = "";
    try {
      let appearance = dashboard.appearance;
      if (avatarFile) {
        await updateAvatar(avatarFile);
        avatarRevision = Date.now();
        avatarAvailable = true;
      } else if (avatarReset) {
        await deleteAvatar();
        avatarRevision = Date.now();
        avatarAvailable = false;
      }
      for (const option of userWallpaperOptions) {
        const draft = wallpaperDrafts[option.id];
        if (draft.file) {
          await updateWallpaper(option.id, draft.file);
        } else if (draft.reset) {
          await deleteWallpaper(option.id);
        } else {
          continue;
        }
        const hasWallpaper = Boolean(draft.file);
        appearance =
          option.id === "welcome"
            ? { ...appearance, has_welcome_wallpaper: hasWallpaper }
            : { ...appearance, has_loading_wallpaper: hasWallpaper };
        wallpaperRevisions[option.id] = Date.now();
      }
      const settings = await updateUserSettings({
        display_name: settingsDisplayName,
        location: settingsLocation,
        timezone: settingsTimezone,
        temperature_unit: settingsTemperatureUnit,
        lines_default_visibility: settingsLinesDefaultVisibility,
      });
      dashboard = { ...dashboard, settings, appearance };
      clearUserSettingsDrafts();
      settingsDialog?.close();
      showToast("Settings saved");
    } catch (reason: unknown) {
      settingsError =
        reason instanceof Error ? reason.message : "Unable to save settings";
    } finally {
      savingSettings = false;
    }
  }

  async function openAdministration() {
    if (dashboard?.user.role !== "administrator") return;
    adminError = "";
    pendingRemovalId = "";
    passwordLoginEnabled = authConfig.password_login_enabled;
    passwordRegistrationEnabled = authConfig.password_registration_enabled;
    oidcRegistrationEnabled = authConfig.oidc_registration_enabled;
    adminDialog?.showModal();
    loadingUsers = true;
    try {
      const [users, authentication] = await Promise.all([
        fetchManagedUsers(),
        fetchAuthenticationSettings(),
      ]);
      managedUsers = users;
      applyAuthenticationConfig(authentication);
    } catch (reason: unknown) {
      adminError =
        reason instanceof Error ? reason.message : "Unable to load users";
    } finally {
      loadingUsers = false;
    }
  }

  function applyAuthenticationConfig(config: AuthenticationConfig) {
    authConfig = config;
    passwordLoginEnabled = config.password_login_enabled;
    passwordRegistrationEnabled = config.password_registration_enabled;
    oidcRegistrationEnabled = config.oidc_registration_enabled;
    if (!config.password_login_enabled && authMode === "login") {
      authMode = config.password_registration_enabled ? "register" : "login";
    } else if (
      !config.password_registration_enabled &&
      authMode === "register"
    ) {
      authMode = "login";
    }
  }

  async function saveAuthenticationSettings() {
    if (loadingUsers || savingAuthenticationSettings) return;
    savingAuthenticationSettings = true;
    adminError = "";
    try {
      const updated = await updateAuthenticationSettings({
        password_login_enabled: passwordLoginEnabled,
        password_registration_enabled: passwordRegistrationEnabled,
        oidc_registration_enabled: oidcRegistrationEnabled,
      });
      applyAuthenticationConfig(updated);
      showToast("Authentication settings saved");
    } catch (reason: unknown) {
      adminError =
        reason instanceof Error
          ? reason.message
          : "Unable to save authentication settings";
    } finally {
      savingAuthenticationSettings = false;
    }
  }

  function handleRoleChange(event: Event, user: ManagedUser) {
    const role = (event.currentTarget as HTMLSelectElement)
      .value as ManagedUser["role"];
    void changeManagedUserRole(user, role);
  }

  async function changeManagedUserRole(
    user: ManagedUser,
    role: ManagedUser["role"],
  ) {
    if (user.role === role || mutatingUserId) return;
    mutatingUserId = user.id;
    adminError = "";
    try {
      const updated = await updateManagedUserRole(user.id, role);
      managedUsers = managedUsers.map((candidate) =>
        candidate.id === updated.id ? updated : candidate,
      );
      showToast(
        `${updated.display_name} is now ${role === "administrator" ? "an administrator" : "a member"}`,
      );
    } catch (reason: unknown) {
      adminError =
        reason instanceof Error ? reason.message : "Unable to update role";
    } finally {
      mutatingUserId = "";
    }
  }

  async function removeManagedUser(user: ManagedUser) {
    if (user.id === dashboard?.user.id || mutatingUserId) return;

    mutatingUserId = user.id;
    adminError = "";
    try {
      await deleteManagedUser(user.id);
      managedUsers = managedUsers.filter(
        (candidate) => candidate.id !== user.id,
      );
      pendingRemovalId = "";
      showToast(`${user.display_name} was removed`);
    } catch (reason: unknown) {
      adminError =
        reason instanceof Error ? reason.message : "Unable to remove user";
    } finally {
      mutatingUserId = "";
    }
  }

  function memberInitials(user: ManagedUser) {
    return user.display_name
      .split(/\s+/)
      .slice(0, 2)
      .map((part) => part[0]?.toUpperCase())
      .join("");
  }

  function memberSince(createdAt: string) {
    return new Intl.DateTimeFormat("en", {
      month: "short",
      year: "numeric",
    }).format(new Date(createdAt));
  }

  async function signOut() {
    try {
      await logoutAccount();
      settingsDialog?.close();
      dashboard = null;
      resetTaskArchiveView();
      clearAvatarDraft();
      avatarAvailable = true;
      avatarRevision = Date.now();
      activePage = { kind: "builtin", id: "dashboard" };
      localStorage.setItem("pandan-active-section", "builtin:dashboard");
      welcomeLeaving = false;
      authPassword = "";
      authError = "";
    } catch (reason: unknown) {
      showToast(
        reason instanceof Error ? reason.message : "Unable to sign out",
      );
    }
  }

  function runCommand(item: CommandItem | undefined) {
    if (!item) return;
    commandDialog?.close();
    item.run();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (focusDialog?.open) return;
    const target = event.target as HTMLElement;
    const isTyping =
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement;

    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      openCommand();
      return;
    }
    if (event.key === "/" && !isTyping) {
      event.preventDefault();
      openCommand();
      return;
    }
    if (
      /^[1-9]$/.test(event.key) &&
      !commandDialog?.open &&
      !isTyping &&
      !layoutEditing
    ) {
      openProductPage(productPages[Number(event.key) - 1].id);
    }
  }

  async function toggleTask(task: Task) {
    pendingTaskDeleteId = "";
    const optimistic = { ...task, completed: !task.completed };
    tasks = tasks.map((item) => (item.id === task.id ? optimistic : item));
    try {
      const updated = await setTaskCompleted(task.id, optimistic.completed);
      tasks = tasks.map((item) => (item.id === task.id ? updated : item));
    } catch (reason: unknown) {
      tasks = tasks.map((item) => (item.id === task.id ? task : item));
      showToast(
        reason instanceof Error ? reason.message : "Task update failed",
      );
    }
  }

  function toggleTaskDetails(taskId: string) {
    pendingTaskDeleteId = "";
    if (expandedTaskIds.has(taskId)) {
      expandedTaskIds.delete(taskId);
    } else {
      expandedTaskIds.add(taskId);
    }
  }

  async function toggleSubtask(task: Task, subtaskId: string) {
    const actionId = `${task.id}:${subtaskId}`;
    if (subtaskActionId) return;
    pendingTaskDeleteId = "";
    subtaskActionId = actionId;
    const optimistic = {
      ...task,
      subtasks: task.subtasks.map((subtask) =>
        subtask.id === subtaskId
          ? { ...subtask, completed: !subtask.completed }
          : subtask,
      ),
    };
    tasks = tasks.map((item) => (item.id === task.id ? optimistic : item));

    try {
      const updated = await updateTask(task.id, {
        subtasks: optimistic.subtasks.map(({ id, title, completed }) => ({
          id,
          title,
          completed,
        })),
      });
      tasks = tasks.map((item) => (item.id === task.id ? updated : item));
    } catch (reason: unknown) {
      tasks = tasks.map((item) => (item.id === task.id ? task : item));
      showToast(
        reason instanceof Error ? reason.message : "Subtask update failed",
      );
    } finally {
      subtaskActionId = "";
    }
  }

  async function addTask(title: string) {
    try {
      const task = await createTask(title);
      tasks = [...tasks, task];
      showToast("Task added");
    } catch (reason: unknown) {
      showToast(
        reason instanceof Error ? reason.message : "Task creation failed",
      );
      throw reason;
    }
  }

  async function clearCompleted() {
    const previousTasks = tasks;
    tasks = tasks.filter((task) => !task.completed);
    try {
      const { deleted } = await clearCompletedTasks();
      showToast(
        `${deleted} completed ${deleted === 1 ? "task" : "tasks"} cleared`,
      );
    } catch (reason: unknown) {
      tasks = previousTasks;
      showToast(
        reason instanceof Error ? reason.message : "Unable to clear tasks",
      );
    }
  }

  function taskPriorityLabel(priority: Task["priority"]) {
    return priority.toUpperCase();
  }

  function taskRepeatLabel(task: Task) {
    if (task.repeat_rule === "none") return "";
    if (task.repeat_rule !== "custom") return task.repeat_rule;
    return `every ${task.repeat_interval} ${task.repeat_unit}`;
  }

  function formatTaskDate(value: string) {
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    }).format(new Date(`${value}T12:00:00`));
  }
</script>

<!--
  The document title, description, and link preview tags live in `app.html` so
  crawlers see them without running the application. Setting them here as well
  would append a second `<title>` to the head, and the browser honours the first.
-->
<svelte:window onkeydown={handleKeydown} />

{#if loadingOverlayVisible}
  <div
    class={[
      "welcome-overlay",
      "loading-welcome-overlay",
      welcomeLeaving && "is-leaving",
    ]}
    style:--welcome-background={wallpaperBackground("welcome")}
    role="status"
    aria-live="polite"
    data-od-id="account-loading-screen"
  >
    <div class="welcome-monogram" aria-hidden="true">P&gt;</div>
    <div class="welcome-copy">
      <strong>Welcome:{loadingWelcomeDisplayName}</strong>
      <div class="welcome-loading-track" aria-hidden="true"><i></i></div>
      <div class="welcome-boot-log" aria-label="Session startup status">
        <samp>[ SESSION AUTHENTICATED ]</samp>
        <samp>{loadingWelcomeDisplayName}@pandan</samp>
        <samp
          >dashboard.init --profile={loadingWelcomeDisplayName.toLowerCase()}</samp
        >
      </div>
    </div>
  </div>
{/if}

{#if setupRequired}
  <main
    class="auth-shell"
    style:--login-background={wallpaperBackground("login")}
    data-od-id="administrator-onboarding"
  >
    <div class="auth-brand" aria-label="Pandan">
      <span class="auth-brand-glyph" aria-hidden="true">P&gt;</span>
      <span>PANDAN</span>
    </div>
    <aside class="auth-context" aria-labelledby="setup-context-title">
      <div class="auth-context-copy">
        <span>[ FIRST RUN ]</span>
        <h2 id="setup-context-title">Initialize your workspace.</h2>
        <p>
          Claim the administrator account, then configure dashboards, users, and
          connected services from one control surface.
        </p>
      </div>
      <dl class="auth-capabilities">
        <div>
          <dt>Administration</dt>
          <dd>User roles and account access</dd>
        </div>
        <div>
          <dt>Personalization</dt>
          <dd>Widgets, wallpaper, and display tuning</dd>
        </div>
        <div>
          <dt>Providers</dt>
          <dd>Encrypted service credentials</dd>
        </div>
      </dl>
    </aside>
    <section
      class="auth-card setup-card"
      aria-labelledby="setup-title"
      data-od-id="administrator-setup-card"
    >
      <div class="setup-badge">Setup required</div>
      <div class="auth-copy">
        <p class="widget-kicker">[ INSTALLATION OWNER ]</p>
        <h1 id="setup-title">Create your administrator.</h1>
        <p>
          Create this first account with a password or your configured single
          sign-on provider. It owns the workspace and can manage every user who
          joins later.
        </p>
      </div>

      <form
        class="auth-form setup-form"
        onsubmit={completeSetup}
        data-od-id="administrator-setup-form"
      >
        <label for="setup-name">Display name</label>
        <input
          id="setup-name"
          class="text-input"
          bind:value={authDisplayName}
          autocomplete="name"
          maxlength="60"
          required
        />

        <label for="setup-email">Administrator email</label>
        <input
          id="setup-email"
          class="text-input"
          type="email"
          bind:value={authEmail}
          autocomplete="email"
          maxlength="254"
          required
        />

        <div class="password-label">
          <label for="setup-password">Password</label><span
            >10 characters minimum</span
          >
        </div>
        <input
          id="setup-password"
          class="text-input"
          type="password"
          bind:value={authPassword}
          autocomplete="new-password"
          minlength="10"
          maxlength="128"
          required
        />

        {#if authError || data.error}
          <p class="form-error" role="alert">{authError || data.error}</p>
        {/if}

        <button
          class="ui-button ui-button--primary primary-btn auth-submit"
          type="submit"
          disabled={authenticating}
          data-od-id="create-administrator"
        >
          {authenticating ? "Securing workspace…" : "Create administrator"}
        </button>
      </form>

      {#if authConfig.oidc_enabled}
        <div class="auth-divider">
          <span>or</span>
        </div>
        <button
          class="ui-button ui-button--secondary oidc-btn"
          type="button"
          onclick={() => window.location.assign("/api/auth/oidc/start")}
          disabled={authenticating}
          data-od-id="oidc-create-administrator"
        >
          Create with {authConfig.oidc_provider_name ?? "single sign-on"}
          <ArrowRight size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
      {/if}

      <div class="setup-note">
        <p>
          Only one initial administrator can be created. This setup closes
          permanently after success.
        </p>
      </div>
    </section>
    <p class="auth-footnote">
      One-time setup. Encrypted password. Private dashboard.
    </p>
  </main>
{:else if !dashboard}
  <main
    class="auth-shell"
    style:--login-background={wallpaperBackground("login")}
    data-od-id="login-page"
  >
    <div class="auth-brand" aria-label="Pandan">
      <span class="auth-brand-glyph" aria-hidden="true">P&gt;</span>
      <span>PANDAN</span>
    </div>
    <aside class="auth-context" aria-labelledby="auth-context-title">
      <div class="auth-context-copy">
        <span>[ PRIVATE WORKSPACE ]</span>
        <h2 id="auth-context-title">Your private workspace.</h2>
        <p>
          Return to your dashboards, tasks, calendars, feeds, journal, and
          release activity.
        </p>
      </div>
      <dl class="auth-capabilities">
        <div>
          <dt>Dashboard</dt>
          <dd>Configurable widgets and appearance</dd>
        </div>
        <div>
          <dt>Planning</dt>
          <dd>Tasks, calendars, and journal</dd>
        </div>
        <div>
          <dt>Sources</dt>
          <dd>Feeds, video, and release activity</dd>
        </div>
      </dl>
    </aside>
    <section
      class="auth-card"
      aria-labelledby="auth-title"
      data-od-id="account-access-card"
    >
      <div class="auth-copy">
        <p class="widget-kicker">[ ACCOUNT ACCESS ]</p>
        <h1 id="auth-title">
          {!passwordAccessEnabled || authMode === "login"
            ? "Welcome back."
            : "Make it yours."}
        </h1>
        <p>
          {!passwordAccessEnabled
            ? "Continue with your organization’s single sign-on provider."
            : authMode === "login"
              ? "Sign in to return to your widgets, tasks, and preferences."
              : "Create an account for a private dashboard that follows your settings."}
        </p>
      </div>

      {#if authConfig.password_login_enabled && authConfig.password_registration_enabled}
        <div class="auth-modes" aria-label="Account access mode">
          <button
            type="button"
            aria-pressed={authMode === "login"}
            onclick={() => setAuthMode("login")}>Sign in</button
          >
          <button
            type="button"
            aria-pressed={authMode === "register"}
            onclick={() => setAuthMode("register")}>Create account</button
          >
        </div>
      {/if}

      {#if (authMode === "login" && authConfig.password_login_enabled) || (authMode === "register" && authConfig.password_registration_enabled)}
        <form
          class="auth-form"
          onsubmit={authenticate}
          data-od-id="account-access-form"
        >
          {#if authMode === "register"}
            <label for="display-name">Display name</label>
            <input
              id="display-name"
              class="text-input"
              bind:value={authDisplayName}
              autocomplete="name"
              maxlength="60"
              required
            />
          {/if}

          <label for="email">Email</label>
          <input
            id="email"
            class="text-input"
            type="email"
            bind:value={authEmail}
            autocomplete="email"
            maxlength="254"
            required
          />

          <div class="password-label">
            <label for="password">Password</label><span
              >10 characters minimum</span
            >
          </div>
          <input
            id="password"
            class="text-input"
            type="password"
            bind:value={authPassword}
            autocomplete={authMode === "login"
              ? "current-password"
              : "new-password"}
            minlength="10"
            maxlength="128"
            required
          />

          <button
            class="ui-button ui-button--primary primary-btn auth-submit"
            type="submit"
            disabled={authenticating}
            data-od-id="account-submit"
          >
            {authenticating
              ? "One moment…"
              : authMode === "login"
                ? "Enter dashboard"
                : "Create my dashboard"}
          </button>
        </form>
      {/if}

      {#if authError || data.error}
        <p class="form-error" role="alert">{authError || data.error}</p>
      {/if}

      {#if authConfig.oidc_enabled}
        {#if passwordAccessEnabled}<div class="auth-divider">
            <span>or</span>
          </div>{/if}
        <button
          class="ui-button ui-button--secondary oidc-btn"
          type="button"
          onclick={() => window.location.assign("/api/auth/oidc/start")}
          data-od-id="oidc-login"
        >
          Continue with {authConfig.oidc_provider_name ?? "single sign-on"}
          <ArrowRight size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
      {/if}
    </section>
    <p class="auth-footnote">
      Secure sessions. Personal settings. Private tasks.
    </p>
  </main>
{:else}
  <div
    class={[
      "dashboard-app",
      layoutEditing && "is-editing",
      sidebarOpen && "sidebar-is-open",
      sidebarCollapsed && "sidebar-is-collapsed",
    ]}
    style:--dashboard-background={wallpaperBackground("welcome")}
    style:--wallpaper-blur={`${backgroundBlur}px`}
    style:--wallpaper-brightness={`${backgroundBrightness}%`}
    style:--wallpaper-contrast={`${backgroundContrast}%`}
    style:--wallpaper-saturation={`${backgroundSaturation}%`}
    data-od-id="dashboard-shell"
  >
    <aside
      id="primary-sidebar"
      class="dashboard-sidebar"
      inert={mobileNavigation.current && !sidebarOpen}
      {@attach captureSidebar}
      onpointerover={showSidebarHint}
      onpointerout={hideSidebarHint}
      onfocusin={showSidebarHint}
      onfocusout={hideSidebarHint}
      data-od-id="primary-sidebar"
    >
      <button
        class="sidebar-brand"
        type="button"
        onclick={() => openProductPage("dashboard")}
        aria-label="Open dashboard"
        aria-describedby="sidebar-desc-brand"
        data-sidebar-title="Pandan dashboard"
        data-sidebar-description="Return to your personal dashboard overview."
      >
        <span class="brand-glyph">P&gt;</span>
        <span class="brand-word">PANDAN / OS</span>
      </button>
      <span id="sidebar-desc-brand" class="sr-only"
        >Return to your personal dashboard overview.</span
      >

      <nav class="sidebar-nav" aria-label="Primary navigation">
        {#each productPages as item (item.id)}
          {@const PageIcon = item.icon}
          {#if item.id === "kanban"}
            <div class="sidebar-nav-group" data-od-id="nav-kanban-group">
              <button
                class="sidebar-link"
                type="button"
                aria-current={activeSection === "kanban" ? "page" : undefined}
                aria-expanded={kanbanMenuOpen}
                aria-describedby="sidebar-desc-kanban"
                onclick={() => {
                  kanbanMenuOpen = !kanbanMenuOpen;
                  if (activeSection !== "kanban") openKanbanSection("boards");
                }}
                data-sidebar-title={item.label}
                data-sidebar-description={item.description}
                data-od-id="nav-kanban"
              >
                <span class="sidebar-index">{item.code}</span>
                <PageIcon size={19} strokeWidth={1.7} aria-hidden="true" />
                <span>Kanban</span>
                <ChevronDown class={kanbanMenuOpen ? "is-open" : undefined} size={15} strokeWidth={1.7} aria-hidden="true" />
              </button>
              <span id="sidebar-desc-kanban" class="sr-only"
                >{item.description}</span
              >
              {#if kanbanMenuOpen && !sidebarCollapsed}
                <div class="sidebar-submenu" aria-label="Kanban navigation">
                  {#each kanbanSubmenuItems as submenuItem (submenuItem.id)}
                    <button
                      type="button"
                      class:active={activeSection === "kanban" && kanbanSection === submenuItem.id}
                      aria-describedby={`sidebar-desc-kanban-${submenuItem.id}`}
                      onclick={() => openKanbanSection(submenuItem.id)}
                      data-sidebar-title={submenuItem.label}
                      data-sidebar-description={submenuItem.description}
                    >{submenuItem.label}</button>
                    <span
                      id={`sidebar-desc-kanban-${submenuItem.id}`}
                      class="sr-only">{submenuItem.description}</span
                    >
                  {/each}
                </div>
              {/if}
            </div>
          {:else}
            <button
              class="sidebar-link"
              type="button"
              aria-current={activeSection === item.id ? "page" : undefined}
              aria-describedby={`sidebar-desc-${item.id}`}
              onclick={() => openProductPage(item.id)}
              data-sidebar-title={item.label}
              data-sidebar-description={item.description}
              data-od-id={`nav-${item.id}`}
            >
              <span class="sidebar-index">{item.code}</span>
              <PageIcon size={19} strokeWidth={1.7} aria-hidden="true" />
              <span>{item.label}</span>
            </button>
            <span id={`sidebar-desc-${item.id}`} class="sr-only"
              >{item.description}</span
            >
          {/if}
        {/each}

        {#if dashboard.embedded_pages.global.length}
          <div
            class="sidebar-custom-group"
            data-od-id="nav-global-custom-pages"
          >
            <span class="sidebar-custom-group-label">Global custom</span>
            {#each dashboard.embedded_pages.global as page (page.id)}
              <button
                class="sidebar-link sidebar-link--custom"
                type="button"
                aria-label={`${page.title}, global custom page`}
                aria-current={activePage.kind === "embedded" &&
                activePage.id === page.id
                  ? "page"
                  : undefined}
                onclick={() => openEmbeddedPage(page.id)}
                data-sidebar-title={page.title}
                data-sidebar-description={`Global custom page · ${page.description}`}
                data-od-id={`nav-global-custom-${page.id}`}
              >
                <b class="sidebar-custom-marker" aria-hidden="true">G</b>
                <PanelTop size={18} strokeWidth={1.7} aria-hidden="true" />
                <span class="sidebar-custom-title">{page.title}</span>
                <span class="sidebar-custom-scope">GLOBAL · CUSTOM</span>
              </button>
            {/each}
          </div>
        {/if}

        {#if dashboard.embedded_pages.personal.length}
          <div
            class="sidebar-custom-group"
            data-od-id="nav-personal-custom-pages"
          >
            <span class="sidebar-custom-group-label">Personal custom</span>
            {#each dashboard.embedded_pages.personal as page (page.id)}
              <button
                class="sidebar-link sidebar-link--custom"
                type="button"
                aria-label={`${page.title}, personal custom page`}
                aria-current={activePage.kind === "embedded" &&
                activePage.id === page.id
                  ? "page"
                  : undefined}
                onclick={() => openEmbeddedPage(page.id)}
                data-sidebar-title={page.title}
                data-sidebar-description={`Personal custom page · ${page.description}`}
                data-od-id={`nav-personal-custom-${page.id}`}
              >
                <b class="sidebar-custom-marker" aria-hidden="true">U</b>
                <PanelTop size={18} strokeWidth={1.7} aria-hidden="true" />
                <span class="sidebar-custom-title">{page.title}</span>
                <span class="sidebar-custom-scope">PERSONAL · CUSTOM</span>
              </button>
            {/each}
          </div>
        {/if}
      </nav>

      <div class="sidebar-footer">
        <div class="sidebar-utilities-shell">
          {#key `${dashboard.settings.location}:${dashboard.settings.timezone}:${dashboard.settings.temperature_unit}:${dashboard.settings.sidebar_timezones.join("|")}`}
            <SidebarUtilities
              settings={dashboard.settings}
              onToast={showToast}
              onSettingsChange={applySidebarSettings}
            />
          {/key}
        </div>
        <button
          class="sidebar-link"
          type="button"
          onclick={openSettings}
          aria-describedby="sidebar-desc-settings"
          data-sidebar-title="Settings"
          data-sidebar-description="Manage your profile, appearance, authentication, and integrations."
          data-od-id="open-user-settings"
        >
          <span class="sidebar-index">13</span>
          <Settings size={19} strokeWidth={1.7} aria-hidden="true" />
          <span>Settings</span>
        </button>
        <span id="sidebar-desc-settings" class="sr-only"
          >Manage your profile, appearance, authentication, and integrations.</span
        >
        <button
          class="sidebar-profile"
          type="button"
          onclick={openSettings}
          aria-label="Open account settings"
          aria-describedby="sidebar-desc-account"
          data-sidebar-title="Account"
          data-sidebar-description="Review your account identity, role, and personal preferences."
        >
          <span class="sidebar-avatar">
            {#if avatarAvailable}
              <img
                src={avatarUrl()}
                alt=""
                onload={() => (avatarAvailable = true)}
                onerror={() => (avatarAvailable = false)}
              />
            {:else}
              {profileInitials}
            {/if}
          </span>
          <span class="sidebar-profile-copy">
            <strong>{dashboard.settings.display_name}</strong>
            <small>{dashboard.user.role}</small>
          </span>
          <Ellipsis size={18} strokeWidth={1.7} aria-hidden="true" />
        </button>
        <span id="sidebar-desc-account" class="sr-only"
          >Review your account identity, role, and personal preferences.</span
        >
      </div>

      {#if sidebarHint}
        <div
          class="sidebar-hint"
          aria-hidden="true"
          style:--sidebar-hint-y={`${sidebarHint.top}px`}
          data-od-id="sidebar-context-popup"
        >
          <strong>{sidebarHint.title}</strong>
          <p>{sidebarHint.description}</p>
        </div>
      {/if}
    </aside>

    <button
      class="sidebar-scrim"
      type="button"
      aria-label="Close navigation"
      onclick={() => (sidebarOpen = false)}
    ></button>

    <main class="dashboard-main" data-od-id="dashboard-main">
      <header class="dashboard-header">
        <div class="dashboard-title-group">
          <button
            class="ui-button ui-button--ghost ui-button--icon mobile-menu-button"
            type="button"
            aria-label={mobileNavigation.current
              ? sidebarOpen
                ? "Close navigation"
                : "Open navigation"
              : sidebarCollapsed
                ? "Expand sidebar"
                : "Collapse sidebar"}
            aria-controls="primary-sidebar"
            aria-expanded={mobileNavigation.current
              ? sidebarOpen
              : !sidebarCollapsed}
            title={mobileNavigation.current
              ? sidebarOpen
                ? "Close navigation"
                : "Open navigation"
              : sidebarCollapsed
                ? "Expand sidebar"
                : "Collapse sidebar"}
            onclick={toggleSidebar}
            data-od-id="toggle-sidebar"
          >
            <Menu size={20} strokeWidth={1.7} aria-hidden="true" />
          </button>
          <div>
            <h1>$ {activeSectionLabel.toLowerCase()}</h1>
            <p>
              SYS.DATE / {dateLabel} · SYS.TIME /
              <time
                class="dashboard-header-clock"
                datetime={currentTime.toISOString()}>{timeLabel}</time
              >
              {timezoneLabel}
            </p>
          </div>
        </div>

        <div class="dashboard-header-actions">
          <button
            class="ui-button ui-button--ghost ui-button--icon header-icon-button"
            type="button"
            aria-label="Search"
            onclick={openCommand}
          >
            <Search size={19} strokeWidth={1.7} aria-hidden="true" />
          </button>
          <button
            class="ui-button ui-button--ghost ui-button--icon header-icon-button"
            type="button"
            aria-label="Notifications"
            onclick={() => showToast("No new notifications")}
          >
            <Bell size={19} strokeWidth={1.7} aria-hidden="true" />
          </button>
          {#if activeSection === "dashboard"}
            <button
              class={["dashboard-edit-button", layoutEditing && "is-active"]}
              type="button"
              aria-pressed={layoutEditing}
              disabled={savingLayout}
              onclick={toggleLayoutEditing}
              data-od-id="edit-dashboard-layout"
            >
              <SlidersHorizontal
                size={18}
                strokeWidth={1.7}
                aria-hidden="true"
              />
              <span>{layoutEditing ? "Done" : "Edit layout"}</span>
            </button>
            <button
              class="ui-button ui-button--primary dashboard-add-button"
              type="button"
              onclick={openWidgetLibrary}
              data-od-id="open-widget-library"
            >
              <Plus size={18} strokeWidth={1.8} aria-hidden="true" />
              <span>Add widget</span>
            </button>
          {/if}
          <a
            class="ui-button ui-button--secondary github-star-button"
            href="https://github.com/NekoShinobi/pandan"
            target="_blank"
            rel="noreferrer"
            title="Star Pandan on GitHub"
            data-od-id="github-star-link"
          >
            <Star size={17} strokeWidth={1.7} aria-hidden="true" />
            <span>GitHub Star</span>
          </a>
        </div>
      </header>

      {#key `${activePage.kind}:${activePage.id}`}
        <div
          class={[
            "product-view",
            activeSection !== "youtube" && "is-translucent",
          ]}
        >
          {#if activeSection === "dashboard"}
            <section class="dashboard-home" data-od-id="dashboard-overview">
              {#if data.error}
                <div class="api-notice" role="status">
                  {data.error}. Start the Rust API to load persisted widgets.
                </div>
              {/if}

              <div class="dashboard-composition">
                <div class="dashboard-primary-column">
                  <section class="dashboard-intro" data-od-id="daily-overview">
                    <div>
                      <p>[ SESSION / READY ]</p>
                      <h2>welcome:{firstName}</h2>
                      <span>$ dashboard status --widgets --utilities</span>
                    </div>
                  </section>

                  <section
                    class="custom-widget-section"
                    data-od-id="custom-widgets"
                  >
                    <div class="custom-widget-heading">
                      <div>
                        <h3>[ MODULES / USER ]</h3>
                        <p>
                          Move and resize these modules when layout editing is
                          enabled.
                        </p>
                      </div>
                    </div>
                    <div
                      class={[
                        "grid-stack",
                        "widget-canvas",
                        draggedWidgetId && "is-dragging",
                      ]}
                      role="list"
                      aria-label="Dashboard widgets"
                      data-od-id="widget-grid-dashboard"
                      {@attach gridAttachment(0)}
                    >
                      {#each dashboardWidgets() as widget (widget.id)}
                        <div
                          class="grid-stack-item"
                          {...gridAttributes(widget)}
                          data-widget-id={widget.id}
                        >
                          <div class="grid-stack-item-content">
                            <DashboardWidgetCard
                              {widget}
                              editing={layoutEditing}
                              {tasks}
                              {feeds}
                              settings={dashboard.settings}
                              {completedCount}
                              {taskProgress}
                              {todayTasks}
                              {todayCompletedCount}
                              {todayTaskProgress}
                              {savingLayout}
                              onToggleTask={toggleTask}
                              onCreateTask={addTask}
                              onClearCompleted={clearCompleted}
                              onStartFocus={startDashboardFocusSession}
                              onToast={showToast}
                              onOpenCalendarDate={openDashboardCalendarDate}
                              onRemove={removeWidget}
                              onUpdateWidget={updateWidgetInstance}
                            />
                          </div>
                        </div>
                      {/each}
                    </div>
                    {#if dashboardWidgets().length === 0}
                      <button
                        class="empty-workspace"
                        type="button"
                        onclick={openWidgetLibrary}
                      >
                        Add the first widget to your dashboard
                      </button>
                    {/if}
                  </section>
                </div>

                <aside
                  class="dashboard-utility-rail"
                  aria-label="Dashboard shortcuts"
                >
                  <section
                    class="utility-box utility-analog-clock"
                    data-od-id="dashboard-analog-clock"
                  >
                    <p>[ LOCAL.TIME ]</p>
                    <div
                      class="analog-clock"
                      role="img"
                      aria-label={`${dashboardClock.label} in ${dashboardClock.zone}`}
                    >
                      {#each clockMarks as mark (mark)}
                        <i
                          class="analog-clock-mark"
                          style:--mark-angle={`${mark * 30}deg`}
                        ></i>
                      {/each}
                      <i
                        class="analog-clock-hand is-hour"
                        style:--hand-angle={`${dashboardClock.hourAngle}deg`}
                      ></i>
                      <i
                        class="analog-clock-hand is-minute"
                        style:--hand-angle={`${dashboardClock.minuteAngle}deg`}
                      ></i>
                      <i
                        class="analog-clock-hand is-second"
                        style:--hand-angle={`${dashboardClock.secondAngle}deg`}
                      ></i>
                      <i class="analog-clock-pin"></i>
                    </div>
                    <span>{dashboardClock.label} / {dashboardClock.zone}</span>
                  </section>
                  <section
                    class="utility-box utility-calendar"
                    data-od-id="dashboard-calendar"
                  >
                    <p>[ CALENDAR ]</p>
                    <div class="utility-calendar-date">
                      <strong>{dateLabel}</strong>
                      <span>{dashboardCalendarMonthLabel}</span>
                    </div>
                    <div
                      class="utility-calendar-grid"
                      aria-label={`${dashboardCalendarMonthLabel} calendar`}
                      data-od-id="dashboard-calendar-month"
                    >
                      {#each dashboardCalendarWeekdays as weekday, index (`${weekday}-${index}`)}
                        <span
                          class="utility-calendar-weekday"
                          aria-hidden="true">{weekday}</span
                        >
                      {/each}
                      {#each dashboardCalendarDays as day (day.key)}
                        <button
                          class={[
                            "utility-calendar-day",
                            !day.currentMonth && "is-outside",
                            day.today && "is-today",
                          ]}
                          type="button"
                          onclick={() => openDashboardCalendarDate(day.key)}
                          aria-label={day.key}
                          aria-current={day.today ? "date" : undefined}
                          data-od-id={`dashboard-calendar-day-${day.key}`}
                        >
                          <time datetime={day.key}>{day.day}</time>
                        </button>
                      {/each}
                    </div>
                  </section>
                  <section class="utility-box utility-progress">
                    <p>[ TASK.PROGRESS ]</p>
                    <strong
                      >{completedCount}<span> / {tasks.length}</span></strong
                    >
                    <span>Completed tasks</span>
                  </section>
                  <section class="utility-box utility-shortcuts">
                    <p>[ COMMANDS ]</p>
                    <button type="button" onclick={openWidgetLibrary}
                      >&gt; add widget</button
                    >
                  </section>
                </aside>
              </div>
            </section>
          {:else if activeSection === "tasks"}
            <section class="feature-page product-page" data-od-id="tasks-page">
              <div class="feature-page-intro task-page-intro page-header">
                <div>
                  <TypedHeading
                    text={`$ tasks --${taskView}`}
                    odId="tasks-heading"
                  />
                  {#key taskView}
                    <p class="view-swap-copy">
                      {taskView === "active"
                        ? "Plan work with due dates, priorities, labels, recurring schedules, and subtasks."
                        : "Review tasks removed from the active plan, restore what matters, or delete them permanently."}
                    </p>
                  {/key}
                </div>
                <div class="task-page-actions">
                  <label class="task-label-filter">
                    <span>
                      <Tag size={14} strokeWidth={1.8} aria-hidden="true" />
                      Label
                    </span>
                    <select
                      bind:value={taskLabelFilter}
                      disabled={!taskLabelOptions.length}
                      aria-label="Filter tasks by label"
                      data-od-id="filter-tasks-by-label"
                    >
                      <option value="">All labels</option>
                      {#each taskLabelOptions as label (label)}
                        <option value={label}>{label}</option>
                      {/each}
                    </select>
                  </label>
                  <nav class="task-view-menu" aria-label="Task views">
                    <button
                      class={[
                        "ui-button",
                        "ui-button--secondary",
                        "task-view-menu-button",
                        taskViewTarget === "active" && "is-active",
                      ]}
                      type="button"
                      aria-pressed={taskViewTarget === "active"}
                      onclick={() => selectTaskView("active")}
                      data-od-id="view-active-tasks"
                    >
                      Active
                      <span>{tasks.length}</span>
                    </button>
                    <button
                      class={[
                        "ui-button",
                        "ui-button--secondary",
                        "task-view-menu-button",
                        taskViewTarget === "archived" && "is-active",
                      ]}
                      type="button"
                      aria-pressed={taskViewTarget === "archived"}
                      onclick={() => selectTaskView("archived")}
                      data-od-id="view-archived-tasks"
                    >
                      Archived
                      <span>{archivedTasksLoaded ? archivedTasks.length : "—"}</span>
                    </button>
                  </nav>
                  <button
                    class="ui-button ui-button--primary primary-btn task-create-button"
                    type="button"
                    onclick={() => openTaskEditor()}
                    data-od-id="create-task"
                  >
                    <Plus size={17} strokeWidth={1.8} aria-hidden="true" />
                    New task
                  </button>
                </div>
              </div>
              <div class="tasks-page-layout">
                <section
                  class="tasks-worklist view-swap"
                  data-view-phase={taskViewSwap.phase}
                  data-view-direction={taskViewSwap.direction}
                  {@attach taskViewSwap.attach}
                  data-od-id={taskView === "active"
                    ? "task-due-groups"
                    : "archived-task-list"}
                >
                  {#snippet taskRow(task: Task, archived: boolean)}
                    <article
                      class={[
                        "task-page-row",
                        task.completed && "is-complete",
                        archived && "is-archived",
                      ]}
                      data-od-id={`task-row-${task.id}`}
                    >
                      <div class="task-row-main">
                        {#if archived}
                          <span class="task-archive-marker" aria-hidden="true">
                            <ArchiveIcon size={16} strokeWidth={1.8} />
                          </span>
                        {:else}
                          <button
                            class="task-complete-button"
                            type="button"
                            aria-label={task.completed
                              ? `Mark ${task.title} incomplete`
                              : `Complete ${task.title}`}
                            onclick={() => toggleTask(task)}
                          >
                            <span class="focus-check" aria-hidden="true"></span>
                          </button>
                        {/if}
                        <button
                          class="task-row-content"
                          type="button"
                          aria-expanded={expandedTaskIds.has(task.id)}
                          aria-controls={`task-details-${task.id}`}
                          onclick={() => toggleTaskDetails(task.id)}
                          data-od-id={`expand-task-${task.id}`}
                        >
                          <span class="task-row-heading">
                            {#if task.priority !== "none"}
                              <span
                                class={[
                                  "task-priority",
                                  `priority-${task.priority}`,
                                ]}
                              >
                                {taskPriorityLabel(task.priority)}
                              </span>
                            {/if}
                            <strong>{task.title}</strong>
                            <ChevronDown
                              class={expandedTaskIds.has(task.id)
                                ? "is-expanded"
                                : ""}
                              size={16}
                              strokeWidth={1.8}
                              aria-hidden="true"
                            />
                          </span>
                          <span class="task-row-metadata">
                            {#if task.due_date}
                              <span>
                                <CalendarDays
                                  size={13}
                                  strokeWidth={1.8}
                                  aria-hidden="true"
                                />
                                {formatTaskDate(task.due_date)}
                              </span>
                            {/if}
                            {#if task.repeat_rule !== "none"}
                              <span>
                                <Repeat2
                                  size={13}
                                  strokeWidth={1.8}
                                  aria-hidden="true"
                                />
                                {taskRepeatLabel(task)}
                              </span>
                            {/if}
                            {#if task.subtasks.length}
                              <span>
                                <CheckSquare2
                                  size={13}
                                  strokeWidth={1.8}
                                  aria-hidden="true"
                                />
                                {task.subtasks.filter((item) => item.completed)
                                  .length}/{task.subtasks.length}
                              </span>
                            {/if}
                            {#if task.attachments.length}
                              <span>
                                <Paperclip
                                  size={13}
                                  strokeWidth={1.8}
                                  aria-hidden="true"
                                />
                                {task.attachments.length}
                              </span>
                            {/if}
                          </span>
                          {#if task.labels.length}
                            <span class="task-label-list">
                              {#each task.labels as label (label)}
                                <span>{label}</span>
                              {/each}
                            </span>
                          {/if}
                        </button>
                        <div
                          class="task-row-actions"
                          role="group"
                          aria-label={`Actions for ${task.title}`}
                        >
                          {#if archived}
                            <button
                              class="ui-button ui-button--secondary task-row-action task-row-restore-action"
                              type="button"
                              disabled={taskActionId === task.id}
                              aria-label={`Restore ${task.title}`}
                              title="Restore task"
                              data-od-id={`restore-task-${task.id}`}
                              onclick={() => restoreTaskFromArchive(task)}
                            >
                              <RotateCcw
                                size={15}
                                strokeWidth={1.8}
                                aria-hidden="true"
                              />
                              <span>Restore</span>
                            </button>
                          {:else}
                            <button
                              class="ui-button ui-button--secondary task-row-action task-row-edit-action"
                              type="button"
                              disabled={taskActionId === task.id}
                              aria-label={`Edit ${task.title}`}
                              title="Edit task"
                              data-od-id={`edit-task-${task.id}`}
                              onclick={() => openTaskEditor(task)}
                            >
                              <Pencil
                                size={15}
                                strokeWidth={1.8}
                                aria-hidden="true"
                              />
                              <span>Edit</span>
                            </button>
                            <button
                              class="ui-button ui-button--danger task-row-action task-row-archive-action"
                              type="button"
                              disabled={taskActionId === task.id}
                              aria-label={`Archive ${task.title}`}
                              title="Archive task"
                              data-od-id={`archive-task-${task.id}`}
                              onclick={() => archiveTaskFromList(task)}
                            >
                              <ArchiveIcon
                                size={15}
                                strokeWidth={1.8}
                                aria-hidden="true"
                              />
                              <span>Archive</span>
                            </button>
                          {/if}
                          <button
                            class={[
                              "ui-button",
                              "ui-button--danger",
                              "task-row-action",
                              "task-row-delete-action",
                              pendingTaskDeleteId === task.id && "is-armed",
                            ]}
                            type="button"
                            disabled={taskActionId === task.id}
                            aria-label={pendingTaskDeleteId === task.id
                              ? `Confirm deletion of ${task.title}`
                              : `Delete ${task.title}`}
                            title={pendingTaskDeleteId === task.id
                              ? "Select again to confirm deletion"
                              : "Delete task"}
                            data-od-id={`delete-task-${task.id}`}
                            onclick={() =>
                              archived
                                ? deleteArchivedTaskFromList(task)
                                : deleteTaskFromList(task)}
                          >
                            <Trash2
                              size={15}
                              strokeWidth={1.8}
                              aria-hidden="true"
                            />
                            <span
                              >{pendingTaskDeleteId === task.id
                                ? "Confirm"
                                : "Delete"}</span
                            >
                          </button>
                        </div>
                      </div>

                      {#if expandedTaskIds.has(task.id)}
                        <div
                          class="task-row-details"
                          id={`task-details-${task.id}`}
                          data-od-id={`task-details-${task.id}`}
                        >
                          {#if task.description}
                            <div class="task-description-block">
                              <span>Description</span>
                              <p>{task.description}</p>
                            </div>
                          {/if}
                          {#if task.subtasks.length}
                            <div class="task-subtask-block">
                              <div class="task-subtask-heading">
                                <span>Subtasks</span>
                                <small>
                                  {task.subtasks.filter(
                                    (item) => item.completed,
                                  ).length} / {task.subtasks.length} done
                                </small>
                              </div>
                              <div class="task-subtask-list">
                                {#each task.subtasks as subtask (subtask.id)}
                                  <button
                                    type="button"
                                    class={[
                                      "ui-toggle-button",
                                      "task-subtask-row",
                                      subtask.completed && "is-complete",
                                    ]}
                                    aria-pressed={subtask.completed}
                                    disabled={archived ||
                                      subtaskActionId ===
                                        `${task.id}:${subtask.id}`}
                                    aria-label={subtask.completed
                                      ? `Mark ${subtask.title} incomplete`
                                      : `Complete ${subtask.title}`}
                                    onclick={() =>
                                      !archived && toggleSubtask(task, subtask.id)}
                                  >
                                    <span class="ui-toggle-indicator" aria-hidden="true">{#if subtask.completed}<Check size={13} />{/if}</span>
                                    <span>{subtask.title}</span>
                                    <small
                                      >{subtask.completed
                                        ? "Done"
                                        : "Open"}</small
                                    >
                                  </button>
                                {/each}
                              </div>
                            </div>
                          {/if}
                          {#if !task.description && !task.subtasks.length}
                            <p class="task-detail-empty">
                              No description or subtasks yet.
                            </p>
                          {/if}
                        </div>
                      {/if}
                    </article>
                  {/snippet}

                  {#if taskView === "active"}
                    {#if filteredActiveTasks.length}
                      {#each taskDueGroups as group (group.id)}
                        <section
                          class="task-due-group"
                          data-od-id={`task-group-${group.id}`}
                        >
                          <header class="task-due-group-heading">
                            <div>
                              <h3>{group.label}</h3>
                              <span>{group.range}</span>
                            </div>
                            <strong>{group.tasks.length}</strong>
                          </header>
                          {#if group.tasks.length}
                            <AnimatedList
                              items={group.tasks}
                              getKey={(task) => task.id}
                              showGradients={false}
                              enableArrowNavigation={false}
                              displayScrollbar={false}
                              class="tasks-animated-list"
                            >
                              {#snippet children(task)}
                                {@render taskRow(task, false)}
                              {/snippet}
                            </AnimatedList>
                          {:else}
                            <p class="task-group-empty">
                              No tasks in this range.
                            </p>
                          {/if}
                        </section>
                      {/each}
                    {:else if tasks.length && taskLabelFilter}
                      <div class="large-empty-state">
                        <Tag size={32} strokeWidth={1.5} aria-hidden="true" />
                        <h3>No tasks labelled {taskLabelFilter}</h3>
                        <p>Choose another label or show the complete list.</p>
                        <button
                          class="ui-button ui-button--secondary"
                          type="button"
                          onclick={() => (taskLabelFilter = "")}
                        >
                          Show all labels
                        </button>
                      </div>
                    {:else}
                      <div class="large-empty-state">
                        <CheckSquare2
                          size={32}
                          strokeWidth={1.5}
                          aria-hidden="true"
                        />
                        <h3>Your task list is clear</h3>
                        <p>
                          Create a task here or use the dashboard quick add.
                        </p>
                        <button
                          class="ui-button ui-button--secondary secondary-btn"
                          type="button"
                          onclick={() => openTaskEditor()}
                          >Create your first task</button
                        >
                      </div>
                    {/if}
                  {:else if loadingArchivedTasks}
                    <div class="large-empty-state" role="status">
                      <ArchiveIcon
                        size={32}
                        strokeWidth={1.5}
                        aria-hidden="true"
                      />
                      <h3>Loading archived tasks</h3>
                      <p>Retrieving tasks stored outside the active plan.</p>
                    </div>
                  {:else if archivedTasksError}
                    <div class="large-empty-state" role="alert">
                      <ArchiveIcon
                        size={32}
                        strokeWidth={1.5}
                        aria-hidden="true"
                      />
                      <h3>Archive unavailable</h3>
                      <p>{archivedTasksError}</p>
                      <button
                        class="ui-button ui-button--secondary"
                        type="button"
                        onclick={loadArchivedTasks}
                      >
                        Retry
                      </button>
                    </div>
                  {:else if filteredArchivedTasks.length}
                    <section
                      class="task-due-group task-archive-group"
                      data-od-id="archived-tasks"
                    >
                      <header class="task-due-group-heading">
                        <div>
                          <h3>Archived tasks</h3>
                          <span>Newest archived first</span>
                        </div>
                        <strong>{filteredArchivedTasks.length}</strong>
                      </header>
                      <AnimatedList
                        items={filteredArchivedTasks}
                        getKey={(task) => task.id}
                        showGradients={false}
                        enableArrowNavigation={false}
                        displayScrollbar={false}
                        class="tasks-animated-list"
                      >
                        {#snippet children(task)}
                          {@render taskRow(task, true)}
                        {/snippet}
                      </AnimatedList>
                    </section>
                  {:else if archivedTasks.length && taskLabelFilter}
                    <div class="large-empty-state">
                      <Tag size={32} strokeWidth={1.5} aria-hidden="true" />
                      <h3>No archived tasks labelled {taskLabelFilter}</h3>
                      <p>Choose another label or show the complete archive.</p>
                      <button
                        class="ui-button ui-button--secondary"
                        type="button"
                        onclick={() => (taskLabelFilter = "")}
                      >
                        Show all labels
                      </button>
                    </div>
                  {:else}
                    <div class="large-empty-state">
                      <ArchiveIcon
                        size={32}
                        strokeWidth={1.5}
                        aria-hidden="true"
                      />
                      <h3>No archived tasks</h3>
                      <p>
                        Archived tasks will appear here until restored or
                        deleted.
                      </p>
                      <button
                        class="ui-button ui-button--secondary"
                        type="button"
                        onclick={() => selectTaskView("active")}
                      >
                        Return to active tasks
                      </button>
                    </div>
                  {/if}
                </section>
                <aside
                  class="tasks-summary-box tasks-focus-panel"
                  data-od-id="tasks-focus-timer"
                >
                    <div class="focus-timer-heading">
                      <span>[ FOCUS.MODE ]</span>
                      <small>{focusRunning ? "RUNNING" : "READY"}</small>
                    </div>
                    <label for="focus-subject">Focus target</label>
                    <input
                      id="focus-subject"
                      class="text-input focus-subject-input"
                      bind:value={focusSubject}
                      placeholder="What needs your attention?"
                      maxlength="120"
                    />
                    <time
                      class="focus-timer-readout"
                      datetime={`PT${focusRemainingSeconds}S`}
                      aria-live="polite">{focusTimeLabel}</time
                    >
                    <div
                      class="focus-timer-track"
                      aria-hidden="true"
                      style:--focus-progress={`${focusProgress}%`}
                    >
                      <i></i>
                    </div>
                    <div
                      class="focus-duration-options"
                      aria-label="Focus length"
                    >
                      {#each focusDurations as minutes (minutes)}
                        <button
                          type="button"
                          class:active={focusDurationMinutes === minutes}
                          aria-pressed={focusDurationMinutes === minutes}
                          disabled={focusRunning}
                          onclick={() => setFocusDuration(minutes)}
                          >{minutes}m</button
                        >
                      {/each}
                      <label class="focus-custom-duration">
                        <span class="sr-only">Custom focus duration</span>
                        <input
                          type="number"
                          min="1"
                          max="240"
                          step="1"
                          value={focusDurationMinutes}
                          style:--focus-duration-digits={Math.max(
                            2,
                            String(focusDurationMinutes).length,
                          )}
                          disabled={focusRunning}
                          aria-label="Custom focus duration in minutes"
                          oninput={(event) =>
                            setFocusDuration(event.currentTarget.valueAsNumber)}
                        />
                        <span>min</span>
                      </label>
                    </div>
                    <div class="focus-timer-actions">
                      <button
                        class="ui-button ui-button--primary primary-btn"
                        type="button"
                        onclick={startFocusSession}
                      >
                        <Play size={15} strokeWidth={1.8} aria-hidden="true" />
                        Start focus
                      </button>
                      <button
                        class="ui-button ui-button--secondary secondary-btn"
                        type="button"
                        onclick={resetFocusTimer}
                      >
                        <RotateCcw
                          size={15}
                          strokeWidth={1.8}
                          aria-hidden="true"
                        />
                        Reset
                      </button>
                    </div>
                    <dl>
                      <div>
                        <dt>Open</dt>
                        <dd>{tasks.length - completedCount}</dd>
                      </div>
                      <div>
                        <dt>Completed</dt>
                        <dd>{completedCount}</dd>
                      </div>
                    </dl>
                </aside>
              </div>
            </section>
          {:else if activeSection === "kanban"}
            <KanbanPage section={kanbanSection} viewerId={dashboard.user.id} />
          {:else if activeSection === "calendar"}
            <CalendarPage
              {tasks}
              onEditTask={openCalendarTask}
              onOpenContact={openCalendarContact}
              initialDate={calendarDetailDate}
              onInitialDateHandled={() => (calendarDetailDate = null)}
            />
          {:else if activeSection === "contacts"}
            <ContactsPage
              initialContactId={contactDetailId}
              onInitialContactHandled={() => (contactDetailId = null)}
            />
          {:else if activeSection === "rss"}
            <RssReaderPage />
          {:else if activeSection === "journal"}
            <JournalPage />
          {:else if activeSection === "lines"}
            <LinesPage
              viewerId={dashboard.user.id}
              viewerName={dashboard.settings.display_name}
              viewerRole={dashboard.user.role}
              defaultVisibility={dashboard.settings.lines_default_visibility}
              homeToken={linesHomeToken}
            />
          {:else if activeSection === "walls"}
            <WallsPage
              viewerId={dashboard.user.id}
              viewerRole={dashboard.user.role}
              onwallapplied={handleWallApplied}
            />
          {:else if activeSection === "youtube"}
            <YoutubePage />
          {:else if activeSection === "podcasts"}
            <PodcastsPage viewerRole={dashboard.user.role} />
          {:else if activeSection === "coding"}
            <CodingPage />
          {:else if activeSection === "subscriptions"}
            <SubscriptionsPage />
          {:else if activeEmbeddedPage}
            <EmbeddedPage page={activeEmbeddedPage} />
          {:else if placeholderPage}
            <section
              class="feature-page placeholder-page product-page"
              data-od-id={`${activeSection}-page`}
            >
              <div class="feature-page-intro page-header">
                <TypedHeading
                  text={`$ ${activeSection} --init`}
                  odId={`${activeSection}-heading`}
                />
                <p>{placeholderPage.description}</p>
              </div>
              <div class="placeholder-page-layout">
                <section class="placeholder-primary-panel">
                  <div class="placeholder-page-icon" aria-hidden="true">
                    <ChartCandlestick size={28} strokeWidth={1.5} />
                  </div>
                  <p>{placeholderPage.primaryCopy}</p>
                  <button
                    type="button"
                    onclick={() =>
                      showToast("This page is ready for its next iteration")}
                    >Set up this page</button
                  >
                </section>
                <div class="placeholder-module-grid">
                  {#each placeholderPage.modules as module (module)}
                    <section>
                      <span>{module}</span>
                      <div class="placeholder-lines" aria-hidden="true">
                        <i></i><i></i><i></i>
                      </div>
                    </section>
                  {/each}
                </div>
              </div>
            </section>
          {/if}
        </div>
      {/key}
    </main>
  </div>

  <!--
    The player lives in the shell, outside the `{#if activeSection}` chain above.
    An audio element owned by PodcastsPage.svelte would be destroyed the moment
    someone navigated to another section, cutting playback off mid-sentence.
  -->
  <audio
    bind:this={podcastAudio}
    preload="metadata"
    onplay={() => podcastPlayer.handlePlay()}
    onpause={() => podcastPlayer.handlePause()}
    onloadedmetadata={() => podcastPlayer.handleLoadedMetadata()}
    ondurationchange={() => podcastPlayer.handleDurationChange()}
    ontimeupdate={() => podcastPlayer.handleTimeUpdate()}
    onwaiting={() => podcastPlayer.handleWaiting()}
    onplaying={() => podcastPlayer.handlePlaying()}
    onerror={() => podcastPlayer.handleError()}
    onended={() => podcastPlayer.handleEnded()}
  ></audio>

  {#if podcastPlayer.episode}
    <div class="podcast-player-bar" data-od-id="podcast-player">
      <div class="podcast-player-transport">
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          data-tip="Previous episode"
          aria-label={podcastPlayer.hasPrevious
            ? "Play the previous episode"
            : "Restart this episode"}
          onclick={() => podcastPlayer.playPrevious()}
        >
          <SkipBack size={16} strokeWidth={1.9} />
        </button>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          data-tip={`Back ${SKIP_BACK_SECONDS}s`}
          aria-label={`Skip back ${SKIP_BACK_SECONDS} seconds`}
          onclick={() => podcastPlayer.skip(-SKIP_BACK_SECONDS)}
        >
          <RotateCcw size={16} strokeWidth={1.9} />
        </button>
        <button
          class="ui-button ui-button--primary ui-button--icon"
          type="button"
          data-tip={podcastPlayer.playing ? "Pause" : "Play"}
          aria-label={podcastPlayer.playing ? "Pause episode" : "Play episode"}
          onclick={() => podcastPlayer.toggle()}
        >
          {#if podcastPlayer.playing}
            <Pause size={17} strokeWidth={2} />
          {:else}
            <Play size={17} strokeWidth={2} />
          {/if}
        </button>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          data-tip={`Forward ${SKIP_FORWARD_SECONDS}s`}
          aria-label={`Skip forward ${SKIP_FORWARD_SECONDS} seconds`}
          onclick={() => podcastPlayer.skip(SKIP_FORWARD_SECONDS)}
        >
          <RotateCw size={16} strokeWidth={1.9} />
        </button>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          data-tip="Next episode"
          aria-label="Play the next queued episode"
          disabled={!podcastPlayer.hasNext}
          onclick={() => podcastPlayer.playNext()}
        >
          <SkipForward size={16} strokeWidth={1.9} />
        </button>
      </div>

      <div class="podcast-player-meta">
        <strong>{podcastPlayer.episode.title}</strong>
        <small>{podcastPlayer.episode.podcast_title}</small>
      </div>

      <label class="podcast-player-scrub">
        <span class="podcast-player-time"
          >{formatPlaybackTime(podcastPlayer.currentTime)}</span
        >
        <input
          type="range"
          min="0"
          max={Math.max(podcastPlayer.duration, 1)}
          step="1"
          value={podcastPlayer.currentTime}
          aria-label="Seek within the episode"
          oninput={(event) =>
            podcastPlayer.seek(Number(event.currentTarget.value))}
        />
        <span class="podcast-player-time"
          >{formatPlaybackTime(podcastPlayer.duration)}</span
        >
      </label>

      <div class="podcast-player-volume" bind:this={podcastVolumeControl}>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          data-tip="Volume"
          aria-label={`Volume, ${Math.round(podcastPlayer.effectiveVolume * 100)} percent`}
          aria-expanded={podcastVolumeOpen}
          aria-controls="podcast-volume-panel"
          onclick={() => (podcastVolumeOpen = !podcastVolumeOpen)}
        >
          {#if podcastPlayer.effectiveVolume === 0}
            <VolumeOff size={16} strokeWidth={1.9} />
          {:else if podcastPlayer.effectiveVolume < 0.5}
            <Volume1 size={16} strokeWidth={1.9} />
          {:else}
            <Volume2 size={16} strokeWidth={1.9} />
          {/if}
        </button>
        {#if podcastVolumeOpen}
          <div class="podcast-player-volume-panel" id="podcast-volume-panel">
            <button
              class="ui-button ui-button--ghost ui-button--icon"
              type="button"
              aria-pressed={podcastPlayer.muted}
              aria-label={podcastPlayer.muted ? "Unmute" : "Mute"}
              onclick={() => podcastPlayer.toggleMuted()}
            >
              {#if podcastPlayer.effectiveVolume === 0}
                <VolumeOff size={15} strokeWidth={1.9} />
              {:else}
                <Volume2 size={15} strokeWidth={1.9} />
              {/if}
            </button>
            <input
              type="range"
              list="podcast-volume-marks"
              min="0"
              max={MAX_PLAYBACK_VOLUME}
              step="0.05"
              value={podcastPlayer.effectiveVolume}
              aria-label="Playback volume"
              aria-valuetext={`${Math.round(podcastPlayer.effectiveVolume * 100)} percent`}
              oninput={(event) =>
                podcastPlayer.setVolume(Number(event.currentTarget.value))}
            />
            <datalist id="podcast-volume-marks">
              <option value="1" label="100%"></option>
            </datalist>
            <span
              class="podcast-player-time"
              class:is-boosted={podcastPlayer.effectiveVolume > 1}
            >
              {Math.round(podcastPlayer.effectiveVolume * 100)}%
            </span>
          </div>
        {/if}
      </div>

      <label class="podcast-player-rate" data-tip="Playback speed">
        <span class="visually-hidden-label">Playback speed</span>
        <select
          value={podcastPlayer.playbackRate}
          onchange={(event) =>
            savePlaybackRate(Number(event.currentTarget.value))}
        >
          {#each [0.75, 1, 1.25, 1.5, 1.75, 2] as rate (rate)}
            <option value={rate}>{rate}&#215;</option>
          {/each}
        </select>
      </label>

      <button
        class="ui-button ui-button--ghost ui-button--icon podcast-player-close"
        type="button"
        data-tip="Close player"
        aria-label="Close the player"
        onclick={() => podcastPlayer.close()}
      >
        <X size={16} strokeWidth={1.9} />
      </button>
    </div>
  {/if}

  <div
    class={["toast", toastMessage && "show"]}
    role="status"
    aria-live="polite"
  >
    {toastMessage}
  </div>

  <dialog
    class={["focus-session-dialog", focusLeaving && "is-leaving"]}
    {@attach captureFocusDialog}
    aria-labelledby="focus-session-target"
    oncancel={(event) => {
      event.preventDefault();
      if (focusSettingsOpen) {
        focusSettingsOpen = false;
      } else {
        endFocusSession();
      }
    }}
    data-od-id="focus-session-overlay"
  >
    <div class="focus-session-shell">
      <div class="focus-session-burst" aria-hidden="true">
        <PrismaticBurst
          intensity={burstIntensity}
          speed={burstSpeed}
          animationType="hover"
          colors={["#07140f", "#47dba2", "#9af7d6", "#395fff", "#c87dff"]}
          distort={burstDistort}
          paused={burstPaused}
          hoverDampness={burstHoverDampness}
          rayCount={burstRayCount}
          mixBlendMode="screen"
        />
      </div>
      <header class="focus-session-header">
        <div>
          <span>[ FOCUS.SESSION ]</span>
          <small>{focusSessionStatus}</small>
        </div>
        <div class="focus-session-header-actions">
          <button
            type="button"
            aria-label={focusSettingsOpen
              ? "Close focus visual settings"
              : "Open focus visual settings"}
            aria-controls="focus-visual-settings"
            aria-expanded={focusSettingsOpen}
            onclick={() => (focusSettingsOpen = !focusSettingsOpen)}
            data-od-id="focus-visual-settings-toggle"
          >
            <Settings size={18} strokeWidth={1.8} aria-hidden="true" />
          </button>
          <button
            type="button"
            aria-label="End focus session"
            onclick={endFocusSession}
          >
            <X size={18} strokeWidth={1.8} aria-hidden="true" />
          </button>
        </div>
      </header>

      <main class="focus-session-content">
        <p>Focus target</p>
        <h2 id="focus-session-target">{focusSubject}</h2>
        <time
          datetime={`PT${focusRemainingSeconds}S`}
          aria-live="polite"
          aria-atomic="true">{focusTimeLabel}</time
        >
        <div
          class="focus-session-progress"
          aria-label={`${Math.round(focusProgress)} percent complete`}
          role="progressbar"
          aria-valuemin="0"
          aria-valuemax="100"
          aria-valuenow={Math.round(focusProgress)}
          style:--focus-progress={`${focusProgress}%`}
        >
          <i></i>
        </div>
        <span>{focusDurationMinutes} minute session</span>
      </main>

      {#if focusSettingsOpen}
        <section
          id="focus-visual-settings"
          class="focus-settings-panel"
          aria-label="Focus visual settings"
          data-od-id="focus-visual-settings"
        >
          <header class="focus-settings-heading">
            <div>
              <span>[ VISUAL.SETTINGS ]</span>
              <strong>Focus atmosphere</strong>
            </div>
            <button type="button" onclick={resetBurstControls}>Reset</button>
          </header>

          <div class="focus-settings-grid">
            <label>
              <span>Intensity <output>{burstIntensity.toFixed(1)}</output></span>
              <input
                type="range"
                min="0.5"
                max="4"
                step="0.1"
                bind:value={burstIntensity}
              />
            </label>

            <label>
              <span>Speed <output>{burstSpeed.toFixed(2)}</output></span>
              <input
                type="range"
                min="0"
                max="1.5"
                step="0.05"
                bind:value={burstSpeed}
              />
            </label>

            <label>
              <span>Distort <output>{burstDistort.toFixed(1)}</output></span>
              <input
                type="range"
                min="0"
                max="10"
                step="0.1"
                bind:value={burstDistort}
              />
            </label>

            <label>
              <span
                >Hover dampness
                <output>{burstHoverDampness.toFixed(2)}</output></span
              >
              <input
                type="range"
                min="0"
                max="1"
                step="0.05"
                bind:value={burstHoverDampness}
              />
            </label>

            <label>
              <span>Ray count <output>{burstRayCount}</output></span>
              <input
                type="range"
                min="0"
                max="48"
                step="1"
                bind:value={burstRayCount}
              />
            </label>

            <button
              class="ui-toggle-button focus-burst-pause"
              type="button"
              aria-pressed={burstPaused}
              onclick={() => (burstPaused = !burstPaused)}
            >
              <span class="ui-toggle-indicator" aria-hidden="true"
                >{#if burstPaused}<Check size={13} />{/if}</span
              >
              <span>Pause atmosphere</span>
            </button>
          </div>
        </section>
      {/if}

      <footer class="focus-session-footer">
        <span>Esc ends session</span>
        <div>
          <button
            class="ui-button ui-button--primary focus-session-primary"
            type="button"
            onclick={toggleFocusTimer}
          >
            {#if focusRemainingSeconds <= 0}
              <RotateCcw size={17} strokeWidth={1.8} aria-hidden="true" />
              Restart
            {:else if focusRunning}
              <Pause size={17} strokeWidth={1.8} aria-hidden="true" />
              Pause
            {:else}
              <Play size={17} strokeWidth={1.8} aria-hidden="true" />
              Resume
            {/if}
          </button>
          <button
            class="ui-button ui-button--secondary focus-session-secondary"
            type="button"
            onclick={endFocusSession}
          >
            End session
          </button>
        </div>
      </footer>
    </div>
  </dialog>

  <dialog
    class="command-dialog"
    {@attach captureCommandDialog}
    onclose={() => {
      commandQuery = "";
      commandIndex = 0;
    }}
    onclick={(event) => event.target === commandDialog && commandDialog.close()}
    data-od-id="command-dialog"
  >
    <div class="dialog-head">
      <Search size={19} strokeWidth={1.8} aria-hidden="true" />
      <label class="sr-only" for="command-search">Filter pages</label>
      <input
        id="command-search"
        class="command-search-input"
        type="search"
        placeholder="Type a page or command..."
        autocomplete="off"
        spellcheck="false"
        bind:value={commandQuery}
        oninput={() => (commandIndex = 0)}
        onkeydown={handleCommandSearchKeydown}
        {@attach captureCommandSearchInput}
        data-od-id="command-search-input"
      />
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        aria-label="Close command menu"
        onclick={() => commandDialog?.close()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </div>
    <div class="command-list">
      {#if !hasLocalCommandMatches}
        <div class="command-empty" role="status">
          <span>[ NO MATCHES ]</span>
          <p>
            Nothing on this instance matches. Try Dashboard, Tasks, RSS, or
            another page name — or search the web below.
          </p>
        </div>
      {/if}
      {#each commandGroups as entry (entry.group)}
        <div class="command-group">
          <p class="command-group-label">[ {entry.group} ]</p>
          {#if entry.group === "WEB"}
            <label class="sr-only" for="command-search-engine"
              >Search engine</label
            >
            <select
              id="command-search-engine"
              class="command-group-engine"
              value={searchEngine}
              onchange={selectSearchEngine}
              aria-label="Search engine"
            >
              {#each searchEngines as engine (engine.id)}
                <option value={engine.id}>{engine.label}</option>
              {/each}
            </select>
          {/if}
        </div>
        {#each entry.items as item (item.id)}
          <button
            class="command-option"
            class:is-active={commandResults[commandIndex]?.id === item.id}
            type="button"
            onclick={() => runCommand(item)}
            onmouseenter={() =>
              (commandIndex = commandResults.findIndex(
                (result) => result.id === item.id,
              ))}
            ><span>{item.label}</span><span class="keycap">{item.hint}</span
            ></button
          >
        {/each}
      {/each}
    </div>
  </dialog>

  <dialog
    class="settings-dialog task-editor-dialog"
    {@attach captureTaskEditorDialog}
    onclose={resetTaskEditor}
    onclick={(event) =>
      event.target === taskEditorDialog && taskEditorDialog.close()}
    data-od-id="task-editor-dialog"
  >
    <div class="settings-heading task-editor-heading">
      <div>
        <span>[ TASK.EDITOR ]</span>
        <h2>{editingTaskId ? "Edit task" : "New task"}</h2>
        <p>
          {editingTaskId
            ? "Update the task details and schedule."
            : "Capture the work, then add only the structure it needs."}
        </p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        type="button"
        aria-label="Close task editor"
        onclick={() => taskEditorDialog?.close()}
      >
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>

    <form
      class="task-editor-form"
      onsubmit={saveTask}
      data-od-id="task-editor-form"
    >
      <div class="task-editor-scroll">
        <div class="task-field task-field-wide">
          <label for="task-name">Name</label>
          <input
            id="task-name"
            class="text-input"
            bind:value={taskName}
            maxlength="180"
            placeholder="What needs to be done?"
            required
          />
        </div>

        <div class="task-field task-field-wide">
          <label for="task-description">Description</label>
          <textarea
            id="task-description"
            class="text-input task-description-input"
            bind:value={taskDescription}
            maxlength="4000"
            rows="4"
            placeholder="Add context, links, or a clear definition of done."
          ></textarea>
        </div>

        <div class="task-field">
          <label for="task-priority">Priority</label>
          <select
            id="task-priority"
            class="select-input"
            bind:value={taskPriority}
          >
            <option value="none">No priority</option>
            <option value="p1">P1 — Urgent</option>
            <option value="p2">P2 — High</option>
            <option value="p3">P3 — Medium</option>
            <option value="p4">P4 — Low</option>
          </select>
        </div>

        <div class="task-field">
          <label for="task-due-date">Due date</label>
          <div class="input-with-icon">
            <CalendarDays size={16} strokeWidth={1.8} aria-hidden="true" />
            <input
              id="task-due-date"
              class="text-input"
              type="date"
              bind:value={taskDueDate}
            />
          </div>
        </div>

        <div class="task-field task-field-wide">
          <label for="task-labels">Labels</label>
          <div class="input-with-icon">
            <Tag size={16} strokeWidth={1.8} aria-hidden="true" />
            <input
              id="task-labels"
              class="text-input"
              bind:value={taskLabels}
              placeholder="design, planning, personal"
            />
          </div>
          <small>Separate labels with commas. Up to 12 labels.</small>
        </div>

        <fieldset class="task-fieldset task-field-wide">
          <legend>
            <Repeat2 size={16} strokeWidth={1.8} aria-hidden="true" />
            Repeat
          </legend>
          <div class="task-repeat-grid">
            <div class="task-field">
              <label for="task-repeat-rule">Schedule</label>
              <select
                id="task-repeat-rule"
                class="select-input"
                bind:value={taskRepeatRule}
              >
                <option value="none">Does not repeat</option>
                <option value="daily">Daily</option>
                <option value="weekly">Weekly</option>
                <option value="monthly">Monthly</option>
                <option value="yearly">Yearly</option>
                <option value="custom">Custom</option>
              </select>
            </div>
            {#if taskRepeatRule === "custom"}
              <div class="task-field">
                <label for="task-repeat-interval">Every</label>
                <div class="repeat-interval-row">
                  <input
                    id="task-repeat-interval"
                    class="text-input"
                    type="number"
                    min="1"
                    max="365"
                    bind:value={taskRepeatInterval}
                  />
                  <select
                    class="select-input"
                    bind:value={taskRepeatUnit}
                    aria-label="Repeat unit"
                  >
                    <option value="days">Days</option>
                    <option value="weeks">Weeks</option>
                    <option value="months">Months</option>
                    <option value="years">Years</option>
                  </select>
                </div>
              </div>
            {/if}
            {#if taskRepeatRule !== "none"}
              <div class="task-field task-repeat-basis">
                <label for="task-reschedule-from">Reschedule from</label>
                <select
                  id="task-reschedule-from"
                  class="select-input"
                  bind:value={taskRescheduleFrom}
                >
                  <option value="due_date">Previous due date</option>
                  <option value="completion_date">Completion date</option>
                </select>
              </div>
            {/if}
          </div>
        </fieldset>

        <fieldset class="task-fieldset task-field-wide">
          <legend>
            <CheckSquare2 size={16} strokeWidth={1.8} aria-hidden="true" />
            Subtasks
          </legend>
          <div class="subtask-editor-list">
            {#each taskSubtasks as subtask, index (subtask.id ?? index)}
              <div class="subtask-editor-row">
                <button
                  class="ui-toggle-button subtask-editor-toggle"
                  type="button"
                  aria-pressed={subtask.completed}
                  aria-label={subtask.completed
                    ? `Mark subtask ${index + 1} incomplete`
                    : `Mark subtask ${index + 1} complete`}
                  onclick={() => (subtask.completed = !subtask.completed)}
                ><span class="ui-toggle-indicator" aria-hidden="true">{#if subtask.completed}<Check size={13} />{/if}</span></button>
                <input
                  class="text-input"
                  bind:value={subtask.title}
                  maxlength="180"
                  placeholder={`Subtask ${index + 1}`}
                />
                <button
                  class="ui-button ui-button--danger ui-button--icon icon-button"
                  type="button"
                  aria-label={`Remove subtask ${index + 1}`}
                  onclick={() => removeSubtaskDraft(index)}
                >
                  <X size={16} strokeWidth={1.8} aria-hidden="true" />
                </button>
              </div>
            {:else}
              <p class="task-field-empty">No subtasks added.</p>
            {/each}
          </div>
          <button
            class="ui-button ui-button--secondary task-inline-action"
            type="button"
            onclick={addSubtaskDraft}
          >
            <Plus size={15} strokeWidth={1.8} aria-hidden="true" />
            Add subtask
          </button>
        </fieldset>

        <fieldset class="task-fieldset task-field-wide">
          <legend>
            <Paperclip size={16} strokeWidth={1.8} aria-hidden="true" />
            Attachments
          </legend>
          {#if taskAttachments.length}
            <div class="task-attachment-list">
              {#each taskAttachments as attachment (attachment.id)}
                <div class="task-attachment-row">
                  <button
                    class="task-attachment-download"
                    type="button"
                    onclick={() => downloadTaskAttachment(attachment)}
                  >
                    <span>{attachment.file_name}</span>
                    <small
                      >{Math.max(1, Math.round(attachment.byte_size / 1024))}
                      KB</small
                    >
                  </button>
                  <button
                    class="ui-button ui-button--danger ui-button--icon icon-button"
                    type="button"
                    aria-label={`Delete ${attachment.file_name}`}
                    onclick={() => removeTaskAttachment(attachment)}
                  >
                    <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
                  </button>
                </div>
              {/each}
            </div>
          {/if}
          <label class="task-file-picker" for="task-attachments">
            <Paperclip size={16} strokeWidth={1.8} aria-hidden="true" />
            <span>
              {pendingTaskFiles.length
                ? `${pendingTaskFiles.length} file${
                    pendingTaskFiles.length === 1 ? "" : "s"
                  } selected`
                : "Choose files"}
            </span>
            <input
              id="task-attachments"
              type="file"
              multiple
              onchange={selectTaskAttachments}
            />
          </label>
          <small>Files are private to your account. Maximum 10 MB each.</small>
        </fieldset>

        {#if taskEditorError}
          <p class="form-error task-field-wide" role="alert">
            {taskEditorError}
          </p>
        {/if}
      </div>

      <div class="task-editor-actions task-field-wide">
        {#if editingTaskId}
          <button
            class="ui-button ui-button--danger task-delete-button"
            type="button"
            disabled={savingTask}
            onclick={removeCurrentTask}
          >
            <Trash2 size={16} strokeWidth={1.8} aria-hidden="true" />
            Delete task
          </button>
        {/if}
        <button
          class="ui-button ui-button--primary primary-btn"
          type="submit"
          disabled={savingTask}
          data-od-id="save-task"
        >
          {savingTask
            ? "Saving…"
            : editingTaskId
              ? "Save changes"
              : "Create task"}
        </button>
      </div>
    </form>
  </dialog>

  <dialog
    class="settings-dialog widget-library-dialog"
    {@attach captureWidgetLibraryDialog}
    onclick={(event) =>
      event.target === widgetLibraryDialog && widgetLibraryDialog.close()}
    data-od-id="widget-library-dialog"
  >
    <div class="settings-heading">
      <div>
        <h2>Add a widget</h2>
        <p>New widgets are added to your dashboard.</p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        aria-label="Close widget library"
        onclick={() => widgetLibraryDialog?.close()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </div>
    <div class="widget-library-grid">
      {#each widgetCatalog as item (item.kind)}
        <button
          class="widget-library-item"
          type="button"
          disabled={addingWidgetKind !== "" || savingLayout}
          onclick={() => addWidget(item.kind, item.size)}
          data-od-id={`add-widget-${item.kind}`}
        >
          <span>
            <strong>{item.title}</strong>
            <small>{item.description}</small>
          </span>
          <span class="data-note">
            {addingWidgetKind === item.kind ? "Adding…" : item.size}
          </span>
        </button>
      {/each}
    </div>
  </dialog>

  <dialog
    class="settings-dialog profile-settings-dialog"
    {@attach captureSettingsDialog}
    onclose={clearUserSettingsDrafts}
    onclick={(event) =>
      event.target === settingsDialog && settingsDialog.close()}
    data-od-id="user-settings-dialog"
  >
    <div class="settings-heading">
      <div>
        <h2>Account settings</h2>
        <p>
          {dashboard.user.role === "administrator" ? "Administrator" : "Member"} /
          {dashboard.user.email}
        </p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        aria-label="Close account settings"
        onclick={() => settingsDialog?.close()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </div>

    <form
      class="settings-form"
      onsubmit={saveSettings}
      data-od-id="user-settings-form"
    >
      <div
        class="settings-form-scroll"
        {@attach captureSettingsScrollContainer}
      >
        <div class="profile-avatar-editor" data-od-id="avatar-settings">
          <span class="settings-avatar" aria-hidden="true">
            {#if avatarPreviewSource()}
              <img
                src={avatarPreviewSource()}
                alt=""
                onerror={() => (avatarAvailable = false)}
              />
            {:else}
              {profileInitials}
            {/if}
          </span>
          <div class="profile-avatar-copy">
            <strong>Profile image</strong>
            <span>JPEG, PNG, WebP, or AVIF up to 10 MB.</span>
          </div>
          <div class="profile-avatar-actions">
            <label
              class="ui-button ui-button--secondary secondary-btn avatar-upload"
            >
              Choose image
              <input
                type="file"
                accept="image/jpeg,image/png,image/webp,image/avif"
                onchange={selectAvatar}
                data-od-id="choose-user-avatar"
              />
            </label>
            <button
              class="ui-button ui-button--danger background-reset"
              type="button"
              onclick={resetAvatar}
              data-od-id="remove-user-avatar">Remove</button
            >
          </div>
        </div>

        <section
          class="profile-wallpaper-editor"
          aria-labelledby="session-wallpaper-heading"
          data-od-id="session-wallpaper-settings"
        >
          <div class="profile-wallpaper-heading">
            <strong id="session-wallpaper-heading">Background</strong>
            <span>JPEG, PNG, WebP, or AVIF up to 30 MB.</span>
          </div>
          <div class="profile-wallpaper-list">
            {#each userWallpaperOptions as option (option.id)}
              <div
                class="profile-wallpaper-row"
                data-od-id={`user-${option.id}-wallpaper-settings`}
              >
                <div
                  class="background-preview appearance-preview profile-wallpaper-preview"
                  style:--background-preview={wallpaperBackground(option.id)}
                  style:--preview-blur="0px"
                  style:--preview-brightness={option.id === "welcome"
                    ? "78%"
                    : "88%"}
                  style:--preview-contrast={option.id === "welcome"
                    ? "108%"
                    : "104%"}
                  style:--preview-saturation={option.id === "welcome"
                    ? "72%"
                    : "82%"}
                  aria-label={`${option.title} preview`}
                  role="img"
                >
                  <span>[ {option.code} ]</span>
                </div>
                <div class="profile-wallpaper-copy">
                  <strong>{option.title}</strong>
                  <p>{option.description}</p>
                  <small>{wallpaperFileLabel(option.id)}</small>
                </div>
                <div class="profile-wallpaper-actions">
                  <label
                    class="ui-button ui-button--secondary secondary-btn background-upload"
                  >
                    Choose image
                    <input
                      type="file"
                      accept="image/jpeg,image/png,image/webp,image/avif"
                      onchange={(event) => selectWallpaper(option.id, event)}
                      data-od-id={`choose-${option.id}-wallpaper`}
                    />
                  </label>
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    onclick={openWallsFromSettings}
                    data-od-id={`browse-walls-${option.id}`}
                    >Browse Walls</button
                  >
                  <button
                    class="ui-button ui-button--danger background-reset"
                    type="button"
                    onclick={() => resetWallpaper(option.id)}
                    data-od-id={`reset-${option.id}-wallpaper`}
                    >Use default</button
                  >
                </div>
              </div>
            {/each}
          </div>
        </section>

        <label for="settings-name">Display name</label>
        <input
          id="settings-name"
          class="text-input"
          bind:value={settingsDisplayName}
          maxlength="60"
          required
        />

        <label for="settings-location">Weather location</label>
        <input
          id="settings-location"
          class="text-input"
          bind:value={settingsLocation}
          maxlength="80"
          required
        />

        <label for="settings-timezone">Timezone</label>
        <input
          id="settings-timezone"
          class="text-input"
          bind:value={settingsTimezone}
          maxlength="80"
          placeholder="Europe/London"
          required
        />

        <label for="settings-temperature">Temperature unit</label>
        <select
          id="settings-temperature"
          class="select-input"
          bind:value={settingsTemperatureUnit}
        >
          <option value="celsius">Celsius</option>
          <option value="fahrenheit">Fahrenheit</option>
        </select>

        <label for="settings-lines-visibility">Lines default visibility</label>
        <select
          id="settings-lines-visibility"
          class="select-input"
          bind:value={settingsLinesDefaultVisibility}
          data-od-id="settings-lines-default-visibility"
        >
          <option value="private">Private — only me</option>
          <option value="public">Instance — all signed-in users</option>
        </select>

        {#if settingsError}
          <p class="form-error" role="alert">{settingsError}</p>
        {/if}

        <button
          class="admin-entry"
          type="button"
          onclick={openAppearance}
          data-od-id="open-dashboard-appearance"
        >
          <span
            ><strong>Appearance</strong><small
              >Background processing and login surface</small
            ></span
          >
          <ImageIcon size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>

        <button
          class="admin-entry"
          type="button"
          onclick={openEmbeddedPagesSettings}
          data-od-id="open-embedded-pages-settings"
        >
          <span
            ><strong>Embedded pages</strong><small
              >Personal links{dashboard.user.role === "administrator"
                ? " and global instance links"
                : " in your sidebar"}</small
            ></span
          >
          <PanelTop size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>

        {#if dashboard.user.role === "administrator"}
          <button
            class="admin-entry"
            type="button"
            onclick={openAdministration}
            data-od-id="open-user-administration"
          >
            <span
              ><strong>User administration</strong><small
                >Roles, access, and account removal</small
              ></span
            >
            <ArrowRight size={18} strokeWidth={1.8} aria-hidden="true" />
          </button>
        {/if}
        <button
          class="admin-entry destructive-entry"
          type="button"
          onclick={openDestructiveActions}
          data-od-id="open-destructive-actions"
        >
          <span
            ><strong>Destructive Actions</strong><small
              >Permanently Remove Complete Areas Of Your Data</small
            ></span
          >
          <Trash2 size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </div>

      <div class="settings-actions">
        <button
          class="ui-button ui-button--ghost sign-out-btn"
          type="button"
          onclick={signOut}>Sign out</button
        >
        <button
          class="ui-button ui-button--primary primary-btn"
          type="submit"
          disabled={savingSettings}
          data-od-id="save-user-settings"
        >
          {savingSettings ? "Saving…" : "Save changes"}
        </button>
      </div>
    </form>
  </dialog>

  {#if embeddedPagesSettingsOpen}
    <EmbeddedPagesSettings
      pages={dashboard.embedded_pages}
      isAdministrator={dashboard.user.role === "administrator"}
      onPagesChange={applyEmbeddedPages}
      onPageDeleted={handleEmbeddedPageDeleted}
      onClose={() => closeEmbeddedPagesSettings()}
      onBack={() => closeEmbeddedPagesSettings(true)}
    />
  {/if}

  <dialog
    class="settings-dialog destructive-dialog"
    {@attach captureDestructiveDialog}
    onclick={(event) =>
      event.target === destructiveDialog && void closeDestructiveActions()}
    data-od-id="destructive-actions-dialog"
  >
    <div class="settings-heading">
      <button
        class="nested-dialog-back"
        type="button"
        aria-label="Go Back To Account Settings"
        onclick={() => closeDestructiveActions(true)}
        data-od-id="back-to-settings-from-destructive-actions"
      >
        <ChevronLeft size={17} strokeWidth={1.8} aria-hidden="true" />
        <span>Settings</span>
      </button>
      <div>
        <h2>Destructive Actions</h2>
        <p>Permanent, Account-Scoped Content Deletion</p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        aria-label="Close Destructive Actions"
        onclick={() => closeDestructiveActions()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </div>

    <div class="destructive-content">
      <div class="destructive-notice">
        <Trash2 size={18} aria-hidden="true" />
        <p>
          These Actions Permanently Delete Only Your Account’s Records. They
          Cannot Be Undone.
        </p>
      </div>
      {#if destructiveError}
        <p class="form-error" role="alert">{destructiveError}</p>
      {/if}
      <div class="destructive-list">
        {#each destructiveContentActions as action (action.scope)}
          <article data-od-id={`delete-${action.scope}-data`}>
            <div>
              <strong>{action.title}</strong>
              <small>{action.description}</small>
            </div>
            <button
              class="ui-button ui-button--danger"
              class:confirm={pendingContentDeletion === action.scope}
              type="button"
              disabled={Boolean(deletingContentScope)}
              onclick={() => removeContentArea(action)}
            >
              {deletingContentScope === action.scope
                ? "Deleting…"
                : pendingContentDeletion === action.scope
                  ? "Confirm Delete"
                  : "Delete All"}
            </button>
          </article>
        {/each}
      </div>
    </div>
  </dialog>

  <dialog
    class="settings-dialog appearance-dialog"
    {@attach captureAppearanceDialog}
    onclose={resetAppearanceDraft}
    onclick={(event) =>
      event.target === appearanceDialog && void closeAppearance()}
    data-od-id="dashboard-appearance-dialog"
  >
    <div class="settings-heading">
      <button
        class="nested-dialog-back"
        type="button"
        aria-label="Go back to account settings"
        onclick={() => void closeAppearance(true)}
        data-od-id="back-to-settings-from-appearance"
      >
        <ChevronLeft size={17} strokeWidth={1.8} aria-hidden="true" />
        <span>Settings</span>
      </button>
      <div>
        <h2>Appearance</h2>
        <p>
          {dashboard.user.role === "administrator"
            ? "Page background processing and the global login wallpaper"
            : "Page background processing"}
        </p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        aria-label="Close appearance settings"
        onclick={() => void closeAppearance()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </div>

    <form
      class="appearance-editor"
      onsubmit={saveAppearance}
      data-od-id="dashboard-appearance-form"
    >
      {#if dashboard.user.role === "administrator"}
        <div class="wallpaper-slot-grid" aria-label="Wallpaper surfaces">
          {#each appearanceWallpaperOptions as option (option.id)}
            <section
              class="wallpaper-slot-card"
              data-od-id={`wallpaper-${option.id}-settings`}
            >
              <div
                class="login-page-preview"
                style:--background-preview={wallpaperBackground(option.id)}
                aria-label={`${option.title} page preview with the selected wallpaper`}
                role="img"
                data-od-id="login-page-image-preview"
              >
                <div class="login-preview-brand" aria-hidden="true">
                  <span>P&gt;</span>
                  <strong>PANDAN</strong>
                </div>
                <div class="login-preview-context" aria-hidden="true">
                  <div>
                    <small>[ PRIVATE WORKSPACE ]</small>
                    <strong>Your private workspace.</strong>
                    <p>Dashboards, tasks, calendars, feeds, and journal.</p>
                  </div>
                </div>
                <div class="login-preview-access" aria-hidden="true">
                  <div class="login-preview-copy">
                    <small>[ ACCOUNT ACCESS ]</small>
                    <strong>Welcome back.</strong>
                    <p>Sign in to return to your dashboard.</p>
                  </div>
                  <div class="login-preview-modes">
                    <span>Sign in</span>
                    <span>Create account</span>
                  </div>
                  <div class="login-preview-form">
                    <span>Email</span>
                    <i></i>
                    <span>Password</span>
                    <i></i>
                    <b>Enter dashboard</b>
                  </div>
                </div>
              </div>
              <div class="wallpaper-slot-copy">
                <strong>{option.title}</strong>
                <p>{option.description}</p>
                <small>Administrator managed · publicly retrievable</small>
              </div>
              <span class="background-file-name">
                {wallpaperFileLabel(option.id)}
              </span>
              <div class="wallpaper-slot-actions">
                <label
                  class="ui-button ui-button--secondary secondary-btn background-upload"
                >
                  Choose image
                  <input
                    type="file"
                    accept="image/jpeg,image/png,image/webp,image/avif"
                    onchange={(event) => selectWallpaper(option.id, event)}
                    data-od-id={`choose-${option.id}-wallpaper`}
                  />
                </label>
                <button
                  class="ui-button ui-button--danger background-reset"
                  type="button"
                  onclick={() => resetWallpaper(option.id)}
                  data-od-id={`reset-${option.id}-wallpaper`}
                  >Use default</button
                >
              </div>
            </section>
          {/each}
        </div>
      {/if}

      <div class="appearance-control-heading">
        <strong>Page background processing</strong>
        <span
          >Applied to the Main background behind authenticated pages.</span
        >
      </div>

      <div class="appearance-controls">
        <label>
          <span><strong>Blur</strong><output>{backgroundBlur}px</output></span>
          <input
            type="range"
            min="0"
            max="24"
            step="1"
            bind:value={backgroundBlur}
          />
        </label>
        <label>
          <span
            ><strong>Brightness</strong><output>{backgroundBrightness}%</output
            ></span
          >
          <input
            type="range"
            min="40"
            max="140"
            step="1"
            bind:value={backgroundBrightness}
          />
        </label>
        <label>
          <span
            ><strong>Contrast</strong><output>{backgroundContrast}%</output
            ></span
          >
          <input
            type="range"
            min="50"
            max="160"
            step="1"
            bind:value={backgroundContrast}
          />
        </label>
        <label>
          <span
            ><strong>Saturation</strong><output>{backgroundSaturation}%</output
            ></span
          >
          <input
            type="range"
            min="0"
            max="180"
            step="1"
            bind:value={backgroundSaturation}
          />
        </label>
      </div>

      {#if appearanceError}
        <p class="form-error" role="alert">{appearanceError}</p>
      {/if}

      <div class="appearance-actions">
        <button
          class="ui-button ui-button--secondary secondary-btn"
          type="button"
          onclick={resetBackgroundFilters}
          data-od-id="reset-background-filters"
        >
          <RotateCcw size={16} strokeWidth={1.8} aria-hidden="true" />
          Reset filters
        </button>
        <button
          class="ui-button ui-button--primary primary-btn"
          type="submit"
          disabled={savingAppearance}
          data-od-id="save-dashboard-appearance"
        >
          {savingAppearance ? "Saving…" : "Save appearance"}
        </button>
      </div>
    </form>
  </dialog>

  <dialog
    class="settings-dialog admin-dialog"
    {@attach captureAdminDialog}
    onclick={(event) => event.target === adminDialog && adminDialog.close()}
    data-od-id="user-administration-dialog"
  >
    <div class="settings-heading">
      <button
        class="nested-dialog-back"
        type="button"
        aria-label="Go back to account settings"
        onclick={() => adminDialog?.close()}
        data-od-id="back-to-settings-from-administration"
      >
        <ChevronLeft size={17} strokeWidth={1.8} aria-hidden="true" />
        <span>Settings</span>
      </button>
      <div>
        <h2>People &amp; access</h2>
        <p>{managedUsers.length} users, {administratorCount} administrators</p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        aria-label="Close user administration"
        onclick={() => adminDialog?.close()}
        ><X size={18} strokeWidth={1.8} aria-hidden="true" /></button
      >
    </div>

    <div class="admin-directory" data-od-id="user-directory">
      <section
        class="authentication-policy"
        aria-labelledby="authentication-policy-title"
        data-od-id="authentication-policy"
      >
        <div class="authentication-policy-heading">
          <div>
            <p class="widget-kicker">[ AUTHENTICATION POLICY ]</p>
            <h3 id="authentication-policy-title">Account access</h3>
          </div>
          <span
            >{authConfig.oidc_enabled ? "OIDC ready" : "OIDC unavailable"}</span
          >
        </div>

        <div class="authentication-policy-row">
          <span>
            <strong id="password-login-label">Password login</strong>
            <small id="password-login-description"
              >Allow existing accounts to sign in with email and password.</small
            >
          </span>
          <button
            class={[
              "authentication-policy-toggle",
              passwordLoginEnabled && "enabled",
            ]}
            type="button"
            role="switch"
            aria-checked={passwordLoginEnabled}
            aria-labelledby="password-login-label"
            aria-describedby="password-login-description"
            disabled={loadingUsers ||
              !authConfig.oidc_enabled ||
              savingAuthenticationSettings}
            onclick={() => (passwordLoginEnabled = !passwordLoginEnabled)}
            data-od-id="password-login-enabled"
          >
            <span class="authentication-policy-toggle-track" aria-hidden="true"
            ></span>
          </button>
        </div>
        <div class="authentication-policy-row">
          <span>
            <strong id="password-registration-label"
              >Password registration</strong
            >
            <small id="password-registration-description"
              >Allow visitors to create password-based accounts.</small
            >
          </span>
          <button
            class={[
              "authentication-policy-toggle",
              passwordRegistrationEnabled && "enabled",
            ]}
            type="button"
            role="switch"
            aria-checked={passwordRegistrationEnabled}
            aria-labelledby="password-registration-label"
            aria-describedby="password-registration-description"
            disabled={loadingUsers || savingAuthenticationSettings}
            onclick={() =>
              (passwordRegistrationEnabled = !passwordRegistrationEnabled)}
            data-od-id="password-registration-enabled"
          >
            <span class="authentication-policy-toggle-track" aria-hidden="true"
            ></span>
          </button>
        </div>
        <div class="authentication-policy-row">
          <span>
            <strong id="oidc-registration-label">OIDC registration</strong>
            <small id="oidc-registration-description"
              >Allow verified OIDC identities to create new accounts. Existing
              users can still sign in.</small
            >
          </span>
          <button
            class={[
              "authentication-policy-toggle",
              oidcRegistrationEnabled && "enabled",
            ]}
            type="button"
            role="switch"
            aria-checked={oidcRegistrationEnabled}
            aria-labelledby="oidc-registration-label"
            aria-describedby="oidc-registration-description"
            disabled={loadingUsers ||
              !authConfig.oidc_enabled ||
              savingAuthenticationSettings}
            onclick={() => (oidcRegistrationEnabled = !oidcRegistrationEnabled)}
            data-od-id="oidc-registration-enabled"
          >
            <span class="authentication-policy-toggle-track" aria-hidden="true"
            ></span>
          </button>
        </div>
        {#if !authConfig.oidc_enabled}
          <p class="authentication-policy-help">
            Configure OIDC before disabling password login or changing OIDC
            registration.
          </p>
        {/if}
        <div class="authentication-policy-actions">
          <button
            class="ui-button ui-button--primary"
            type="button"
            disabled={loadingUsers || savingAuthenticationSettings}
            onclick={() => void saveAuthenticationSettings()}
            data-od-id="save-authentication-settings"
          >
            {savingAuthenticationSettings ? "Saving…" : "Save access settings"}
          </button>
        </div>
      </section>

      <div class="admin-directory-note">
        <p>
          Role changes apply immediately. Your own administrator account is
          locked here to protect access.
        </p>
      </div>

      {#if adminError}
        <p class="form-error" role="alert">{adminError}</p>
      {/if}

      {#if loadingUsers}
        <div class="admin-loading" role="status">
          <span class="sr-only">Loading user directory…</span>
          {#each [1, 2, 3] as row (row)}
            <span class="admin-loading-row" aria-hidden="true"></span>
          {/each}
        </div>
      {:else}
        <div class="admin-user-list">
          {#each managedUsers as user (user.id)}
            <article
              class="admin-user-row"
              data-od-id={`managed-user-${user.id}`}
            >
              <div class="admin-avatar" aria-hidden="true">
                {memberInitials(user)}
              </div>
              <div class="admin-user-copy">
                <div class="admin-user-name">
                  <strong>{user.display_name}</strong>
                  {#if user.id === dashboard.user.id}<span class="you-badge"
                      >You</span
                    >{/if}
                </div>
                <span>{user.email}</span>
                <small>Joined {memberSince(user.created_at)}</small>
              </div>
              <div class="admin-user-controls">
                <label class="sr-only" for={`role-${user.id}`}
                  >Role for {user.display_name}</label
                >
                <select
                  id={`role-${user.id}`}
                  class="select-input role-select"
                  value={user.role}
                  disabled={user.id === dashboard.user.id ||
                    mutatingUserId !== ""}
                  onchange={(event) => handleRoleChange(event, user)}
                  data-od-id={`user-role-${user.id}`}
                >
                  <option value="member">Member</option>
                  <option value="administrator">Administrator</option>
                </select>
                <button
                  class="ui-button ui-button--danger remove-user-btn"
                  type="button"
                  disabled={user.id === dashboard.user.id ||
                    mutatingUserId !== ""}
                  aria-label={`Remove ${user.display_name}`}
                  onclick={() => (pendingRemovalId = user.id)}
                  data-od-id={`remove-user-${user.id}`}
                >
                  Remove
                </button>
              </div>
              {#if pendingRemovalId === user.id}
                <div
                  class="remove-confirmation"
                  data-od-id={`remove-confirmation-${user.id}`}
                >
                  <p>
                    <strong>Remove {user.display_name}?</strong>
                    <span
                      >Their dashboard, settings, and active sessions will be
                      deleted.</span
                    >
                  </p>
                  <div>
                    <button
                      class="ui-button ui-button--secondary secondary-btn"
                      type="button"
                      onclick={() => (pendingRemovalId = "")}
                      >Keep account</button
                    >
                    <button
                      class="ui-button ui-button--danger remove-user-btn confirm-remove"
                      type="button"
                      disabled={mutatingUserId !== ""}
                      onclick={() => removeManagedUser(user)}
                    >
                      {mutatingUserId === user.id
                        ? "Removing…"
                        : "Confirm removal"}
                    </button>
                  </div>
                </div>
              {/if}
            </article>
          {:else}
            <p class="empty-state roomy">No user accounts found.</p>
          {/each}
        </div>
      {/if}
    </div>
  </dialog>
{/if}

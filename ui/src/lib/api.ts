export interface HealthResponse {
  status: "ok" | "unavailable";
  database: "connected" | "disconnected";
}

export interface Task {
  id: string;
  title: string;
  description: string;
  completed: boolean;
  priority: "p1" | "p2" | "p3" | "p4" | "none";
  due_date: string | null;
  repeat_rule: "none" | "daily" | "weekly" | "monthly" | "yearly" | "custom";
  repeat_interval: number;
  repeat_unit: "days" | "weeks" | "months" | "years";
  reschedule_from: "due_date" | "completion_date";
  completed_at: string | null;
  labels: string[];
  subtasks: TaskSubtask[];
  attachments: TaskAttachment[];
  created_at: string;
  updated_at: string;
}

export interface TaskSubtask {
  id: string;
  title: string;
  completed: boolean;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface TaskAttachment {
  id: string;
  file_name: string;
  mime_type: string;
  byte_size: number;
  created_at: string;
}

export type LineVisibility = "private" | "public";

export interface LinePostAttachment {
  id: string;
  file_name: string;
  mime_type: string;
  byte_size: number;
  created_at: string;
}

export interface LinePostReaction {
  emoji: string;
  count: number;
  reacted_by_viewer: boolean;
}

export interface LinePost {
  id: string;
  user_id: string;
  author_name: string;
  content: string;
  visibility: LineVisibility;
  reply_to_post_id: string | null;
  reply_to_author_name: string | null;
  reply_to_content: string | null;
  tags: string[];
  attachments: LinePostAttachment[];
  reactions: LinePostReaction[];
  reply_count: number;
  created_at: string;
  updated_at: string;
}

export interface LineAuthorProfile {
  user_id: string;
  display_name: string;
  post_count: number;
  first_post_at: string | null;
}

export interface LineAuthorFeed {
  author: LineAuthorProfile;
  posts: LinePost[];
}

export interface LineThread {
  parent: LinePost | null;
  post: LinePost;
  replies: LinePost[];
}

export type KanbanRole = "admin" | "member" | "guest";
export type KanbanSection = "boards" | "workspaces" | "invitations";
export type KanbanLabelColor =
  "accent" | "blue" | "amber" | "red" | "violet" | "gray";

export interface KanbanWorkspace {
  id: string;
  name: string;
  description: string;
  role: KanbanRole;
  member_count: number;
  board_count: number;
  permissions: string[];
  created_at: string;
  updated_at: string;
}
export interface KanbanInvitation {
  workspace_id: string;
  workspace_name: string;
  role: KanbanRole;
  invited_by_name: string;
  created_at: string;
}
export interface KanbanOverview {
  workspaces: KanbanWorkspace[];
  invitations: KanbanInvitation[];
}
export interface KanbanMember {
  user_id: string;
  display_name: string;
  email: string;
  role: KanbanRole;
  status: "invited" | "active";
  created_at: string;
}
export interface KanbanDirectoryUser {
  user_id: string;
  display_name: string;
  email: string;
}
export interface KanbanRolePermission {
  role: KanbanRole;
  permission: string;
  granted: boolean;
}
export interface KanbanMemberPermission {
  user_id: string;
  permission: string;
  granted: boolean;
}
export interface KanbanWorkspaceSettings {
  workspace: KanbanWorkspace;
  members: KanbanMember[];
  role_permissions: KanbanRolePermission[];
  member_overrides: KanbanMemberPermission[];
}
export interface KanbanBoardSummary {
  id: string;
  workspace_id: string;
  name: string;
  description: string;
  visibility: "private" | "public";
  archived: boolean;
  favorite: boolean;
  position: number;
  column_count: number;
  card_count: number;
  created_at: string;
  updated_at: string;
}
export interface KanbanLabel {
  id: string;
  board_id: string;
  name: string;
  color: KanbanLabelColor;
}
export interface KanbanComment {
  id: string;
  card_id: string;
  user_id: string | null;
  author_name: string;
  content: string;
  created_at: string;
  updated_at: string;
}
export interface KanbanChecklistItem {
  id: string;
  checklist_id: string;
  title: string;
  completed: boolean;
  position: number;
}
export interface KanbanChecklist {
  id: string;
  card_id: string;
  name: string;
  position: number;
  items: KanbanChecklistItem[];
}
export interface KanbanAttachment {
  id: string;
  card_id: string;
  file_name: string;
  mime_type: string;
  byte_size: number;
  created_at: string;
}
export interface KanbanActivity {
  id: string;
  card_id: string;
  actor_name: string;
  action: string;
  detail: string;
  created_at: string;
}
export interface KanbanCard {
  id: string;
  column_id: string;
  title: string;
  description: string;
  due_date: string | null;
  position: number;
  assignees: KanbanMember[];
  labels: KanbanLabel[];
  comments: KanbanComment[];
  checklists: KanbanChecklist[];
  attachments: KanbanAttachment[];
  activity: KanbanActivity[];
  created_at: string;
  updated_at: string;
}
export interface KanbanColumn {
  id: string;
  board_id: string;
  name: string;
  position: number;
  cards: KanbanCard[];
  created_at: string;
  updated_at: string;
}
export interface KanbanBoard {
  id: string;
  workspace_id: string;
  name: string;
  description: string;
  visibility: "private" | "public";
  archived: boolean;
  favorite: boolean;
  permissions: string[];
  members: KanbanMember[];
  labels: KanbanLabel[];
  columns: KanbanColumn[];
  created_at: string;
  updated_at: string;
}
export interface KanbanCardInput {
  title: string;
  description?: string;
  due_date?: string | null;
  assignee_ids?: string[];
  label_ids?: string[];
}

export interface TaskInput {
  title: string;
  description?: string;
  priority?: Task["priority"];
  labels?: string[];
  subtasks?: Array<Pick<TaskSubtask, "title" | "completed"> & { id?: string }>;
  due_date?: string | null;
  repeat_rule?: Task["repeat_rule"];
  repeat_interval?: number;
  repeat_unit?: Task["repeat_unit"];
  reschedule_from?: Task["reschedule_from"];
}

export interface FeedItem {
  id: string;
  category: "Design" | "Technology" | "Culture";
  source: string;
  title: string;
  summary: string;
  reading_minutes: number;
  published_at: string;
}

export type RssRetentionMode = "read" | "all";

export interface RssSubscription {
  id: string;
  url: string;
  base_url: string;
  title: string;
  category: string;
  auto_delete_days: number | null;
  auto_delete_mode: RssRetentionMode;
  last_fetched_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface RssReaderItem {
  id: string;
  subscription_id: string;
  source: string;
  category: string;
  base_url: string;
  url: string;
  title: string;
  summary: string;
  published_at: string;
  fetched_at: string;
  read_at: string | null;
  saved_at: string | null;
}

export interface RssReaderResponse {
  subscriptions: RssSubscription[];
  items: RssReaderItem[];
}

export interface RssSubscriptionInput {
  url: string;
  category: string;
  auto_delete_days: number | null;
  auto_delete_mode: RssRetentionMode;
}

export type YoutubeDisplayMode = "thumbnails" | "compact";

export interface YoutubeSubscription {
  channel_id: string;
  title: string;
  channel_url: string;
  thumbnail_url: string;
  last_fetched_at: string | null;
  last_error: string | null;
  created_at: string;
}

export interface YoutubeVideo {
  id: string;
  channel_id: string;
  channel_title: string;
  url: string;
  thumbnail_url: string;
  title: string;
  published_at: string;
  fetched_at: string;
  watch_later_at: string | null;
}

export interface YoutubeGroup {
  id: string;
  name: string;
  position: number;
  channel_ids: string[];
  created_at: string;
  updated_at: string;
}

export interface YoutubeReaderResponse {
  subscriptions: YoutubeSubscription[];
  groups: YoutubeGroup[];
  videos: YoutubeVideo[];
  watch_later: YoutubeVideo[];
  display_mode: YoutubeDisplayMode;
}

export interface JournalNode {
  id: string;
  parent_id: string | null;
  name: string;
  content: string;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface JournalResponse {
  nodes: JournalNode[];
}

export type PodcastRequestStatus =
  "pending" | "approved" | "rejected" | "withdrawn";

export type PodcastDownloadStatus =
  "queued" | "downloading" | "ready" | "failed";

export interface PodcastSummary {
  id: string;
  title: string;
  description: string;
  author: string;
  site_url: string;
  feed_url: string;
  artwork_url: string;
  has_artwork: boolean;
  auto_download_count: number;
  max_retained_episodes: number;
  subscribed: boolean;
  episode_count: number;
  downloaded_count: number;
  latest_published_at: string | null;
  last_fetched_at: string | null;
  last_error: string | null;
  created_at: string;
}

export interface Podcast {
  id: string;
  feed_url: string;
  normalized_url: string;
  title: string;
  description: string;
  author: string;
  site_url: string;
  language: string;
  artwork_url: string;
  has_artwork: boolean;
  auto_download_count: number;
  max_retained_episodes: number;
  added_by: string | null;
  last_fetched_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface PodcastEpisode {
  id: string;
  podcast_id: string;
  podcast_title: string;
  title: string;
  description: string;
  episode_url: string;
  enclosure_type: string;
  enclosure_bytes: number | null;
  duration_seconds: number | null;
  published_at: string;
  download_status: PodcastDownloadStatus | null;
  download_progress: number;
  position_seconds: number;
  completed_at: string | null;
  saved_at: string | null;
  queue_position: number | null;
}

export interface PodcastRequest {
  id: string;
  user_id: string;
  requester_name: string;
  feed_url: string;
  resolved_title: string;
  resolved_author: string;
  resolved_artwork_url: string;
  note: string;
  status: PodcastRequestStatus;
  decision_note: string;
  decided_by_name: string | null;
  decided_at: string | null;
  podcast_id: string | null;
  created_at: string;
  updated_at: string;
}

/** The part of the administrator policy members are allowed to see. */
export interface PodcastPolicy {
  requests_enabled: boolean;
  member_downloads_enabled: boolean;
  max_pending_requests_per_user: number;
}

export interface PodcastOverview {
  podcasts: PodcastSummary[];
  queue: PodcastEpisode[];
  saved: PodcastEpisode[];
  recent: PodcastEpisode[];
  in_progress: PodcastEpisode[];
  requests: PodcastRequest[];
  policy: PodcastPolicy;
}

export interface PodcastAdminSettings {
  requests_enabled: boolean;
  member_downloads_enabled: boolean;
  max_pending_requests_per_user: number;
  storage_budget_bytes: number;
  max_episode_bytes: number;
  default_auto_download_count: number;
  updated_at: string;
  storage_used_bytes: number;
}

export interface PodcastRequestOutcome {
  /** `subscribed` when the feed was already catalogued, so no review was needed. */
  outcome: "requested" | "subscribed";
  request: PodcastRequest | null;
  podcast_id: string | null;
}

export type CalendarColor = `#${string}`;

export interface CalendarSubscription {
  id: string;
  url: string;
  name: string;
  color: CalendarColor;
  last_fetched_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface CalendarEvent {
  id: string;
  subscription_id: string;
  calendar_name: string;
  calendar_color: CalendarColor;
  title: string;
  description: string;
  location: string;
  url: string;
  start_at: string;
  end_at: string | null;
  all_day: boolean;
}

export interface CalendarResponse {
  subscriptions: CalendarSubscription[];
  events: CalendarEvent[];
}

export interface ContactMethod {
  label: string;
  value: string;
}

export interface ContactAddress {
  label: string;
  street: string;
  city: string;
  region: string;
  postal_code: string;
  country: string;
}

export interface ContactImportantDate {
  label: string;
  date: string;
  recurring: boolean;
}

export interface Contact {
  id: string;
  dav_source_id: string | null;
  source_kind: "manual" | "monica" | "carddav";
  source_reference: string | null;
  first_name: string;
  middle_name: string;
  last_name: string;
  nickname: string;
  pronouns: string;
  company: string;
  job_title: string;
  birthday: string | null;
  emails: ContactMethod[];
  phones: ContactMethod[];
  addresses: ContactAddress[];
  important_dates: ContactImportantDate[];
  tags: string[];
  relationship_context: string;
  notes: string;
  favorite: boolean;
  archived: boolean;
  has_photo: boolean;
  created_at: string;
  updated_at: string;
}

export type ContactInput = Omit<
  Contact,
  | "id"
  | "dav_source_id"
  | "source_kind"
  | "source_reference"
  | "has_photo"
  | "created_at"
  | "updated_at"
>;

export interface ContactDavSource {
  id: string;
  name: string;
  url: string;
  username: string;
  has_password: boolean;
  last_synced_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface ContactsResponse {
  contacts: Contact[];
  dav_sources: ContactDavSource[];
  secret_storage_enabled: boolean;
}

export interface ContactImportResult {
  imported: number;
  skipped: number;
  total: number;
}

export interface PaymentSubscription {
  id: string;
  service: string;
  description: string;
  frequency: string;
  amount_micros: number;
  currency: string;
  first_paid_on: string;
  created_at: string;
  updated_at: string;
}

export interface PaymentSubscriptionInput {
  service: string;
  description: string;
  frequency: string;
  amount_micros: number;
  currency: string;
  first_paid_on: string;
}

export type CodingProvider =
  "github" | "gitlab" | "codeberg" | "gitea" | "forgejo";

export interface CodingProject {
  id: string;
  provider: CodingProvider;
  host: string;
  repository: string;
  has_credential: boolean;
  created_at: string;
  updated_at: string;
}

export interface CodingRelease {
  project_id: string;
  version: string;
  url: string;
  published_at: string;
}

export interface CodingMergeRequest {
  id: number;
  reference: string;
  title: string;
  url: string;
  updated_at: string;
  draft: boolean;
  merge_status: string;
}

export interface CodingOwnedRepository {
  provider: CodingProvider;
  host: string;
  repository: string;
  url: string;
  open_pull_requests: number | null;
}

export interface CodingPipeline {
  project_id: string;
  id: number;
  status: string;
  reference: string;
  sha: string;
  url: string;
  updated_at: string;
}

export interface CodingCredential {
  provider: CodingProvider;
  host: string;
  connected: boolean;
}

export interface CodingResponse {
  projects: CodingProject[];
  releases: CodingRelease[];
  merge_requests: CodingMergeRequest[];
  owned_repositories: CodingOwnedRepository[];
  pipelines: CodingPipeline[];
  credentials: CodingCredential[];
  secret_storage_enabled: boolean;
  provider_errors: string[];
}

export interface CreateJournalNodeInput {
  parent_id: string | null;
  name: string;
  content?: string;
}

export interface UpdateJournalNodeInput {
  name?: string;
  content?: string;
  parent_id?: string | null;
  position?: number;
}

export type WidgetKind =
  | "weather"
  | "task-summary"
  | "focus"
  | "task-list"
  | "task-progress"
  | "feed-list"
  | "feed-sources"
  | "youtube"
  | "rss"
  | "reddit"
  | "stocks"
  | "calendar"
  | "clock"
  | "iframe"
  | "html"
  | "releases"
  | "streams"
  | "bible-verse";

export type WidgetSize = "compact" | "standard" | "wide" | "full";

export interface DashboardWidget {
  id: string;
  kind: WidgetKind;
  workspace: number;
  position: number;
  size: WidgetSize;
  grid_x: number;
  grid_y: number;
  grid_w: number;
  grid_h: number;
  config: Record<string, unknown>;
  has_secret: boolean;
  created_at: string;
  updated_at: string;
}

export interface WidgetCapabilities {
  secret_storage_enabled: boolean;
}

export interface WidgetDataItem {
  title: string;
  url?: string;
  comments_url?: string;
  source?: string;
  summary?: string | null;
  thumbnail?: string | null;
  published_at?: string | number;
  score?: number;
  comments?: number;
  symbol?: string;
  value?: number;
  change?: number;
  currency?: string;
  provider?: string;
  version?: string;
  live?: boolean;
  viewers?: number | null;
  category?: string | null;
}

export interface WidgetData {
  items: WidgetDataItem[];
  partial?: boolean;
}

export interface User {
  id: string;
  email: string;
  role: "administrator" | "member";
  created_at: string;
}

export interface UserSettings {
  user_id: string;
  display_name: string;
  location: string;
  timezone: string;
  sidebar_timezones: string[];
  temperature_unit: "celsius" | "fahrenheit";
  lines_default_visibility: LineVisibility;
  podcast_playback_rate: number;
  updated_at: string;
}

export type UserContentScope =
  | "contacts"
  | "tasks"
  | "lines"
  | "calendar"
  | "rss"
  | "journal"
  | "youtube"
  | "podcasts"
  | "coding"
  | "subscriptions";

export interface DeleteUserContentResult {
  scope: UserContentScope;
  deleted: number;
}

export interface UserAppearance {
  user_id: string;
  has_dashboard_wallpaper: boolean;
  has_welcome_wallpaper: boolean;
  has_loading_wallpaper: boolean;
  has_login_wallpaper: boolean;
  background_blur: number;
  background_brightness: number;
  background_contrast: number;
  background_saturation: number;
  updated_at: string;
}

export type WallpaperSlot = "dashboard" | "welcome" | "loading" | "login";

export interface ManagedUser {
  id: string;
  email: string;
  display_name: string;
  role: User["role"];
  created_at: string;
}

export interface AuthResponse {
  user: User;
  settings: UserSettings;
}

export interface OidcConfig {
  enabled: boolean;
  provider_name: string | null;
}

export interface AuthenticationConfig {
  password_login_enabled: boolean;
  password_registration_enabled: boolean;
  oidc_enabled: boolean;
  oidc_registration_enabled: boolean;
  oidc_provider_name: string | null;
}

export interface SetupStatus {
  required: boolean;
}

export interface DashboardResponse {
  user: User;
  settings: UserSettings;
  appearance: UserAppearance;
  tasks: Task[];
  feeds: FeedItem[];
  widgets: DashboardWidget[];
  embedded_pages: EmbeddedPagesResponse;
}

export type EmbeddedPageScope = "global" | "user";

export interface EmbeddedPage {
  id: string;
  scope: EmbeddedPageScope;
  owner_user_id: string | null;
  created_by_user_id: string | null;
  title: string;
  description: string;
  url: string;
  allow_same_origin: boolean;
  iframe_height: number;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface EmbeddedPagesResponse {
  global: EmbeddedPage[];
  personal: EmbeddedPage[];
}

export interface EmbeddedPageInput {
  title: string;
  description: string;
  url: string;
  allow_same_origin: boolean;
  iframe_height: number;
}

interface ApiErrorResponse {
  error?: string;
}

export class ApiError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function requestJson<T>(
  path: string,
  init?: RequestInit,
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<T> {
  const response = await fetcher(path, init);
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }

  return (await response.json()) as T;
}

async function requestEmpty(path: string, init?: RequestInit): Promise<void> {
  const response = await fetch(path, init);
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export async function fetchHealth(
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<HealthResponse> {
  const response = await fetcher("/api/health");
  const payload = (await response.json()) as HealthResponse;

  if (!response.ok) {
    throw new Error(`Health check failed with status ${response.status}`);
  }

  return payload;
}

export function fetchDashboard(
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<DashboardResponse> {
  return requestJson<DashboardResponse>("/api/dashboard", undefined, fetcher);
}

export function fetchEmbeddedPages(): Promise<EmbeddedPagesResponse> {
  return requestJson<EmbeddedPagesResponse>("/api/embedded-pages");
}

export function createPersonalEmbeddedPage(
  input: EmbeddedPageInput,
): Promise<EmbeddedPage> {
  return requestJson<EmbeddedPage>("/api/embedded-pages", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updatePersonalEmbeddedPage(
  pageId: string,
  input: EmbeddedPageInput,
): Promise<EmbeddedPage> {
  return requestJson<EmbeddedPage>(
    `/api/embedded-pages/${encodeURIComponent(pageId)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}

export function deletePersonalEmbeddedPage(pageId: string): Promise<void> {
  return requestEmpty(
    `/api/embedded-pages/${encodeURIComponent(pageId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
}

export function reorderPersonalEmbeddedPages(
  pageIds: string[],
): Promise<EmbeddedPage[]> {
  return requestJson<EmbeddedPage[]>("/api/embedded-pages/order", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ page_ids: pageIds }),
  });
}

export function createGlobalEmbeddedPage(
  input: EmbeddedPageInput,
): Promise<EmbeddedPage> {
  return requestJson<EmbeddedPage>("/api/admin/embedded-pages", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updateGlobalEmbeddedPage(
  pageId: string,
  input: EmbeddedPageInput,
): Promise<EmbeddedPage> {
  return requestJson<EmbeddedPage>(
    `/api/admin/embedded-pages/${encodeURIComponent(pageId)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}

export function deleteGlobalEmbeddedPage(pageId: string): Promise<void> {
  return requestEmpty(
    `/api/admin/embedded-pages/${encodeURIComponent(pageId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
}

export function reorderGlobalEmbeddedPages(
  pageIds: string[],
): Promise<EmbeddedPage[]> {
  return requestJson<EmbeddedPage[]>("/api/admin/embedded-pages/order", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ page_ids: pageIds }),
  });
}

export function createDashboardWidget(input: {
  kind: WidgetKind;
  workspace: number;
  size: WidgetSize;
}): Promise<DashboardWidget> {
  return requestJson<DashboardWidget>("/api/widgets", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updateDashboardWidgetLayout(
  widgets: Array<
    Pick<
      DashboardWidget,
      | "id"
      | "workspace"
      | "position"
      | "size"
      | "grid_x"
      | "grid_y"
      | "grid_w"
      | "grid_h"
    >
  >,
): Promise<DashboardWidget[]> {
  return requestJson<DashboardWidget[]>("/api/widgets/layout", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ widgets }),
  });
}

export function fetchWidgetCapabilities(): Promise<WidgetCapabilities> {
  return requestJson<WidgetCapabilities>("/api/widgets/capabilities");
}

export function updateDashboardWidgetConfig(
  id: string,
  input: {
    config: Record<string, unknown>;
    secret?: string;
    clear_secret?: boolean;
  },
): Promise<DashboardWidget> {
  return requestJson<DashboardWidget>(
    `/api/widgets/${encodeURIComponent(id)}`,
    {
      method: "PUT",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}

export function fetchWidgetData(
  id: string,
  refresh = false,
): Promise<WidgetData> {
  const query = refresh ? "?refresh=true" : "";
  return requestJson<WidgetData>(
    `/api/widgets/${encodeURIComponent(id)}/data${query}`,
  );
}

export async function deleteDashboardWidget(id: string): Promise<void> {
  const response = await fetch(`/api/widgets/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function fetchOidcConfig(
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<OidcConfig> {
  return requestJson<OidcConfig>("/api/auth/oidc/config", undefined, fetcher);
}

export function fetchAuthenticationConfig(
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<AuthenticationConfig> {
  return requestJson<AuthenticationConfig>(
    "/api/auth/config",
    undefined,
    fetcher,
  );
}

export function fetchSetupStatus(
  fetcher: typeof globalThis.fetch = globalThis.fetch,
): Promise<SetupStatus> {
  return requestJson<SetupStatus>("/api/setup", undefined, fetcher);
}

export function createAdministrator(input: {
  email: string;
  password: string;
  display_name: string;
}): Promise<AuthResponse> {
  return requestJson<AuthResponse>("/api/setup", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function registerAccount(input: {
  email: string;
  password: string;
  display_name: string;
}): Promise<AuthResponse> {
  return requestJson<AuthResponse>("/api/auth/register", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function loginAccount(input: {
  email: string;
  password: string;
}): Promise<AuthResponse> {
  return requestJson<AuthResponse>("/api/auth/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export async function logoutAccount(): Promise<void> {
  const response = await fetch("/api/auth/logout", {
    method: "POST",
    credentials: "same-origin",
  });
  if (!response.ok) {
    throw new ApiError("Unable to sign out", response.status);
  }
}

export function updateUserSettings(input: {
  display_name: string;
  location: string;
  timezone: string;
  sidebar_timezones?: string[];
  temperature_unit: UserSettings["temperature_unit"];
  lines_default_visibility: LineVisibility;
  podcast_playback_rate?: number;
}): Promise<UserSettings> {
  return requestJson<UserSettings>("/api/settings", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function deleteUserContent(
  scope: UserContentScope,
): Promise<DeleteUserContentResult> {
  return requestJson<DeleteUserContentResult>(
    `/api/settings/data/${encodeURIComponent(scope)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
}

export async function updateAvatar(file: File): Promise<void> {
  const response = await fetch("/api/settings/avatar", {
    method: "PUT",
    headers: { "content-type": file.type },
    credentials: "same-origin",
    body: file,
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? "Unable to save avatar",
      response.status,
    );
  }
}

export async function deleteAvatar(): Promise<void> {
  const response = await fetch("/api/settings/avatar", {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? "Unable to remove avatar",
      response.status,
    );
  }
}

export function updateAppearance(input: {
  background_blur: number;
  background_brightness: number;
  background_contrast: number;
  background_saturation: number;
}): Promise<UserAppearance> {
  return requestJson<UserAppearance>("/api/settings/appearance", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export async function updateWallpaper(
  slot: WallpaperSlot,
  file: File,
): Promise<void> {
  const response = await fetch(`/api/settings/wallpapers/${slot}`, {
    method: "PUT",
    headers: { "content-type": file.type },
    credentials: "same-origin",
    body: file,
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? "Unable to save wallpaper",
      response.status,
    );
  }
}

export async function deleteWallpaper(slot: WallpaperSlot): Promise<void> {
  const response = await fetch(`/api/settings/wallpapers/${slot}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? "Unable to reset wallpaper",
      response.status,
    );
  }
}

export function fetchManagedUsers(): Promise<ManagedUser[]> {
  return requestJson<ManagedUser[]>("/api/admin/users", {
    credentials: "same-origin",
  });
}

export function fetchAuthenticationSettings(): Promise<AuthenticationConfig> {
  return requestJson<AuthenticationConfig>("/api/admin/authentication", {
    credentials: "same-origin",
  });
}

export function updateAuthenticationSettings(input: {
  password_login_enabled: boolean;
  password_registration_enabled: boolean;
  oidc_registration_enabled: boolean;
}): Promise<AuthenticationConfig> {
  return requestJson<AuthenticationConfig>("/api/admin/authentication", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updateManagedUserRole(
  id: string,
  role: ManagedUser["role"],
): Promise<ManagedUser> {
  return requestJson<ManagedUser>(
    `/api/admin/users/${encodeURIComponent(id)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ role }),
    },
  );
}

export async function deleteManagedUser(id: string): Promise<void> {
  const response = await fetch(`/api/admin/users/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function createTask(input: string | TaskInput): Promise<Task> {
  return requestJson<Task>("/api/tasks", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(typeof input === "string" ? { title: input } : input),
  });
}

export function updateTask(
  id: string,
  input: Partial<TaskInput> & { completed?: boolean },
): Promise<Task> {
  return requestJson<Task>(`/api/tasks/${encodeURIComponent(id)}`, {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(input),
  });
}

export function setTaskCompleted(
  id: string,
  completed: boolean,
): Promise<Task> {
  return updateTask(id, { completed });
}

export async function deleteTask(id: string): Promise<void> {
  const response = await fetch(`/api/tasks/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export async function archiveTask(id: string): Promise<void> {
  const response = await fetch(`/api/tasks/${encodeURIComponent(id)}/archive`, {
    method: "PATCH",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function fetchArchivedTasks(): Promise<Task[]> {
  return requestJson<Task[]>("/api/tasks/archived");
}

export async function restoreTask(id: string): Promise<void> {
  const response = await fetch(`/api/tasks/${encodeURIComponent(id)}/restore`, {
    method: "PATCH",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? "Request failed while restoring task",
      response.status,
    );
  }
}

export function uploadTaskAttachment(
  taskId: string,
  file: File,
): Promise<TaskAttachment> {
  return requestJson<TaskAttachment>(
    `/api/tasks/${encodeURIComponent(taskId)}/attachments?file_name=${encodeURIComponent(file.name)}`,
    {
      method: "POST",
      headers: {
        "content-type": file.type || "application/octet-stream",
      },
      credentials: "same-origin",
      body: file,
    },
  );
}

export async function deleteTaskAttachment(
  taskId: string,
  attachmentId: string,
): Promise<void> {
  const response = await fetch(
    `/api/tasks/${encodeURIComponent(taskId)}/attachments/${encodeURIComponent(attachmentId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function taskAttachmentUrl(
  taskId: string,
  attachmentId: string,
): string {
  return `/api/tasks/${encodeURIComponent(taskId)}/attachments/${encodeURIComponent(attachmentId)}`;
}

export function fetchLinePosts(
  options: {
    scope?: "instance" | "mine";
    q?: string;
    tag?: string;
  } = {},
): Promise<LinePost[]> {
  const query = new URLSearchParams();
  query.set("scope", options.scope ?? "instance");
  if (options.q?.trim()) query.set("q", options.q.trim());
  if (options.tag?.trim()) query.set("tag", options.tag.trim());
  return requestJson<LinePost[]>(`/api/lines/posts?${query.toString()}`, {
    credentials: "same-origin",
  });
}

export function createLinePost(input: {
  content: string;
  visibility: LineVisibility;
  reply_to_post_id?: string | null;
}): Promise<LinePost> {
  return requestJson<LinePost>("/api/lines/posts", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export async function deleteLinePost(postId: string): Promise<void> {
  const response = await fetch(
    `/api/lines/posts/${encodeURIComponent(postId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function uploadLinePostAttachment(
  postId: string,
  file: File,
): Promise<LinePostAttachment> {
  return requestJson<LinePostAttachment>(
    `/api/lines/posts/${encodeURIComponent(postId)}/attachments?file_name=${encodeURIComponent(file.name)}`,
    {
      method: "POST",
      headers: { "content-type": file.type || "application/octet-stream" },
      credentials: "same-origin",
      body: file,
    },
  );
}

export function linePostAttachmentUrl(
  postId: string,
  attachmentId: string,
): string {
  return `/api/lines/posts/${encodeURIComponent(postId)}/attachments/${encodeURIComponent(attachmentId)}`;
}

export function fetchLineThread(postId: string): Promise<LineThread> {
  return requestJson<LineThread>(
    `/api/lines/posts/${encodeURIComponent(postId)}/thread`,
    { credentials: "same-origin" },
  );
}

export function fetchLineAuthorFeed(userId: string): Promise<LineAuthorFeed> {
  return requestJson<LineAuthorFeed>(
    `/api/lines/authors/${encodeURIComponent(userId)}`,
    { credentials: "same-origin" },
  );
}

export function lineAuthorAvatarUrl(userId: string): string {
  return `/api/lines/authors/${encodeURIComponent(userId)}/avatar`;
}

export function setLinePostReaction(
  postId: string,
  emoji: string,
  active: boolean,
): Promise<LinePost> {
  return requestJson<LinePost>(
    `/api/lines/posts/${encodeURIComponent(postId)}/reactions/${encodeURIComponent(emoji)}`,
    { method: active ? "PUT" : "DELETE", credentials: "same-origin" },
  );
}

export function clearCompletedTasks(): Promise<{ deleted: number }> {
  return requestJson<{ deleted: number }>("/api/tasks/completed", {
    method: "DELETE",
  });
}

export function fetchRssReader(): Promise<RssReaderResponse> {
  return requestJson<RssReaderResponse>("/api/rss", {
    credentials: "same-origin",
  });
}

export function createRssSubscription(
  input: RssSubscriptionInput,
): Promise<RssReaderResponse> {
  return requestJson<RssReaderResponse>("/api/rss/subscriptions", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updateRssSubscription(
  id: string,
  input: Omit<RssSubscriptionInput, "url">,
): Promise<RssReaderResponse> {
  return requestJson<RssReaderResponse>(
    `/api/rss/subscriptions/${encodeURIComponent(id)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}

export async function deleteRssSubscription(id: string): Promise<void> {
  const response = await fetch(
    `/api/rss/subscriptions/${encodeURIComponent(id)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function refreshRssSubscription(id: string): Promise<RssReaderResponse> {
  return requestJson<RssReaderResponse>(
    `/api/rss/subscriptions/${encodeURIComponent(id)}/refresh`,
    { method: "POST", credentials: "same-origin" },
  );
}

export function setRssItemRead(
  id: string,
  read: boolean,
): Promise<RssReaderItem> {
  return requestJson<RssReaderItem>(
    `/api/rss/items/${encodeURIComponent(id)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ read }),
    },
  );
}

export function setRssItemSaved(
  id: string,
  saved: boolean,
): Promise<RssReaderItem> {
  return requestJson<RssReaderItem>(
    `/api/rss/items/${encodeURIComponent(id)}/read-later`,
    {
      method: "PUT",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ saved }),
    },
  );
}

export function pruneRssItems(
  days: number,
  mode: RssRetentionMode,
): Promise<{ deleted: number }> {
  return requestJson<{ deleted: number }>("/api/rss/prune", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ days, mode }),
  });
}

export function fetchYoutubeReader(): Promise<YoutubeReaderResponse> {
  return requestJson<YoutubeReaderResponse>("/api/youtube", {
    credentials: "same-origin",
  });
}

export function createYoutubeSubscription(
  channelId: string,
): Promise<YoutubeReaderResponse> {
  return requestJson<YoutubeReaderResponse>("/api/youtube/subscriptions", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ channel_id: channelId }),
  });
}

export async function deleteYoutubeSubscription(
  channelId: string,
): Promise<void> {
  const response = await fetch(
    `/api/youtube/subscriptions/${encodeURIComponent(channelId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function refreshYoutubeSubscription(
  channelId: string,
): Promise<YoutubeReaderResponse> {
  return requestJson<YoutubeReaderResponse>(
    `/api/youtube/subscriptions/${encodeURIComponent(channelId)}/refresh`,
    { method: "POST", credentials: "same-origin" },
  );
}

export function setYoutubeWatchLater(
  videoId: string,
  saved: boolean,
): Promise<YoutubeReaderResponse> {
  return requestJson<YoutubeReaderResponse>(
    `/api/youtube/videos/${encodeURIComponent(videoId)}/watch-later`,
    { method: saved ? "PUT" : "DELETE", credentials: "same-origin" },
  );
}

export function createYoutubeGroup(
  name: string,
): Promise<YoutubeReaderResponse> {
  return requestJson<YoutubeReaderResponse>("/api/youtube/groups", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ name }),
  });
}

export function updateYoutubeGroup(
  groupId: string,
  name: string,
  channelIds: string[],
): Promise<YoutubeReaderResponse> {
  return requestJson<YoutubeReaderResponse>(
    `/api/youtube/groups/${encodeURIComponent(groupId)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ name, channel_ids: channelIds }),
    },
  );
}

export async function deleteYoutubeGroup(groupId: string): Promise<void> {
  const response = await fetch(
    `/api/youtube/groups/${encodeURIComponent(groupId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function updateYoutubeDisplayMode(
  displayMode: YoutubeDisplayMode,
): Promise<YoutubeReaderResponse> {
  return requestJson<YoutubeReaderResponse>("/api/youtube/display-mode", {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ display_mode: displayMode }),
  });
}

export function fetchCalendar(): Promise<CalendarResponse> {
  return requestJson<CalendarResponse>("/api/calendar", {
    credentials: "same-origin",
  });
}

export function createCalendarSubscription(
  url: string,
  color: CalendarColor,
): Promise<CalendarResponse> {
  return requestJson<CalendarResponse>("/api/calendar/subscriptions", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ url, color }),
  });
}

export function refreshCalendarSubscription(
  id: string,
): Promise<CalendarResponse> {
  return requestJson<CalendarResponse>(
    `/api/calendar/subscriptions/${encodeURIComponent(id)}/refresh`,
    { method: "POST", credentials: "same-origin" },
  );
}

export async function deleteCalendarSubscription(id: string): Promise<void> {
  const response = await fetch(
    `/api/calendar/subscriptions/${encodeURIComponent(id)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function fetchContacts(): Promise<ContactsResponse> {
  return requestJson<ContactsResponse>("/api/contacts", {
    credentials: "same-origin",
  });
}

export function createContact(input: ContactInput): Promise<Contact> {
  return requestJson<Contact>("/api/contacts", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updateContact(
  id: string,
  input: ContactInput,
): Promise<Contact> {
  return requestJson<Contact>(`/api/contacts/${encodeURIComponent(id)}`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export async function deleteContact(id: string): Promise<void> {
  const response = await fetch(`/api/contacts/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export async function updateContactPhoto(
  id: string,
  file: File,
): Promise<void> {
  const response = await fetch(
    `/api/contacts/${encodeURIComponent(id)}/photo`,
    {
      method: "PUT",
      headers: { "content-type": file.type },
      credentials: "same-origin",
      body: file,
    },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? "Unable to save contact photo",
      response.status,
    );
  }
}

export async function deleteContactPhoto(id: string): Promise<void> {
  const response = await fetch(
    `/api/contacts/${encodeURIComponent(id)}/photo`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? "Unable to remove contact photo",
      response.status,
    );
  }
}

export async function exportContacts(): Promise<Blob> {
  const response = await fetch("/api/contacts/export", {
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
  return response.blob();
}

export function importContacts(
  format: "monica-json" | "pandan-json",
  payload: unknown,
): Promise<ContactImportResult> {
  return requestJson<ContactImportResult>("/api/contacts/import", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ format, payload }),
  });
}

export function createContactDavSource(input: {
  name: string;
  url: string;
  username: string;
  password: string;
}): Promise<ContactDavSource> {
  return requestJson<ContactDavSource>("/api/contacts/dav", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function syncContactDavSource(
  id: string,
): Promise<{ source: ContactDavSource; imported: number }> {
  return requestJson<{ source: ContactDavSource; imported: number }>(
    `/api/contacts/dav/${encodeURIComponent(id)}/sync`,
    { method: "POST", credentials: "same-origin" },
  );
}

export async function deleteContactDavSource(id: string): Promise<void> {
  const response = await fetch(`/api/contacts/dav/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function fetchPaymentSubscriptions(): Promise<PaymentSubscription[]> {
  return requestJson<PaymentSubscription[]>("/api/payment-subscriptions", {
    credentials: "same-origin",
  });
}

export function createPaymentSubscription(
  input: PaymentSubscriptionInput,
): Promise<PaymentSubscription> {
  return requestJson<PaymentSubscription>("/api/payment-subscriptions", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updatePaymentSubscription(
  id: string,
  input: PaymentSubscriptionInput,
): Promise<PaymentSubscription> {
  return requestJson<PaymentSubscription>(
    `/api/payment-subscriptions/${encodeURIComponent(id)}`,
    {
      method: "PUT",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}

export async function deletePaymentSubscription(id: string): Promise<void> {
  const response = await fetch(
    `/api/payment-subscriptions/${encodeURIComponent(id)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function fetchCoding(): Promise<CodingResponse> {
  return requestJson<CodingResponse>("/api/coding", {
    credentials: "same-origin",
  });
}

export function createCodingProject(
  repository: string,
): Promise<CodingProject> {
  return requestJson<CodingProject>("/api/coding/projects", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ repository }),
  });
}

export async function deleteCodingProject(id: string): Promise<void> {
  const response = await fetch(
    `/api/coding/projects/${encodeURIComponent(id)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

export function updateCodingCredential(input: {
  provider: CodingProvider;
  host: string;
  token?: string;
  clear?: boolean;
}): Promise<CodingCredential> {
  return requestJson<CodingCredential>("/api/coding/credential", {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function fetchJournal(): Promise<JournalResponse> {
  return requestJson<JournalResponse>("/api/journal", {
    credentials: "same-origin",
  });
}

export function createJournalNode(
  input: CreateJournalNodeInput,
): Promise<JournalNode> {
  return requestJson<JournalNode>("/api/journal/nodes", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

export function updateJournalNode(
  id: string,
  input: UpdateJournalNodeInput,
): Promise<JournalNode> {
  return requestJson<JournalNode>(
    `/api/journal/nodes/${encodeURIComponent(id)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}

export async function deleteJournalNode(id: string): Promise<void> {
  const response = await fetch(`/api/journal/nodes/${encodeURIComponent(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
  if (!response.ok) {
    const payload = (await response
      .json()
      .catch(() => ({}))) as ApiErrorResponse;
    throw new ApiError(
      payload.error ?? `Request failed with status ${response.status}`,
      response.status,
    );
  }
}

const kanbanJson = { "content-type": "application/json" };
const kanbanPath = (value: string) => encodeURIComponent(value);

export function fetchKanbanOverview(): Promise<KanbanOverview> {
  return requestJson<KanbanOverview>("/api/kanban", {
    credentials: "same-origin",
  });
}
export function createKanbanWorkspace(input: {
  name: string;
  description: string;
}): Promise<KanbanWorkspace> {
  return requestJson<KanbanWorkspace>("/api/kanban/workspaces", {
    method: "POST",
    headers: kanbanJson,
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}
export function updateKanbanWorkspace(
  id: string,
  input: { name: string; description: string },
): Promise<void> {
  return requestEmpty(`/api/kanban/workspaces/${kanbanPath(id)}`, {
    method: "PUT",
    headers: kanbanJson,
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}
export function deleteKanbanWorkspace(id: string): Promise<void> {
  return requestEmpty(`/api/kanban/workspaces/${kanbanPath(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}
export function fetchKanbanWorkspaceSettings(
  id: string,
): Promise<KanbanWorkspaceSettings> {
  return requestJson<KanbanWorkspaceSettings>(
    `/api/kanban/workspaces/${kanbanPath(id)}/settings`,
    { credentials: "same-origin" },
  );
}
export function searchKanbanDirectory(
  workspaceId: string,
  query: string,
): Promise<KanbanDirectoryUser[]> {
  return requestJson<KanbanDirectoryUser[]>(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/directory?q=${encodeURIComponent(query)}`,
    { credentials: "same-origin" },
  );
}
export function inviteKanbanMember(
  workspaceId: string,
  userId: string,
  role: KanbanRole,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/members`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ user_id: userId, role }),
    },
  );
}
export function respondKanbanInvitation(
  workspaceId: string,
  accept: boolean,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/invitations`,
    {
      method: "PUT",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ accept }),
    },
  );
}
export function updateKanbanMemberRole(
  workspaceId: string,
  userId: string,
  role: KanbanRole,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/members/${kanbanPath(userId)}`,
    {
      method: "PUT",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ role }),
    },
  );
}
export function removeKanbanMember(
  workspaceId: string,
  userId: string,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/members/${kanbanPath(userId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
}
export function setKanbanRolePermission(
  workspaceId: string,
  role: "member" | "guest",
  permission: string,
  granted: boolean,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/roles/${role}/permissions/${kanbanPath(permission)}`,
    {
      method: "PUT",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ granted }),
    },
  );
}
export function setKanbanMemberPermission(
  workspaceId: string,
  userId: string,
  permission: string,
  granted: boolean,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/members/${kanbanPath(userId)}/permissions/${kanbanPath(permission)}`,
    {
      method: "PUT",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ granted }),
    },
  );
}
export function resetKanbanMemberPermissions(
  workspaceId: string,
  userId: string,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/members/${kanbanPath(userId)}/permissions`,
    { method: "DELETE", credentials: "same-origin" },
  );
}
export function fetchKanbanBoards(
  workspaceId: string,
  archived = false,
): Promise<KanbanBoardSummary[]> {
  return requestJson<KanbanBoardSummary[]>(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/boards?archived=${archived}`,
    { credentials: "same-origin" },
  );
}
export function createKanbanBoard(
  workspaceId: string,
  input: {
    name: string;
    description: string;
    visibility: "private" | "public";
  },
): Promise<KanbanBoard> {
  return requestJson<KanbanBoard>(
    `/api/kanban/workspaces/${kanbanPath(workspaceId)}/boards`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}
export function fetchKanbanBoard(id: string): Promise<KanbanBoard> {
  return requestJson<KanbanBoard>(`/api/kanban/boards/${kanbanPath(id)}`, {
    credentials: "same-origin",
  });
}
export function updateKanbanBoard(
  id: string,
  input: {
    name: string;
    description: string;
    visibility: "private" | "public";
    archived: boolean;
  },
): Promise<void> {
  return requestEmpty(`/api/kanban/boards/${kanbanPath(id)}`, {
    method: "PUT",
    headers: kanbanJson,
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}
export function deleteKanbanBoard(id: string): Promise<void> {
  return requestEmpty(`/api/kanban/boards/${kanbanPath(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}
export function setKanbanBoardFavorite(
  id: string,
  favorite: boolean,
): Promise<void> {
  return requestEmpty(`/api/kanban/boards/${kanbanPath(id)}/favorite`, {
    method: "PUT",
    headers: kanbanJson,
    credentials: "same-origin",
    body: JSON.stringify({ favorite }),
  });
}
export function createKanbanColumn(
  boardId: string,
  name: string,
): Promise<{ id: string }> {
  return requestJson<{ id: string }>(
    `/api/kanban/boards/${kanbanPath(boardId)}/columns`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ name }),
    },
  );
}
export function updateKanbanColumn(
  id: string,
  input: { name?: string; position?: number },
): Promise<void> {
  return requestEmpty(`/api/kanban/columns/${kanbanPath(id)}`, {
    method: "PUT",
    headers: kanbanJson,
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}
export function deleteKanbanColumn(id: string): Promise<void> {
  return requestEmpty(`/api/kanban/columns/${kanbanPath(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}
export function createKanbanCard(
  columnId: string,
  input: KanbanCardInput,
): Promise<KanbanCard> {
  return requestJson<KanbanCard>(
    `/api/kanban/columns/${kanbanPath(columnId)}/cards`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify(input),
    },
  );
}
export function fetchKanbanCard(id: string): Promise<KanbanCard> {
  return requestJson<KanbanCard>(`/api/kanban/cards/${kanbanPath(id)}`, {
    credentials: "same-origin",
  });
}
export function updateKanbanCard(
  id: string,
  input: KanbanCardInput,
): Promise<KanbanCard> {
  return requestJson<KanbanCard>(`/api/kanban/cards/${kanbanPath(id)}`, {
    method: "PUT",
    headers: kanbanJson,
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}
export function moveKanbanCard(
  id: string,
  columnId: string,
  position: number,
): Promise<void> {
  return requestEmpty(`/api/kanban/cards/${kanbanPath(id)}/move`, {
    method: "PUT",
    headers: kanbanJson,
    credentials: "same-origin",
    body: JSON.stringify({ column_id: columnId, position }),
  });
}
export function archiveKanbanCard(id: string): Promise<void> {
  return requestEmpty(`/api/kanban/cards/${kanbanPath(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}
export function createKanbanLabel(
  boardId: string,
  name: string,
  color: KanbanLabelColor,
): Promise<KanbanLabel> {
  return requestJson<KanbanLabel>(
    `/api/kanban/boards/${kanbanPath(boardId)}/labels`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ name, color }),
    },
  );
}
export function deleteKanbanLabel(
  boardId: string,
  labelId: string,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/boards/${kanbanPath(boardId)}/labels/${kanbanPath(labelId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
}
export function createKanbanComment(
  cardId: string,
  content: string,
): Promise<{ id: string }> {
  return requestJson<{ id: string }>(
    `/api/kanban/cards/${kanbanPath(cardId)}/comments`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ content }),
    },
  );
}
export function deleteKanbanComment(id: string): Promise<void> {
  return requestEmpty(`/api/kanban/comments/${kanbanPath(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}
export function createKanbanChecklist(
  cardId: string,
  name: string,
): Promise<{ id: string }> {
  return requestJson<{ id: string }>(
    `/api/kanban/cards/${kanbanPath(cardId)}/checklists`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ name }),
    },
  );
}
export function deleteKanbanChecklist(id: string): Promise<void> {
  return requestEmpty(`/api/kanban/checklists/${kanbanPath(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}
export function createKanbanChecklistItem(
  checklistId: string,
  title: string,
): Promise<{ id: string }> {
  return requestJson<{ id: string }>(
    `/api/kanban/checklists/${kanbanPath(checklistId)}/items`,
    {
      method: "POST",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ title, completed: false }),
    },
  );
}
export function updateKanbanChecklistItem(
  checklistId: string,
  itemId: string,
  title: string,
  completed: boolean,
): Promise<void> {
  return requestEmpty(
    `/api/kanban/checklists/${kanbanPath(checklistId)}/items/${kanbanPath(itemId)}`,
    {
      method: "PUT",
      headers: kanbanJson,
      credentials: "same-origin",
      body: JSON.stringify({ title, completed }),
    },
  );
}
export function uploadKanbanAttachment(
  cardId: string,
  file: File,
): Promise<KanbanAttachment> {
  return requestJson<KanbanAttachment>(
    `/api/kanban/cards/${kanbanPath(cardId)}/attachments?file_name=${encodeURIComponent(file.name)}`,
    {
      method: "POST",
      headers: { "content-type": file.type || "application/octet-stream" },
      credentials: "same-origin",
      body: file,
    },
  );
}
export function kanbanAttachmentUrl(id: string): string {
  return `/api/kanban/attachments/${kanbanPath(id)}`;
}
export function deleteKanbanAttachment(id: string): Promise<void> {
  return requestEmpty(`/api/kanban/attachments/${kanbanPath(id)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}

// --- Podcasts ---------------------------------------------------------------
//
// The catalogue is administrator-curated. Members submit a feed for review and
// subscribe to what has been approved; only administrators publish, decide, or
// change storage policy. Episode audio is served from the instance's own disk by
// `podcastAudioUrl`, never proxied from the origin feed.

export function fetchPodcasts(): Promise<PodcastOverview> {
  return requestJson<PodcastOverview>("/api/podcasts");
}

export function fetchPodcastEpisodes(
  podcastId: string,
  options: { limit?: number; offset?: number } = {},
): Promise<PodcastEpisode[]> {
  const query = new URLSearchParams();
  if (options.limit !== undefined) query.set("limit", String(options.limit));
  if (options.offset !== undefined) query.set("offset", String(options.offset));
  const suffix = query.size > 0 ? `?${query.toString()}` : "";
  return requestJson<PodcastEpisode[]>(
    `/api/podcasts/${encodeURIComponent(podcastId)}/episodes${suffix}`,
  );
}

export function submitPodcastRequest(
  feedUrl: string,
  note: string,
): Promise<PodcastRequestOutcome> {
  return requestJson<PodcastRequestOutcome>("/api/podcasts/requests", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ feed_url: feedUrl, note }),
  });
}

export function withdrawPodcastRequest(requestId: string): Promise<void> {
  return requestEmpty(
    `/api/podcasts/requests/${encodeURIComponent(requestId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
}

export function subscribeToPodcast(podcastId: string): Promise<void> {
  return requestEmpty(
    `/api/podcasts/${encodeURIComponent(podcastId)}/subscription`,
    { method: "PUT", credentials: "same-origin" },
  );
}

export function unsubscribeFromPodcast(podcastId: string): Promise<void> {
  return requestEmpty(
    `/api/podcasts/${encodeURIComponent(podcastId)}/subscription`,
    { method: "DELETE", credentials: "same-origin" },
  );
}

export function podcastArtworkUrl(podcastId: string): string {
  return `/api/podcasts/${encodeURIComponent(podcastId)}/artwork`;
}

/** The instance's own copy of the episode. Supports range requests, so seeking works. */
export function podcastAudioUrl(episodeId: string): string {
  return `/api/podcasts/episodes/${encodeURIComponent(episodeId)}/audio`;
}

export function requestPodcastDownload(episodeId: string): Promise<void> {
  return requestEmpty(
    `/api/podcasts/episodes/${encodeURIComponent(episodeId)}/download`,
    { method: "POST", credentials: "same-origin" },
  );
}

export function savePodcastProgress(
  episodeId: string,
  positionSeconds: number,
  completed: boolean,
): Promise<void> {
  return requestEmpty(
    `/api/podcasts/episodes/${encodeURIComponent(episodeId)}/progress`,
    {
      method: "PUT",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({
        position_seconds: Math.max(0, Math.round(positionSeconds)),
        completed,
      }),
    },
  );
}

export function setPodcastEpisodeSaved(
  episodeId: string,
  saved: boolean,
): Promise<void> {
  return requestEmpty(
    `/api/podcasts/episodes/${encodeURIComponent(episodeId)}/saved`,
    { method: saved ? "PUT" : "DELETE", credentials: "same-origin" },
  );
}

export function appendToPodcastQueue(
  episodeId: string,
): Promise<PodcastEpisode[]> {
  return requestJson<PodcastEpisode[]>("/api/podcasts/queue", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ episode_id: episodeId }),
  });
}

export function reorderPodcastQueue(
  episodeIds: string[],
): Promise<PodcastEpisode[]> {
  return requestJson<PodcastEpisode[]>("/api/podcasts/queue", {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ episode_ids: episodeIds }),
  });
}

export function removeFromPodcastQueue(
  episodeId: string,
): Promise<PodcastEpisode[]> {
  return requestJson<PodcastEpisode[]>(
    `/api/podcasts/queue/${encodeURIComponent(episodeId)}`,
    { method: "DELETE", credentials: "same-origin" },
  );
}

// --- Podcasts: administrator ------------------------------------------------

export function fetchPodcastRequests(
  status: "all" | PodcastRequestStatus = "pending",
): Promise<PodcastRequest[]> {
  return requestJson<PodcastRequest[]>(
    `/api/podcasts/requests?status=${encodeURIComponent(status)}`,
  );
}

export function approvePodcastRequest(
  requestId: string,
  note: string,
): Promise<Podcast> {
  return requestJson<Podcast>(
    `/api/podcasts/requests/${encodeURIComponent(requestId)}/approve`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ note }),
    },
  );
}

export function rejectPodcastRequest(
  requestId: string,
  note: string,
): Promise<void> {
  return requestEmpty(
    `/api/podcasts/requests/${encodeURIComponent(requestId)}/reject`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({ note }),
    },
  );
}

export function addPodcast(feedUrl: string): Promise<Podcast> {
  return requestJson<Podcast>("/api/podcasts", {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ feed_url: feedUrl }),
  });
}

export function updatePodcastRetention(
  podcastId: string,
  autoDownloadCount: number,
  maxRetainedEpisodes: number,
): Promise<Podcast> {
  return requestJson<Podcast>(
    `/api/podcasts/${encodeURIComponent(podcastId)}`,
    {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      credentials: "same-origin",
      body: JSON.stringify({
        auto_download_count: autoDownloadCount,
        max_retained_episodes: maxRetainedEpisodes,
      }),
    },
  );
}

export function deletePodcast(podcastId: string): Promise<void> {
  return requestEmpty(`/api/podcasts/${encodeURIComponent(podcastId)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}

export function removePodcastDownload(episodeId: string): Promise<void> {
  return requestEmpty(
    `/api/podcasts/episodes/${encodeURIComponent(episodeId)}/download`,
    { method: "DELETE", credentials: "same-origin" },
  );
}

/**
 * Queues every uncached episode of one show. Administrator-only.
 *
 * Resolves with the number of episodes newly queued; episodes already cached or already
 * in flight are left alone and not counted.
 */
export function downloadAllPodcastEpisodes(
  podcastId: string,
): Promise<{ queued: number }> {
  return requestJson<{ queued: number }>(
    `/api/podcasts/${encodeURIComponent(podcastId)}/downloads`,
    { method: "POST", credentials: "same-origin" },
  );
}

export function fetchPodcastSettings(): Promise<PodcastAdminSettings> {
  return requestJson<PodcastAdminSettings>("/api/podcasts/settings");
}

export function updatePodcastSettings(input: {
  requests_enabled: boolean;
  member_downloads_enabled: boolean;
  max_pending_requests_per_user: number;
  storage_budget_bytes: number;
  max_episode_bytes: number;
  default_auto_download_count: number;
}): Promise<PodcastAdminSettings> {
  return requestJson<PodcastAdminSettings>("/api/podcasts/settings", {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(input),
  });
}

// --- Walls ------------------------------------------------------------------

export type WallStatus = "pending" | "approved" | "rejected";

export type WallScope = "collection" | "mine" | "review";

export type WallSlot = "welcome" | "login";

export interface Wall {
  id: string;
  user_id: string | null;
  submitted_by_name: string;
  title: string;
  description: string;
  status: WallStatus;
  decision_note: string;
  decided_by_name: string | null;
  decided_at: string | null;
  mime_type: string;
  byte_size: number;
  width: number;
  height: number;
  tags: string[];
  created_at: string;
  updated_at: string;
}

export interface WallSelections {
  welcome: string | null;
  login: string | null;
}

export function wallThumbnailUrl(wallId: string): string {
  return `/api/walls/${encodeURIComponent(wallId)}/thumbnail`;
}

export function wallImageUrl(wallId: string): string {
  return `/api/walls/${encodeURIComponent(wallId)}/image`;
}

export function fetchWalls(
  options: {
    scope?: WallScope;
    status?: WallStatus | "";
    q?: string;
    tag?: string;
  } = {},
): Promise<Wall[]> {
  const params = new URLSearchParams();
  if (options.scope) params.set("scope", options.scope);
  if (options.status) params.set("status", options.status);
  if (options.q) params.set("q", options.q);
  if (options.tag) params.set("tag", options.tag);
  const suffix = params.size > 0 ? `?${params.toString()}` : "";
  return requestJson<Wall[]>(`/api/walls${suffix}`);
}

export function fetchWall(wallId: string): Promise<Wall> {
  return requestJson<Wall>(`/api/walls/${encodeURIComponent(wallId)}`);
}

export function fetchWallSelections(): Promise<WallSelections> {
  return requestJson<WallSelections>("/api/walls/selections");
}

export function submitWall(
  file: File,
  details: { title: string; description: string; tags: string[] },
): Promise<Wall> {
  const params = new URLSearchParams({
    title: details.title,
    description: details.description,
    tags: details.tags.join(","),
  });
  return requestJson<Wall>(`/api/walls?${params.toString()}`, {
    method: "POST",
    headers: { "content-type": file.type },
    credentials: "same-origin",
    body: file,
  });
}

export function updateWall(
  wallId: string,
  details: { title: string; description: string; tags: string[] },
): Promise<Wall> {
  return requestJson<Wall>(`/api/walls/${encodeURIComponent(wallId)}`, {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify(details),
  });
}

export function deleteWall(wallId: string): Promise<void> {
  return requestEmpty(`/api/walls/${encodeURIComponent(wallId)}`, {
    method: "DELETE",
    credentials: "same-origin",
  });
}

export function approveWall(wallId: string, note: string): Promise<Wall> {
  return requestJson<Wall>(`/api/walls/${encodeURIComponent(wallId)}/approve`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ note }),
  });
}

export function rejectWall(wallId: string, note: string): Promise<Wall> {
  return requestJson<Wall>(`/api/walls/${encodeURIComponent(wallId)}/reject`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ note }),
  });
}

export function applyWall(wallId: string, slot: WallSlot): Promise<void> {
  return requestEmpty(`/api/walls/${encodeURIComponent(wallId)}/apply`, {
    method: "PUT",
    headers: { "content-type": "application/json" },
    credentials: "same-origin",
    body: JSON.stringify({ slot }),
  });
}

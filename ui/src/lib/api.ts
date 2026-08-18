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
  | "search"
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
  updated_at: string;
}

export type UserContentScope =
  | "contacts"
  | "tasks"
  | "calendar"
  | "rss"
  | "journal"
  | "youtube"
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

export async function updateContactPhoto(id: string, file: File): Promise<void> {
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

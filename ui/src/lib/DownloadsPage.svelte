<script lang="ts">
  import AlertTriangle from "lucide-svelte/icons/alert-triangle";
  import CheckCircle2 from "lucide-svelte/icons/check-circle-2";
  import CirclePlay from "lucide-svelte/icons/circle-play";
  import Download from "lucide-svelte/icons/download";
  import FileAudio from "lucide-svelte/icons/file-audio";
  import FileVideo from "lucide-svelte/icons/file-video";
  import HardDrive from "lucide-svelte/icons/hard-drive";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import RotateCcw from "lucide-svelte/icons/rotate-ccw";
  import ScanSearch from "lucide-svelte/icons/scan-search";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import {
    cancelYoutubeDownload,
    createYoutubeDownloadJobs,
    deleteYoutubeDownload,
    fetchYoutubeDownloadPolicy,
    fetchYoutubeDownloads,
    inspectYoutubeDownload,
    listYoutubeDownloadJobs,
    openYoutubeDownloadEventStream,
    retryYoutubeDownload,
    updateYoutubeDownloadPolicy,
    youtubeDownloadFileUrl,
    youtubeDownloadPreviewUrl,
    type YoutubeDownloadAdminPolicy,
    type YoutubeDownloadFormat,
    type YoutubeDownloadInspection,
    type YoutubeDownloadJob,
    type YoutubeDownloadMediaKind,
    type YoutubeDownloadOverview,
  } from "$lib/api";
  import { podcastPlayer } from "$lib/podcastPlayer.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";

  type DownloadsView = "new" | "queue" | "library" | "policy";
  type ComposerMode = "single" | "batch";
  type LibraryFilter = "all" | "complete" | "failed" | "cancelled";

  let {
    viewerRole,
    initialUrl = "",
    onPrefillHandled = () => {},
  }: {
    viewerRole: "administrator" | "member";
    initialUrl?: string;
    onPrefillHandled?: () => void;
  } = $props();

  let view = $state<DownloadsView>("new");
  let overview = $state.raw<YoutubeDownloadOverview | null>(null);
  let history = $state.raw<YoutubeDownloadJob[]>([]);
  let loading = $state(true);
  let refreshing = $state(false);
  let pageError = $state("");
  let streamConnected = $state(true);
  let hasLoaded = false;

  let composerMode = $state<ComposerMode>("single");
  let singleUrl = $state("");
  let batchUrls = $state("");
  let inspection = $state.raw<YoutubeDownloadInspection | null>(null);
  let inspectedUrl = $state("");
  let mediaKind = $state<YoutubeDownloadMediaKind>("video");
  let outputFormat = $state<YoutubeDownloadFormat>("mp4");
  let maxHeight = $state<number | null>(null);
  let composerBusy = $state(false);
  let composerError = $state("");
  let rejectedUrls = $state.raw<Array<{ url: string; error: string }>>([]);
  let queuedNotice = $state("");

  let libraryFilter = $state<LibraryFilter>("all");
  let loadingOlder = $state(false);
  let historyExhausted = $state(false);
  let busyJobId = $state("");
  let confirmingDelete = $state("");
  let previewJob = $state.raw<YoutubeDownloadJob | null>(null);
  let previewError = $state("");
  let previewDialog: HTMLDialogElement | undefined;

  let policy = $state.raw<YoutubeDownloadAdminPolicy | null>(null);
  let policyBusy = $state(false);
  let policyError = $state("");
  let policySaved = $state("");

  const activeStatuses = [
    "queued",
    "inspecting",
    "downloading",
    "postprocessing",
  ];
  const settledStatuses = ["complete", "failed", "cancelled"];
  const videoFormats: YoutubeDownloadFormat[] = ["mp4", "mkv", "webm"];
  const audioFormats: YoutubeDownloadFormat[] = ["m4a", "mp3", "opus"];

  let activeJobs = $derived(
    overview?.active_jobs.filter((job) =>
      activeStatuses.includes(job.status),
    ) ?? [],
  );
  let settledHistory = $derived(
    history.filter((job) => settledStatuses.includes(job.status)),
  );
  let filteredHistory = $derived(
    settledHistory.filter(
      (job) => libraryFilter === "all" || job.status === libraryFilter,
    ),
  );
  let availableFormats = $derived.by(() => {
    const profiles = mediaKind === "video" ? videoFormats : audioFormats;
    if (composerMode !== "single" || !inspection) return profiles;
    const sourceFormats =
      mediaKind === "video"
        ? inspection.video_formats
        : inspection.audio_formats;
    return profiles.filter((format) => sourceFormats.includes(format));
  });
  let memberBlocked = $derived(
    overview !== null &&
      viewerRole !== "administrator" &&
      !overview.policy.member_downloads_enabled,
  );
  let capabilityBlocked = $derived(
    overview !== null && !overview.capability.available,
  );

  $effect(() => {
    if (!initialUrl) return;
    view = "new";
    composerMode = "single";
    singleUrl = initialUrl;
    clearInspection();
    onPrefillHandled();
  });

  onMount(() => {
    let source: EventSource | null = null;
    let disposed = false;
    void loadSnapshot();
    source = openYoutubeDownloadEventStream();
    source.onopen = () => {
      if (disposed) return;
      const reconnecting = !streamConnected;
      streamConnected = true;
      if (reconnecting) void loadSnapshot(true);
    };
    source.onerror = () => {
      if (!disposed) streamConnected = false;
    };
    source.addEventListener("job", (event) => {
      if (disposed || !(event instanceof MessageEvent)) return;
      try {
        mergeJob(JSON.parse(event.data) as YoutubeDownloadJob);
      } catch {
        streamConnected = false;
      }
    });
    return () => {
      disposed = true;
      source?.close();
    };
  });

  async function loadSnapshot(quiet = false) {
    if (hasLoaded || quiet) refreshing = true;
    else loading = true;
    pageError = "";
    try {
      const next = await fetchYoutubeDownloads();
      overview = next;
      history = dedupeJobs(next.history);
      historyExhausted = next.history.length < 30;
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to load downloads");
    } finally {
      hasLoaded = true;
      loading = false;
      refreshing = false;
    }
  }

  function mergeJob(job: YoutubeDownloadJob) {
    if (!overview) return;
    const nextActive = activeStatuses.includes(job.status)
      ? [job, ...overview.active_jobs.filter((entry) => entry.id !== job.id)]
      : overview.active_jobs.filter((entry) => entry.id !== job.id);
    overview = { ...overview, active_jobs: nextActive };
    history = dedupeJobs([job, ...history]);
  }

  function dedupeJobs(jobs: YoutubeDownloadJob[]) {
    const seen: string[] = [];
    return jobs.filter((job) => {
      if (seen.includes(job.id)) return false;
      seen.push(job.id);
      return true;
    });
  }

  function selectView(next: DownloadsView) {
    view = next;
    pageError = "";
    if (next === "policy" && viewerRole === "administrator" && !policy)
      void loadPolicy();
  }

  function clearInspection() {
    inspection = null;
    inspectedUrl = "";
    rejectedUrls = [];
    queuedNotice = "";
    composerError = "";
  }

  function setComposerMode(next: ComposerMode) {
    composerMode = next;
    clearInspection();
  }

  function selectMediaKind(next: YoutubeDownloadMediaKind) {
    mediaKind = next;
    const profiles = next === "video" ? videoFormats : audioFormats;
    const sourceFormats = inspection
      ? next === "video"
        ? inspection.video_formats
        : inspection.audio_formats
      : profiles;
    const formats = profiles.filter((format) => sourceFormats.includes(format));
    if (!formats.includes(outputFormat)) outputFormat = formats[0];
    if (next === "audio") maxHeight = null;
  }

  async function inspectSingle() {
    composerBusy = true;
    composerError = "";
    queuedNotice = "";
    try {
      const next = await inspectYoutubeDownload(singleUrl.trim());
      inspection = next;
      inspectedUrl = singleUrl.trim();
      maxHeight = next.available_heights[0] ?? null;
      const formats =
        mediaKind === "video" ? next.video_formats : next.audio_formats;
      if (!formats.includes(outputFormat)) outputFormat = formats[0];
    } catch (reason: unknown) {
      inspection = null;
      composerError = message(reason, "Unable to inspect this video");
    } finally {
      composerBusy = false;
    }
  }

  async function queueDownloads() {
    composerBusy = true;
    composerError = "";
    queuedNotice = "";
    rejectedUrls = [];
    const urls =
      composerMode === "single"
        ? [singleUrl.trim()]
        : batchUrls
            .split(/\r?\n/)
            .map((url) => url.trim())
            .filter(Boolean);
    try {
      const result = await createYoutubeDownloadJobs({
        urls,
        media_kind: mediaKind,
        output_format: outputFormat,
        max_height: mediaKind === "video" ? maxHeight : null,
      });
      for (const job of result.jobs) mergeJob(job);
      rejectedUrls = result.rejected;
      queuedNotice = result.jobs.length
        ? `${result.jobs.length} ${result.jobs.length === 1 ? "download" : "downloads"} added to the queue.`
        : "No downloads were queued.";
      if (result.jobs.length && composerMode === "single") {
        inspection = null;
        inspectedUrl = "";
        singleUrl = "";
        selectView("queue");
      }
    } catch (reason: unknown) {
      composerError = message(reason, "Unable to queue downloads");
    } finally {
      composerBusy = false;
    }
  }

  async function cancelJob(job: YoutubeDownloadJob) {
    busyJobId = job.id;
    pageError = "";
    try {
      mergeJob(await cancelYoutubeDownload(job.id));
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to cancel download");
    } finally {
      busyJobId = "";
    }
  }

  async function retryJob(job: YoutubeDownloadJob) {
    busyJobId = job.id;
    pageError = "";
    try {
      mergeJob(await retryYoutubeDownload(job.id));
      selectView("queue");
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to retry download");
    } finally {
      busyJobId = "";
    }
  }

  async function removeJob(job: YoutubeDownloadJob) {
    if (confirmingDelete !== job.id) {
      confirmingDelete = job.id;
      return;
    }
    busyJobId = job.id;
    pageError = "";
    try {
      await deleteYoutubeDownload(job.id);
      history = history.filter((entry) => entry.id !== job.id);
      if (overview && job.status === "complete") {
        overview = {
          ...overview,
          usage_bytes: Math.max(0, overview.usage_bytes - job.byte_size),
        };
      }
      confirmingDelete = "";
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to delete download");
    } finally {
      busyJobId = "";
    }
  }

  function capturePreviewDialog(node: HTMLDialogElement) {
    previewDialog = node;
    return () => {
      if (previewDialog === node) previewDialog = undefined;
    };
  }

  async function openPreview(job: YoutubeDownloadJob) {
    if (job.media_kind !== "video") return;
    previewJob = job;
    previewError = "";
    await tick();
    if (previewDialog && !previewDialog.open) previewDialog.showModal();
  }

  function playOrView(job: YoutubeDownloadJob) {
    if (job.media_kind === "audio") {
      void podcastPlayer.playDownload(job);
      return;
    }
    void openPreview(job);
  }

  function closePreview() {
    previewDialog?.close();
  }

  function resetPreview() {
    previewJob = null;
    previewError = "";
  }

  async function loadOlder() {
    const oldest = settledHistory.at(-1);
    if (!oldest) return;
    loadingOlder = true;
    pageError = "";
    try {
      const next = await listYoutubeDownloadJobs({
        before: oldest.created_at,
        limit: 30,
      });
      history = dedupeJobs([...history, ...next]);
      historyExhausted = next.length < 30;
    } catch (reason: unknown) {
      pageError = message(reason, "Unable to load older downloads");
    } finally {
      loadingOlder = false;
    }
  }

  async function loadPolicy() {
    policyBusy = true;
    policyError = "";
    try {
      policy = await fetchYoutubeDownloadPolicy();
    } catch (reason: unknown) {
      policyError = message(reason, "Unable to load download policy");
    } finally {
      policyBusy = false;
    }
  }

  async function savePolicy() {
    if (!policy) return;
    policyBusy = true;
    policyError = "";
    policySaved = "";
    try {
      policy = await updateYoutubeDownloadPolicy({
        member_downloads_enabled: policy.member_downloads_enabled,
        storage_budget_bytes: policy.storage_budget_bytes,
        per_user_budget_bytes: policy.per_user_budget_bytes,
        max_output_bytes: policy.max_output_bytes,
        global_concurrency: policy.global_concurrency,
        per_user_concurrency: policy.per_user_concurrency,
        max_batch_urls: policy.max_batch_urls,
        max_queued_per_user: policy.max_queued_per_user,
      });
      policySaved = "Policy saved.";
      if (overview) {
        overview = {
          ...overview,
          policy: {
            member_downloads_enabled: policy.member_downloads_enabled,
            per_user_budget_bytes: policy.per_user_budget_bytes,
            max_output_bytes: policy.max_output_bytes,
            max_batch_urls: policy.max_batch_urls,
            max_queued_per_user: policy.max_queued_per_user,
          },
        };
      }
    } catch (reason: unknown) {
      policyError = message(reason, "Unable to save download policy");
    } finally {
      policyBusy = false;
    }
  }

  function updatePolicyNumber(
    field: keyof YoutubeDownloadAdminPolicy,
    value: string,
    unit = 1,
  ) {
    if (!policy) return;
    const next = Number(value);
    if (!Number.isFinite(next)) return;
    policy = { ...policy, [field]: Math.round(next * unit) };
  }

  function profileLabel(job: YoutubeDownloadJob) {
    const height = job.max_height ? ` · ${job.max_height}p` : "";
    return `${job.media_kind} · ${job.output_format.toUpperCase()}${height}`;
  }

  function statusLabel(job: YoutubeDownloadJob, index = 0) {
    if (job.status === "queued") return `Queued · position ${index + 1}`;
    if (job.status === "inspecting") return "Preparing source";
    if (job.status === "postprocessing") return "Finalizing file";
    if (job.status === "downloading" && job.progress_percent !== null) {
      return `${Math.round(job.progress_percent)}% · ${formatBytes(job.downloaded_bytes)}${job.total_bytes ? ` of ${formatBytes(job.total_bytes)}` : ""}`;
    }
    return job.status;
  }

  function formatBytes(bytes: number) {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    const exponent = Math.min(
      Math.floor(Math.log(bytes) / Math.log(1024)),
      units.length - 1,
    );
    return `${(bytes / 1024 ** exponent).toFixed(exponent === 0 ? 0 : 1)} ${units[exponent]}`;
  }

  function formatDuration(seconds: number | null) {
    if (seconds === null) return "Duration unavailable";
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    const remainder = Math.floor(seconds % 60);
    return hours
      ? `${hours}:${String(minutes).padStart(2, "0")}:${String(remainder).padStart(2, "0")}`
      : `${minutes}:${String(remainder).padStart(2, "0")}`;
  }

  function formatDate(value: string) {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(value));
  }

  function compactToolVersion(value: string | null) {
    if (!value) return "not detected";
    return value
      .split(" · ", 1)[0]
      .replace(/\s+Copyright.*$/, "")
      .replace(/\s+\(stable.*$/, "")
      .trim();
  }

  function message(reason: unknown, fallback: string) {
    return reason instanceof Error ? reason.message : fallback;
  }
</script>

<section class="downloads-page product-page" data-od-id="downloads-page">
  <header class="downloads-header page-header" data-od-id="downloads-heading">
    <div>
      <TypedHeading text={`$ downloads --${view}`} odId="downloads-heading" />
      <p>
        Private, account-scoped YouTube transfers with server-enforced formats
        and storage limits.
      </p>
    </div>
    <button
      class="ui-button ui-button--secondary"
      type="button"
      onclick={() => void loadSnapshot()}
      disabled={refreshing}
      data-od-id="downloads-refresh"
    >
      <RefreshCw size={15} strokeWidth={1.8} aria-hidden="true" />
      {refreshing ? "Refreshing" : "Refresh"}
    </button>
  </header>

  <nav
    class="downloads-view-tabs"
    aria-label="Download views"
    data-od-id="downloads-views"
  >
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={view === "new"}
      onclick={() => selectView("new")}>New</button
    >
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={view === "queue"}
      onclick={() => selectView("queue")}
      >Queue <span>{activeJobs.length}</span></button
    >
    <button
      class="ui-view-tab"
      type="button"
      aria-pressed={view === "library"}
      onclick={() => selectView("library")}
      >Library <span
        >{settledHistory.filter((job) => job.status === "complete")
          .length}</span
      ></button
    >
    {#if viewerRole === "administrator"}
      <button
        class="ui-view-tab"
        type="button"
        aria-pressed={view === "policy"}
        onclick={() => selectView("policy")}>Policy</button
      >
    {/if}
  </nav>

  {#if !streamConnected}
    <div
      class="downloads-connection"
      role="status"
      data-od-id="downloads-connection-status"
    >
      Live updates interrupted. Existing records remain available while Pandan
      reconnects.
    </div>
  {/if}
  {#if pageError}
    <div class="downloads-message downloads-message--error" role="alert">
      <span>{pageError}</span>
      <button
        class="ui-button ui-button--ghost"
        type="button"
        onclick={() => (pageError = "")}>Dismiss</button
      >
    </div>
  {/if}

  {#if loading && !overview}
    <div class="downloads-loading" role="status">Loading download service…</div>
  {:else if view === "new"}
    <div class="downloads-new" data-od-id="downloads-new-view">
      {#if capabilityBlocked || memberBlocked}
        <section
          class="downloads-unavailable"
          data-od-id="downloads-unavailable"
        >
          <AlertTriangle size={22} strokeWidth={1.7} aria-hidden="true" />
          <div>
            <h3>
              {capabilityBlocked
                ? "Download service unavailable"
                : "Member downloads are paused"}
            </h3>
            <p>
              {capabilityBlocked
                ? overview?.capability.unavailable_reason
                : "An administrator has disabled new member downloads. Your existing library remains available."}
            </p>
          </div>
        </section>
      {:else}
        <section class="downloads-composer" data-od-id="downloads-composer">
          <div class="downloads-composer-heading">
            <div>
              <span>[ SOURCE ]</span>
              <h3>Queue a YouTube video</h3>
            </div>
            <div
              class="downloads-segments"
              role="group"
              aria-label="Composer mode"
            >
              <button
                type="button"
                aria-pressed={composerMode === "single"}
                onclick={() => setComposerMode("single")}>Single</button
              >
              <button
                type="button"
                aria-pressed={composerMode === "batch"}
                onclick={() => setComposerMode("batch")}>Batch</button
              >
            </div>
          </div>

          {#if composerMode === "single"}
            <label class="downloads-source-field">
              <span>YouTube URL</span>
              <input
                type="url"
                bind:value={singleUrl}
                oninput={() => {
                  if (singleUrl.trim() !== inspectedUrl) clearInspection();
                }}
                placeholder="https://www.youtube.com/watch?v=…"
                autocomplete="off"
                data-od-id="download-url-input"
              />
            </label>
            {#if !inspection}
              <button
                class="ui-button ui-button--primary downloads-submit"
                type="button"
                onclick={() => void inspectSingle()}
                disabled={composerBusy || !singleUrl.trim()}
                data-od-id="download-inspect"
              >
                <ScanSearch size={16} strokeWidth={1.8} aria-hidden="true" />
                {composerBusy ? "Inspecting" : "Inspect video"}
              </button>
            {:else}
              <article
                class="downloads-inspection"
                data-od-id="download-inspection"
              >
                <div class="downloads-inspection-mark" aria-hidden="true">
                  <FileVideo size={24} strokeWidth={1.5} />
                </div>
                <div>
                  <span>{inspection.channel_name}</span>
                  <h3>{inspection.title}</h3>
                  <p>
                    {formatDuration(inspection.duration_seconds)} · {inspection
                      .available_heights.length
                      ? `up to ${inspection.available_heights[0]}p`
                      : "source quality"}
                  </p>
                </div>
                <CheckCircle2
                  size={20}
                  strokeWidth={1.7}
                  aria-label="Source validated"
                />
              </article>
            {/if}
          {:else}
            <label class="downloads-source-field">
              <span
                >YouTube URLs · one per line · up to {overview?.policy
                  .max_batch_urls ?? 10}</span
              >
              <textarea
                bind:value={batchUrls}
                rows="8"
                placeholder="https://youtu.be/…&#10;https://www.youtube.com/watch?v=…"
                data-od-id="download-batch-input"></textarea>
            </label>
          {/if}

          {#if composerMode === "batch" || inspection}
            <div class="downloads-profile" data-od-id="download-profile">
              <fieldset>
                <legend>Media</legend>
                <div class="downloads-segments">
                  <button
                    type="button"
                    aria-pressed={mediaKind === "video"}
                    onclick={() => selectMediaKind("video")}
                    ><FileVideo
                      size={15}
                      strokeWidth={1.8}
                      aria-hidden="true"
                    /> Video</button
                  >
                  <button
                    type="button"
                    aria-pressed={mediaKind === "audio"}
                    onclick={() => selectMediaKind("audio")}
                    ><FileAudio
                      size={15}
                      strokeWidth={1.8}
                      aria-hidden="true"
                    /> Audio</button
                  >
                </div>
              </fieldset>
              <label>
                <span>Format</span>
                <select bind:value={outputFormat}>
                  {#each availableFormats as format (format)}
                    <option value={format}>{format.toUpperCase()}</option>
                  {/each}
                </select>
              </label>
              {#if mediaKind === "video"}
                <label>
                  <span>Maximum resolution</span>
                  <select bind:value={maxHeight}>
                    <option value={null}>Best available</option>
                    {#each inspection?.available_heights ?? [2160, 1440, 1080, 720, 480, 360] as height (height)}
                      <option value={height}>{height}p</option>
                    {/each}
                  </select>
                </label>
              {/if}
            </div>
            <div class="downloads-limit-note">
              <HardDrive size={16} strokeWidth={1.7} aria-hidden="true" />
              Maximum output {formatBytes(
                overview?.policy.max_output_bytes ?? 0,
              )} · your storage {formatBytes(overview?.usage_bytes ?? 0)} of {formatBytes(
                overview?.policy.per_user_budget_bytes ?? 0,
              )}
            </div>
            <button
              class="ui-button ui-button--primary downloads-submit"
              type="button"
              onclick={() => void queueDownloads()}
              disabled={composerBusy ||
                (composerMode === "batch" ? !batchUrls.trim() : !inspection)}
              data-od-id="download-queue-submit"
            >
              <Download size={16} strokeWidth={1.8} aria-hidden="true" />
              {composerBusy
                ? "Queuing"
                : composerMode === "batch"
                  ? "Queue batch"
                  : "Queue download"}
            </button>
          {/if}

          {#if composerError}<p class="downloads-inline-error" role="alert">
              {composerError}
            </p>{/if}
          {#if queuedNotice}<p class="downloads-inline-success" role="status">
              {queuedNotice}
            </p>{/if}
          {#if rejectedUrls.length}
            <div
              class="downloads-rejections"
              data-od-id="download-batch-rejections"
            >
              <strong>{rejectedUrls.length} rejected</strong>
              {#each rejectedUrls as rejection (`${rejection.url}:${rejection.error}`)}
                <p><span>{rejection.url}</span>{rejection.error}</p>
              {/each}
            </div>
          {/if}
          <p class="downloads-legal-note">
            Save only content you own or have permission to download. You are
            responsible for complying with the source's terms and applicable
            law.
          </p>
        </section>
      {/if}
    </div>
  {:else if view === "queue"}
    <section class="downloads-queue" data-od-id="downloads-queue-view">
      <div class="downloads-section-heading">
        <div>
          <span>[ ACTIVE TRANSFERS ]</span>
          <h3>{activeJobs.length} in progress</h3>
        </div>
        <p>
          Jobs are scheduled fairly across accounts. Closing this page does not
          stop a transfer.
        </p>
      </div>
      {#if activeJobs.length}
        <div class="downloads-job-list">
          {#each activeJobs as job, index (job.id)}
            <article
              class="downloads-job"
              data-od-id={`download-job-${job.id}`}
            >
              <div class="downloads-job-title">
                <span>{job.channel_name || "YouTube"}</span>
                <h3>{job.title || "Preparing video metadata"}</h3>
                <p>{profileLabel(job)} · added {formatDate(job.created_at)}</p>
              </div>
              <div class="downloads-job-state">
                <strong>{statusLabel(job, index)}</strong>
                <div
                  class:indeterminate={job.progress_percent === null}
                  class="downloads-progress"
                  role="progressbar"
                  aria-label={statusLabel(job, index)}
                  aria-valuemin="0"
                  aria-valuemax="100"
                  aria-valuenow={job.progress_percent === null
                    ? undefined
                    : Math.round(job.progress_percent)}
                >
                  <span
                    style:width={`${Math.max(0, Math.min(100, job.progress_percent ?? 0))}%`}
                  ></span>
                </div>
                <small
                  >{job.speed_bytes_per_second
                    ? `${formatBytes(job.speed_bytes_per_second)}/s`
                    : ""}{job.eta_seconds
                    ? ` · ${job.eta_seconds}s remaining`
                    : ""}</small
                >
              </div>
              <button
                class="ui-button ui-button--secondary"
                type="button"
                onclick={() => void cancelJob(job)}
                disabled={busyJobId === job.id}
                data-od-id={`download-cancel-${job.id}`}
              >
                <X size={15} strokeWidth={1.9} aria-hidden="true" /> Cancel
              </button>
            </article>
          {/each}
        </div>
      {:else}
        <div class="downloads-empty">
          <CheckCircle2 size={24} strokeWidth={1.5} aria-hidden="true" />
          <h3>The queue is clear</h3>
          <p>
            New transfers will continue here even if you move to another Pandan
            page.
          </p>
          <button
            class="ui-button ui-button--secondary"
            type="button"
            onclick={() => selectView("new")}>Queue a video</button
          >
        </div>
      {/if}
    </section>
  {:else if view === "library"}
    <section class="downloads-library" data-od-id="downloads-library-view">
      <div class="downloads-library-toolbar">
        <div class="downloads-filters" role="group" aria-label="Library filter">
          {#each ["all", "complete", "failed", "cancelled"] as filter (filter)}
            <button
              type="button"
              aria-pressed={libraryFilter === filter}
              onclick={() => (libraryFilter = filter as LibraryFilter)}
              >{filter}</button
            >
          {/each}
        </div>
        <p>{formatBytes(overview?.usage_bytes ?? 0)} stored</p>
      </div>
      {#if filteredHistory.length}
        <div class="downloads-library-list">
          {#each filteredHistory as job (job.id)}
            <article
              class="downloads-library-row"
              data-od-id={`download-library-${job.id}`}
            >
              <span class={`downloads-status downloads-status--${job.status}`}
                >{job.status}</span
              >
              <div>
                <h3>{job.title || "Untitled YouTube video"}</h3>
                <p>
                  {job.channel_name || "YouTube"} · {profileLabel(job)} · {job.status ===
                  "complete"
                    ? `${formatBytes(job.byte_size)} · completed ${formatDate(job.completed_at ?? job.updated_at)}`
                    : formatDate(job.updated_at)}
                </p>
                {#if job.status === "complete" && job.display_file_name}<small
                    >{job.display_file_name}</small
                  >{/if}
                {#if job.last_error}<small>{job.last_error}</small>{/if}
              </div>
              <div class="downloads-row-actions">
                {#if job.status === "complete"}
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    onclick={() => playOrView(job)}
                    data-od-id={`download-preview-${job.id}`}
                  >
                    <CirclePlay
                      size={15}
                      strokeWidth={1.8}
                      aria-hidden="true"
                    />
                    {job.media_kind === "audio" ? "Play" : "View"}
                  </button>
                  <!-- eslint-disable svelte/no-navigation-without-resolve -- authenticated API file response -->
                  <a
                    class="ui-button ui-button--secondary"
                    href={youtubeDownloadFileUrl(job.id)}
                    download
                    data-od-id={`download-file-${job.id}`}
                    ><Download size={15} strokeWidth={1.8} aria-hidden="true" /> Download</a
                  >
                  <!-- eslint-enable svelte/no-navigation-without-resolve -->
                {:else if job.status === "failed" || job.status === "cancelled"}
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    onclick={() => void retryJob(job)}
                    disabled={busyJobId === job.id}
                    ><RotateCcw
                      size={15}
                      strokeWidth={1.8}
                      aria-hidden="true"
                    /> Retry</button
                  >
                {/if}
                <button
                  class="ui-button ui-button--danger"
                  type="button"
                  onclick={() => void removeJob(job)}
                  disabled={busyJobId === job.id}
                  data-od-id={`download-delete-${job.id}`}
                >
                  <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
                  {confirmingDelete === job.id ? "Confirm delete" : "Delete"}
                </button>
              </div>
            </article>
          {/each}
        </div>
        {#if !historyExhausted}
          <button
            class="ui-button ui-button--secondary downloads-load-more"
            type="button"
            onclick={() => void loadOlder()}
            disabled={loadingOlder}
            >{loadingOlder ? "Loading" : "Load older"}</button
          >
        {/if}
      {:else}
        <div class="downloads-empty">
          <HardDrive size={24} strokeWidth={1.5} aria-hidden="true" />
          <h3>No matching downloads</h3>
          <p>Completed files and settled attempts will appear here.</p>
        </div>
      {/if}
    </section>
  {:else if view === "policy" && viewerRole === "administrator"}
    <section class="downloads-policy" data-od-id="downloads-policy-view">
      {#if policyBusy && !policy}
        <div class="downloads-loading" role="status">
          Loading instance policy…
        </div>
      {:else if policy}
        <div
          class="downloads-policy-status"
          data-od-id="downloads-policy-toolchain"
        >
          <div>
            <span>[ TOOLCHAIN ]</span>
            <h3>{policy.capability.available ? "Ready" : "Unavailable"}</h3>
          </div>
          <dl>
            <div>
              <dt>yt-dlp</dt>
              <dd title={policy.capability.yt_dlp_version ?? undefined}>
                {compactToolVersion(policy.capability.yt_dlp_version)}
              </dd>
            </div>
            <div>
              <dt>FFmpeg</dt>
              <dd title={policy.capability.ffmpeg_version ?? undefined}>
                {compactToolVersion(policy.capability.ffmpeg_version)}
              </dd>
            </div>
            <div>
              <dt>ffprobe</dt>
              <dd title={policy.capability.ffprobe_version ?? undefined}>
                {compactToolVersion(policy.capability.ffprobe_version)}
              </dd>
            </div>
            <div>
              <dt>Deno</dt>
              <dd title={policy.capability.deno_version ?? undefined}>
                {compactToolVersion(policy.capability.deno_version)}
              </dd>
            </div>
            <div>
              <dt>Storage</dt>
              <dd>{formatBytes(policy.storage_used_bytes)} used</dd>
            </div>
          </dl>
          {#if policy.capability.unavailable_reason}<p>
              {policy.capability.unavailable_reason}
            </p>{/if}
        </div>

        <form
          class="downloads-policy-form"
          onsubmit={(event) => {
            event.preventDefault();
            void savePolicy();
          }}
        >
          <button
            class="ui-toggle-button downloads-policy-toggle"
            type="button"
            aria-pressed={policy.member_downloads_enabled}
            onclick={() =>
              (policy = policy
                ? {
                    ...policy,
                    member_downloads_enabled: !policy.member_downloads_enabled,
                  }
                : policy)}
            data-od-id="downloads-policy-member-toggle"
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span
              ><strong>Member downloads</strong><small
                >Administrators remain able to create jobs when disabled.</small
              ></span
            >
          </button>
          <fieldset
            class="downloads-policy-group"
            data-od-id="downloads-policy-storage"
          >
            <legend>[ STORAGE LIMITS ]</legend>
            <div class="downloads-policy-grid">
              <label
                ><span>Instance storage · GiB</span><input
                  type="number"
                  min="1"
                  step="1"
                  value={policy.storage_budget_bytes / 1073741824}
                  oninput={(event) =>
                    updatePolicyNumber(
                      "storage_budget_bytes",
                      event.currentTarget.value,
                      1073741824,
                    )}
                /></label
              >
              <label
                ><span>Per-account storage · GiB</span><input
                  type="number"
                  min="1"
                  step="1"
                  value={policy.per_user_budget_bytes / 1073741824}
                  oninput={(event) =>
                    updatePolicyNumber(
                      "per_user_budget_bytes",
                      event.currentTarget.value,
                      1073741824,
                    )}
                /></label
              >
              <label
                ><span>Maximum output · GiB</span><input
                  type="number"
                  min="0.1"
                  step="0.1"
                  value={policy.max_output_bytes / 1073741824}
                  oninput={(event) =>
                    updatePolicyNumber(
                      "max_output_bytes",
                      event.currentTarget.value,
                      1073741824,
                    )}
                /></label
              >
            </div>
          </fieldset>
          <fieldset
            class="downloads-policy-group downloads-policy-group--queue"
            data-od-id="downloads-policy-queue-limits"
          >
            <legend>[ QUEUE LIMITS ]</legend>
            <div class="downloads-policy-grid">
              <label
                ><span>Global concurrency</span><input
                  type="number"
                  min="1"
                  max="8"
                  step="1"
                  value={policy.global_concurrency}
                  oninput={(event) =>
                    updatePolicyNumber(
                      "global_concurrency",
                      event.currentTarget.value,
                    )}
                /></label
              >
              <label
                ><span>Per-account concurrency</span><input
                  type="number"
                  min="1"
                  max="4"
                  step="1"
                  value={policy.per_user_concurrency}
                  oninput={(event) =>
                    updatePolicyNumber(
                      "per_user_concurrency",
                      event.currentTarget.value,
                    )}
                /></label
              >
              <label
                ><span>Maximum batch URLs</span><input
                  type="number"
                  min="1"
                  max="50"
                  step="1"
                  value={policy.max_batch_urls}
                  oninput={(event) =>
                    updatePolicyNumber(
                      "max_batch_urls",
                      event.currentTarget.value,
                    )}
                /></label
              >
              <label
                ><span>Queued per account</span><input
                  type="number"
                  min="1"
                  max="200"
                  step="1"
                  value={policy.max_queued_per_user}
                  oninput={(event) =>
                    updatePolicyNumber(
                      "max_queued_per_user",
                      event.currentTarget.value,
                    )}
                /></label
              >
            </div>
          </fieldset>
          <div class="downloads-policy-actions">
            <div class="downloads-policy-feedback">
              {#if policyError}<p class="downloads-inline-error" role="alert">
                  {policyError}
                </p>{/if}
              {#if policySaved}<p
                  class="downloads-inline-success"
                  role="status"
                >
                  {policySaved}
                </p>{/if}
            </div>
            <button
              class="ui-button ui-button--primary downloads-policy-save"
              type="submit"
              disabled={policyBusy}
              data-od-id="downloads-policy-save"
              >{policyBusy ? "Saving" : "Save policy"}</button
            >
          </div>
        </form>
      {:else}
        <div class="downloads-empty">
          <AlertTriangle size={24} strokeWidth={1.5} aria-hidden="true" />
          <h3>Policy unavailable</h3>
          <p>{policyError || "The instance policy could not be loaded."}</p>
          <button
            class="ui-button ui-button--secondary"
            type="button"
            onclick={() => void loadPolicy()}>Try again</button
          >
        </div>
      {/if}
    </section>
  {/if}
</section>

<dialog
  class="ui-dialog downloads-media-dialog"
  aria-labelledby="downloads-preview-title"
  {@attach capturePreviewDialog}
  onclose={resetPreview}
  onclick={(event) => event.target === previewDialog && closePreview()}
  data-od-id="downloads-media-dialog"
>
  {#if previewJob}
    <header class="downloads-media-header">
      <div>
        <span>[ VIDEO.PLAYER ]</span>
        <h2 id="downloads-preview-title">
          {previewJob.title || "Untitled YouTube video"}
        </h2>
        <p>
          {previewJob.channel_name || "YouTube"} · {profileLabel(previewJob)} · {formatDuration(
            previewJob.duration_seconds,
          )}
        </p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close video viewer"
        onclick={closePreview}
        data-od-id="downloads-media-close"
      >
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </header>

    <div class="downloads-media-body">
      <div class="downloads-media-surface">
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          controls
          playsinline
          preload="metadata"
          src={youtubeDownloadPreviewUrl(previewJob.id)}
          aria-label={`View ${previewJob.title || "downloaded video"}`}
          onloadedmetadata={() => (previewError = "")}
          onerror={() =>
            (previewError =
              "This browser could not play the downloaded video or codec.")}
          data-od-id="downloads-video-player"
        ></video>
      </div>
      {#if previewError}
        <p class="downloads-media-error" role="alert">
          {previewError} You can still download the original file.
        </p>
      {/if}
    </div>

    <footer class="downloads-media-footer">
      <p>Playback stays private and streams from this Pandan instance.</p>
      <div>
        <!-- eslint-disable svelte/no-navigation-without-resolve -- authenticated API file response -->
        <a
          class="ui-button ui-button--secondary"
          href={youtubeDownloadFileUrl(previewJob.id)}
          download
          data-od-id="downloads-media-download"
          ><Download size={15} strokeWidth={1.8} aria-hidden="true" /> Download file</a
        >
        <!-- eslint-enable svelte/no-navigation-without-resolve -->
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={closePreview}>Close</button
        >
      </div>
    </footer>
  {/if}
</dialog>

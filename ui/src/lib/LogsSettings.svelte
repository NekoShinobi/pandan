<script lang="ts">
  import { onMount } from "svelte";
  import {
    fetchLogs,
    updateLoggingSettings,
    type LogEntry,
    type LogLevel,
    type LoggingSnapshot,
  } from "$lib/api";

  const levelOptions: Array<{ value: LogLevel; label: string }> = [
    { value: "error", label: "Error" },
    { value: "warn", label: "Warning" },
    { value: "info", label: "Info" },
    { value: "debug", label: "Debug" },
    { value: "trace", label: "Trace" },
  ];

  let snapshot = $state.raw<LoggingSnapshot | null>(null);
  let fileEnabled = $state(true);
  let logLevel = $state<LogLevel>("info");
  let retentionDays = $state(14);
  let maxFileSizeMb = $state(10);
  let maxFiles = $state(20);
  let loading = $state(true);
  let refreshing = $state(false);
  let saving = $state(false);
  let error = $state("");
  let savedMessage = $state("");
  let query = $state("");
  let levelFilter = $state<"all" | LogLevel>("all");

  const filteredEntries = $derived.by(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return (snapshot?.entries ?? []).filter((entry) => {
      if (levelFilter !== "all" && entry.level !== levelFilter) return false;
      if (!normalizedQuery) return true;
      return [
        entry.message,
        entry.target,
        entry.file,
        JSON.stringify(entry.fields),
      ].some((value) => value.toLocaleLowerCase().includes(normalizedQuery));
    });
  });

  onMount(() => {
    void loadLogs(false);
  });

  async function loadLogs(manual: boolean) {
    if (manual) refreshing = true;
    else loading = true;
    error = "";
    try {
      const next = await fetchLogs(200);
      snapshot = next;
      fileEnabled = next.settings.file_enabled;
      logLevel = next.settings.log_level;
      retentionDays = next.settings.retention_days;
      maxFileSizeMb = next.settings.max_file_size_mb;
      maxFiles = next.settings.max_files;
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to load instance logs";
    } finally {
      loading = false;
      refreshing = false;
    }
  }

  async function saveSettings(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true;
    error = "";
    savedMessage = "";
    try {
      const settings = await updateLoggingSettings({
        file_enabled: fileEnabled,
        log_level: logLevel,
        retention_days: retentionDays,
        max_file_size_mb: maxFileSizeMb,
        max_files: maxFiles,
      });
      if (snapshot) snapshot = { ...snapshot, settings };
      savedMessage = "Logging settings saved.";
      await loadLogs(true);
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to save logging settings";
    } finally {
      saving = false;
    }
  }

  function formatBytes(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function formatTimestamp(value: string) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat(undefined, {
      month: "short",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    }).format(date);
  }

  function fieldsFor(entry: LogEntry) {
    return Object.keys(entry.fields).length
      ? JSON.stringify(entry.fields, null, 2)
      : "";
  }
</script>

<div class="logs-settings" data-od-id="logs-settings">
  <section
    class="authentication-policy logging-policy"
    aria-labelledby="logging-policy-title"
    data-od-id="logging-policy"
  >
    <div class="authentication-policy-heading">
      <div>
        <p class="widget-kicker">[ LOGGING POLICY ]</p>
        <h3 id="logging-policy-title">Persistent diagnostics</h3>
      </div>
      <span>{fileEnabled ? `${logLevel} and above` : "File output off"}</span>
    </div>

    <p class="logging-intro">
      Pandan writes structured JSON lines on a background thread. Rotation
      limits individual files; retention removes older rotated files. Sensitive
      credentials, request bodies, authorization headers, query strings, and
      complete source URLs are excluded from application events.
    </p>

    <form class="logging-form" onsubmit={saveSettings}>
      <div class="authentication-policy-row">
        <span>
          <strong id="file-logging-label">Persistent file logging</strong>
          <small id="file-logging-description">
            Keep diagnostics across server restarts. Console output remains
            controlled by the operator's RUST_LOG setting.
          </small>
        </span>
        <button
          class="ui-toggle-button authentication-policy-toggle"
          type="button"
          aria-pressed={fileEnabled}
          aria-labelledby="file-logging-label"
          aria-describedby="file-logging-description"
          disabled={saving}
          onclick={() => (fileEnabled = !fileEnabled)}
          data-od-id="file-logging-enabled"
        >
          <span class="ui-toggle-indicator" aria-hidden="true"></span>
        </button>
      </div>

      <div class="logging-fields">
        <label>
          <span>Minimum level</span>
          <select
            class="select-input"
            bind:value={logLevel}
            disabled={saving}
            data-od-id="logging-level"
          >
            {#each levelOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
          <small
            >Use Debug or Trace temporarily; both can produce substantial
            volume.</small
          >
        </label>
        <label>
          <span>Retention days</span>
          <input
            class="text-input"
            type="number"
            min="1"
            max="365"
            bind:value={retentionDays}
            disabled={saving}
            data-od-id="logging-retention-days"
          />
          <small>Rotated files older than this are deleted.</small>
        </label>
        <label>
          <span>Rotate at</span>
          <div class="number-with-unit">
            <input
              class="text-input"
              type="number"
              min="1"
              max="256"
              bind:value={maxFileSizeMb}
              disabled={saving}
              data-od-id="logging-max-file-size"
            />
            <span>MB</span>
          </div>
          <small
            >The active file rotates before the next event crosses this size.</small
          >
        </label>
        <label>
          <span>Rotated files kept</span>
          <input
            class="text-input"
            type="number"
            min="1"
            max="100"
            bind:value={maxFiles}
            disabled={saving}
            data-od-id="logging-max-files"
          />
          <small>The count limit applies in addition to age retention.</small>
        </label>
      </div>

      {#if error}
        <p class="form-error logging-message" role="alert">{error}</p>
      {:else if savedMessage}
        <p class="logging-message saved" role="status">{savedMessage}</p>
      {/if}

      <div class="authentication-policy-actions">
        <span>Changes apply to new file events immediately.</span>
        <button
          class="ui-button ui-button--primary"
          type="submit"
          disabled={saving || loading}
          data-od-id="save-logging-settings"
        >
          {saving ? "Saving…" : "Save logging settings"}
        </button>
      </div>
    </form>
  </section>

  <section
    class="authentication-policy log-viewer"
    aria-labelledby="log-viewer-title"
    data-od-id="log-viewer"
  >
    <div class="authentication-policy-heading log-viewer-heading">
      <div>
        <p class="widget-kicker">[ INSTANCE LOGS ]</p>
        <h3 id="log-viewer-title">Recent events</h3>
      </div>
      <button
        class="ui-button ui-button--secondary"
        type="button"
        disabled={refreshing || loading}
        onclick={() => void loadLogs(true)}
        data-od-id="refresh-instance-logs"
      >
        {refreshing ? "Refreshing…" : "Refresh"}
      </button>
    </div>

    {#if snapshot}
      <dl class="log-storage" data-od-id="log-storage-status">
        <div>
          <dt>Stored</dt>
          <dd>{formatBytes(snapshot.storage.retained_bytes)}</dd>
        </div>
        <div>
          <dt>Active file</dt>
          <dd>{formatBytes(snapshot.storage.active_bytes)}</dd>
        </div>
        <div>
          <dt>Rotated</dt>
          <dd>{snapshot.storage.rotated_files}</dd>
        </div>
        <div>
          <dt>Dropped</dt>
          <dd>{snapshot.storage.dropped_entries}</dd>
        </div>
        <div class="storage-path">
          <dt>Directory</dt>
          <dd>{snapshot.storage.directory}</dd>
        </div>
      </dl>
      {#if snapshot.storage.last_error}
        <p class="form-error storage-error" role="alert">
          Writer warning: {snapshot.storage.last_error}
        </p>
      {/if}
    {/if}

    <div class="log-tools">
      <label>
        <span>Find in recent events</span>
        <input
          class="text-input"
          type="search"
          bind:value={query}
          placeholder="Message, target, or field"
          autocomplete="off"
          data-od-id="search-instance-logs"
        />
      </label>
      <label>
        <span>Level</span>
        <select
          class="select-input"
          bind:value={levelFilter}
          data-od-id="filter-instance-logs"
        >
          <option value="all">All levels</option>
          {#each levelOptions as option (option.value)}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      </label>
      <span class="log-result-count">
        {filteredEntries.length} of {snapshot?.entries.length ?? 0}
      </span>
    </div>

    {#if loading && !snapshot}
      <p class="log-empty" role="status">Loading persisted events…</p>
    {:else if snapshot && filteredEntries.length > 0}
      <div
        class="log-list overlay-scroll-region"
        data-od-id="instance-log-entries"
      >
        {#each filteredEntries as entry, index (entry.id)}
          <details
            class={["log-entry", `level-${entry.level}`]}
            data-od-id={`log-entry-${index + 1}`}
          >
            <summary>
              <time datetime={entry.timestamp}
                >{formatTimestamp(entry.timestamp)}</time
              >
              <span class="log-level">{entry.level}</span>
              <span class="log-message"
                >{entry.message || "Structured event"}</span
              >
              <span class="log-target">{entry.target}</span>
            </summary>
            <div class="log-entry-detail">
              <dl>
                <div>
                  <dt>File</dt>
                  <dd>{entry.file}</dd>
                </div>
                <div>
                  <dt>Target</dt>
                  <dd>{entry.target || "—"}</dd>
                </div>
                <div>
                  <dt>Timestamp</dt>
                  <dd>{entry.timestamp}</dd>
                </div>
              </dl>
              {#if fieldsFor(entry)}
                <pre>{fieldsFor(entry)}</pre>
              {/if}
            </div>
          </details>
        {/each}
      </div>
    {:else}
      <p class="log-empty">
        {snapshot?.entries.length
          ? "No recent events match these filters."
          : "No persisted events are available yet."}
      </p>
    {/if}
  </section>
</div>

<style>
  .logs-settings {
    display: grid;
    gap: 20px;
  }

  .logging-policy,
  .log-viewer {
    margin-bottom: 0;
  }

  .logging-intro {
    max-width: 78ch;
    margin: 0;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-size: 11px;
    line-height: 1.6;
  }

  .logging-form {
    display: grid;
  }

  .logging-fields {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 14px;
    padding: 16px;
    border-bottom: 1px solid var(--border);
  }

  .logging-fields > label,
  .log-tools > label {
    min-width: 0;
    display: grid;
    align-content: start;
    gap: 6px;
  }

  .logging-fields label > span,
  .log-tools label > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .logging-fields small {
    color: var(--muted);
    font-size: 10px;
    line-height: 1.45;
  }

  .number-with-unit {
    position: relative;
  }

  .number-with-unit input {
    padding-right: 48px;
  }

  .number-with-unit > span {
    position: absolute;
    top: 50%;
    right: 13px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.06em;
    transform: translateY(-50%);
  }

  .logging-message,
  .storage-error {
    margin: 12px 16px 0;
  }

  .logging-message.saved {
    color: var(--fg);
    font-size: 11px;
  }

  .authentication-policy-actions > span {
    color: var(--muted);
    font-size: 10px;
  }

  .log-storage {
    display: grid;
    grid-template-columns: repeat(4, minmax(100px, 1fr));
    margin: 0;
    border-bottom: 1px solid var(--border);
  }

  .log-storage > div {
    min-width: 0;
    padding: 12px 16px;
    border-right: 1px solid var(--border);
  }

  .log-storage > div:nth-child(4),
  .log-storage > div:last-child {
    border-right: 0;
  }

  .log-storage .storage-path {
    grid-column: 1 / -1;
    border-top: 1px solid var(--border);
  }

  .log-storage dt,
  .log-entry-detail dt {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .log-storage dd {
    min-width: 0;
    margin: 3px 0 0;
    overflow-wrap: anywhere;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .log-tools {
    display: grid;
    grid-template-columns: minmax(220px, 1fr) minmax(140px, 0.3fr) auto;
    align-items: end;
    gap: 12px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }

  .log-result-count {
    min-height: 44px;
    display: grid;
    place-items: center;
    padding: 0 12px;
    border: 1px solid var(--border);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .log-list {
    max-height: min(52vh, 620px);
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .log-entry {
    border-bottom: 1px solid var(--border);
  }

  .log-entry:last-child {
    border-bottom: 0;
  }

  .log-entry summary {
    min-height: 52px;
    display: grid;
    grid-template-columns: 126px 58px minmax(220px, 1fr) minmax(150px, 0.45fr);
    align-items: center;
    gap: 10px;
    padding: 8px 16px;
    color: var(--fg);
    cursor: pointer;
    list-style: none;
  }

  .log-entry summary::-webkit-details-marker {
    display: none;
  }

  .log-entry summary:hover {
    background: color-mix(in oklch, var(--fg) 7%, transparent);
  }

  .log-entry summary:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -3px;
  }

  .log-entry[open] summary {
    background: color-mix(in oklch, var(--fg) 8%, transparent);
  }

  .log-entry time,
  .log-level,
  .log-target {
    min-width: 0;
    overflow: hidden;
    font-family: var(--font-mono);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .log-entry time,
  .log-target {
    color: var(--muted);
  }

  .log-level {
    width: fit-content;
    padding: 3px 6px;
    border: 1px solid var(--border);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .level-error .log-level {
    border-color: color-mix(in oklch, var(--danger) 58%, var(--border));
    color: var(--danger);
  }

  .level-warn .log-level {
    border-color: color-mix(in oklch, var(--fg) 46%, var(--border));
    color: var(--fg);
  }

  .log-message {
    min-width: 0;
    overflow-wrap: anywhere;
    font-size: 12px;
    font-weight: 550;
  }

  .log-entry-detail {
    display: grid;
    gap: 12px;
    padding: 12px 16px 16px 244px;
    border-top: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 72%, transparent);
  }

  .log-entry-detail dl {
    display: grid;
    gap: 6px;
    margin: 0;
  }

  .log-entry-detail dl > div {
    display: grid;
    grid-template-columns: 76px minmax(0, 1fr);
    gap: 10px;
  }

  .log-entry-detail dd {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .log-entry-detail pre {
    max-height: 240px;
    margin: 0;
    overflow: auto;
    padding: 12px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--bg) 82%, transparent);
    color: var(--fg);
    font: 10px/1.6 var(--font-mono);
    scrollbar-gutter: stable;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .log-empty {
    margin: 0;
    padding: 36px 16px;
    color: var(--muted);
    font-size: 11px;
    text-align: center;
  }

  @media (max-width: 1020px) {
    .logging-fields {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .log-entry summary {
      grid-template-columns: 112px 54px minmax(180px, 1fr);
    }

    .log-target {
      grid-column: 3;
    }

    .log-entry-detail {
      padding-left: 16px;
    }
  }

  @media (max-width: 680px) {
    .logging-fields,
    .log-storage,
    .log-tools {
      grid-template-columns: 1fr;
    }

    .log-storage > div,
    .log-storage > div:nth-child(4) {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }

    .log-storage > div:last-child {
      border-bottom: 0;
    }

    .log-storage .storage-path {
      grid-column: auto;
      border-top: 0;
    }

    .log-entry summary {
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 5px 10px;
      padding-block: 12px;
    }

    .log-entry time,
    .log-message,
    .log-target {
      grid-column: 1;
    }

    .log-level {
      grid-column: 2;
      grid-row: 1;
    }

    .log-message,
    .log-target {
      white-space: normal;
    }

    .log-target {
      grid-row: 3;
    }

    .authentication-policy-actions {
      align-items: stretch;
      flex-direction: column;
    }

    .authentication-policy-actions .ui-button {
      width: 100%;
    }
  }

  @supports not (scrollbar-gutter: stable) {
    .log-list,
    .log-entry-detail pre {
      padding-right: var(--scrollbar-size);
    }
  }
</style>

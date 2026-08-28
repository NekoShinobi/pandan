<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    deleteJellyfinConfig,
    fetchJellyfinConfig,
    fetchJellyfinStatus,
    initiateJellyfinQuickConnect,
    linkJellyfinPassword,
    pollJellyfinQuickConnect,
    unlinkJellyfin,
    updateJellyfinConfig,
    verifyJellyfinLink,
    type JellyfinConfig,
    type JellyfinQuickConnect,
    type JellyfinStatus,
  } from "$lib/api";

  let { mode }: { mode: "admin" | "account" } = $props();

  let config = $state<JellyfinConfig | null>(null);
  let status = $state<JellyfinStatus | null>(null);
  let baseUrl = $state("");
  let username = $state("");
  let password = $state("");
  let quick = $state<JellyfinQuickConnect | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let action = $state("");
  let pendingDelete = $state(false);
  let error = $state("");
  let notice = $state("");
  let quickSecondsRemaining = $state(0);
  let quickExpiresAt = 0;
  let quickPollInFlight = false;
  let quickAttemptVersion = 0;
  let destroyed = false;
  let pollTimer: ReturnType<typeof setTimeout> | undefined;
  let countdownTimer: ReturnType<typeof setInterval> | undefined;

  onMount(() => {
    void load();
  });

  onDestroy(() => {
    destroyed = true;
    quickAttemptVersion += 1;
    clearQuickTimers();
  });

  async function load() {
    loading = true;
    error = "";
    try {
      if (mode === "admin") {
        config = await fetchJellyfinConfig();
        baseUrl = config.base_url ?? "";
      } else {
        status = await fetchJellyfinStatus();
      }
    } catch (reason: unknown) {
      error = message(reason, "Unable to load Jellyfin settings");
    } finally {
      loading = false;
    }
  }

  async function saveConfig(event: SubmitEvent) {
    event.preventDefault();
    if (saving || !baseUrl.trim()) return;
    saving = true;
    error = "";
    notice = "";
    try {
      config = await updateJellyfinConfig(baseUrl.trim());
      baseUrl = config.base_url ?? "";
      pendingDelete = false;
      notice =
        "Connected to " +
        (config.server_name ?? "Jellyfin") +
        ". Existing account links were cleared.";
    } catch (reason: unknown) {
      error = message(reason, "Unable to connect to Jellyfin");
    } finally {
      saving = false;
    }
  }

  async function removeConfig() {
    if (!pendingDelete) {
      pendingDelete = true;
      return;
    }
    action = "delete-config";
    error = "";
    notice = "";
    try {
      await deleteJellyfinConfig();
      config = {
        configured: false,
        base_url: null,
        server_id: null,
        server_name: null,
        server_version: null,
        secret_storage_enabled: config?.secret_storage_enabled ?? false,
      };
      baseUrl = "";
      pendingDelete = false;
      notice = "Jellyfin was disconnected and every account link was removed.";
    } catch (reason: unknown) {
      error = message(reason, "Unable to remove Jellyfin");
    } finally {
      action = "";
    }
  }

  async function startQuickConnect() {
    if (action) return;
    action = "quick";
    error = "";
    notice = "";
    try {
      beginQuickConnect(await initiateJellyfinQuickConnect());
    } catch (reason: unknown) {
      error = message(reason, "Unable to start Quick Connect");
    } finally {
      action = "";
    }
  }

  function beginQuickConnect(attempt: JellyfinQuickConnect) {
    quickAttemptVersion += 1;
    quick = attempt;
    syncQuickDeadline(attempt.expires_in_seconds);
    updateQuickCountdown();
    if (countdownTimer) clearInterval(countdownTimer);
    countdownTimer = setInterval(updateQuickCountdown, 250);
    schedulePoll(750);
  }

  function syncQuickDeadline(seconds: number) {
    quickExpiresAt = Date.now() + Math.max(0, seconds) * 1000;
  }

  function updateQuickCountdown() {
    if (!quick || quick.approved) {
      quickSecondsRemaining = 0;
      return;
    }
    quickSecondsRemaining = Math.max(
      0,
      Math.ceil((quickExpiresAt - Date.now()) / 1000),
    );
    if (quickSecondsRemaining === 0) expireQuickConnect();
  }

  function schedulePoll(delay = 1500) {
    if (pollTimer) clearTimeout(pollTimer);
    if (destroyed || !quick || quick.approved || quickSecondsRemaining <= 0) return;
    pollTimer = setTimeout(() => void checkQuickConnect(), delay);
  }

  async function checkQuickConnect() {
    if (
      destroyed ||
      quickPollInFlight ||
      !quick ||
      quickSecondsRemaining <= 0
    ) {
      return;
    }
    quickPollInFlight = true;
    const attemptVersion = quickAttemptVersion;
    try {
      const result = await pollJellyfinQuickConnect();
      if (destroyed || attemptVersion !== quickAttemptVersion) return;
      quick = result;
      if (result.approved) {
        clearQuickTimers();
        quickSecondsRemaining = 0;
        notice = "Jellyfin account linked.";
        await load();
        return;
      }
      error = "";
      syncQuickDeadline(result.expires_in_seconds);
      updateQuickCountdown();
      schedulePoll();
    } catch {
      if (!destroyed && quickSecondsRemaining > 0) {
        error = "Approval status is temporarily unavailable. Retrying automatically.";
        schedulePoll(3000);
      }
    } finally {
      quickPollInFlight = false;
      if (
        !destroyed &&
        attemptVersion !== quickAttemptVersion &&
        quick &&
        quickSecondsRemaining > 0
      ) {
        schedulePoll(0);
      }
    }
  }

  function expireQuickConnect() {
    if (!quick) return;
    quickAttemptVersion += 1;
    clearQuickTimers();
    quick = null;
    quickSecondsRemaining = 0;
    error = "Quick Connect code expired. Start a new request.";
  }

  function clearQuickTimers() {
    if (pollTimer) clearTimeout(pollTimer);
    if (countdownTimer) clearInterval(countdownTimer);
    pollTimer = undefined;
    countdownTimer = undefined;
  }

  function formatCountdown(seconds: number) {
    const minutes = Math.floor(seconds / 60);
    return `${minutes}:${String(seconds % 60).padStart(2, "0")}`;
  }

  async function linkWithPassword(event: SubmitEvent) {
    event.preventDefault();
    if (action || !username.trim() || !password) return;
    quickAttemptVersion += 1;
    clearQuickTimers();
    quick = null;
    quickSecondsRemaining = 0;
    action = "password";
    error = "";
    notice = "";
    try {
      await linkJellyfinPassword(username.trim(), password);
      password = "";
      notice = "Jellyfin account linked.";
      await load();
    } catch (reason: unknown) {
      error = message(reason, "Unable to link Jellyfin account");
    } finally {
      password = "";
      action = "";
    }
  }

  async function verify() {
    if (action) return;
    action = "verify";
    error = "";
    notice = "";
    try {
      await verifyJellyfinLink();
      notice = "Jellyfin connection verified.";
      await load();
    } catch (reason: unknown) {
      error = message(reason, "Unable to verify Jellyfin");
      await load();
    } finally {
      action = "";
    }
  }

  async function disconnect() {
    if (!pendingDelete) {
      pendingDelete = true;
      return;
    }
    action = "disconnect";
    error = "";
    notice = "";
    try {
      await unlinkJellyfin();
      quick = null;
      pendingDelete = false;
      notice = "Jellyfin account disconnected.";
      await load();
    } catch (reason: unknown) {
      error = message(reason, "Unable to disconnect Jellyfin");
    } finally {
      action = "";
    }
  }

  function message(reason: unknown, fallback: string) {
    return reason instanceof Error ? reason.message : fallback;
  }
</script>

<section
  class="authentication-policy jellyfin-settings"
  aria-labelledby={"jellyfin-" + mode + "-title"}
  data-od-id={"jellyfin-" + mode + "-settings"}
>
  <div class="authentication-policy-heading">
    <div>
      <p class="widget-kicker">[ JELLYFIN MUSIC ]</p>
      <h3 id={"jellyfin-" + mode + "-title"}>
        {mode === "admin" ? "Media server" : "Music account"}
      </h3>
    </div>
    {#if mode === "admin" && config?.configured}
      <span>Connected</span>
    {:else if mode === "account" && status?.connected}
      <span>Linked</span>
    {/if}
  </div>

  {#if loading}
    <p class="network-access-empty" role="status">Loading Jellyfin settings…</p>
  {:else if mode === "admin"}
    <p class="network-access-intro">
      Connect this Pandan instance to one Jellyfin server. Private-network or
      HTTP servers also need an exact Jellyfin allow rule below.
    </p>
    <form class="network-access-form jellyfin-config-form" onsubmit={saveConfig}>
      <label class="network-access-origin">
        <span>Jellyfin base URL</span>
        <input
          class="text-input"
          type="url"
          bind:value={baseUrl}
          placeholder="https://media.example.com/jellyfin"
          maxlength="2000"
          required
          autocomplete="url"
          spellcheck="false"
          data-od-id="jellyfin-base-url"
        />
      </label>
      <button
        class="ui-button ui-button--primary"
        type="submit"
        disabled={saving || !baseUrl.trim()}
        data-od-id="save-jellyfin-config"
      >
        {saving ? "Checking…" : config?.configured ? "Reconnect" : "Connect"}
      </button>
    </form>
    {#if config?.configured}
      <div class="jellyfin-connection-summary">
        <div>
          <strong>{config.server_name}</strong>
          <small>Jellyfin {config.server_version}</small>
        </div>
        <button
          class="ui-button ui-button--danger"
          type="button"
          disabled={action !== ""}
          onclick={() => void removeConfig()}
          data-od-id="remove-jellyfin-config"
        >
          {action === "delete-config"
            ? "Removing…"
            : pendingDelete
              ? "Confirm remove"
              : "Remove server"}
        </button>
      </div>
    {/if}
    <p class="network-access-note">
      Reconnecting or removing the server clears every account link. Jellyfin
      tokens remain encrypted and never reach the browser.
    </p>
  {:else if !status?.configured}
    <p class="network-access-empty">
      An administrator must connect this instance to Jellyfin before accounts can link.
    </p>
  {:else if !status.secret_storage_enabled}
    <p class="form-error" role="alert">
      Encrypted credential storage is unavailable. Set the instance secret key
      before linking a Jellyfin account.
    </p>
  {:else if status.connected}
    <div class="jellyfin-connection-summary">
      <div>
        <strong>{status.jellyfin_username}</strong>
        <small>Connected to {status.server_name}</small>
      </div>
      <div class="jellyfin-settings-actions">
        <button
          class="ui-button ui-button--secondary"
          type="button"
          disabled={action !== ""}
          onclick={() => void verify()}
          data-od-id="verify-jellyfin-link"
        >
          {action === "verify" ? "Checking…" : "Verify"}
        </button>
        <button
          class="ui-button ui-button--danger"
          type="button"
          disabled={action !== ""}
          onclick={() => void disconnect()}
          data-od-id="disconnect-jellyfin-link"
        >
          {action === "disconnect"
            ? "Disconnecting…"
            : pendingDelete
              ? "Confirm disconnect"
              : "Disconnect"}
        </button>
      </div>
    </div>
    {#if status.last_error}
      <p class="form-error" role="alert">{status.last_error}</p>
    {/if}
  {:else}
    <p class="network-access-intro">
      Link your own Jellyfin identity. Pandan receives a token after approval;
      your password is never stored.
    </p>
    {#if quick}
      <div class="jellyfin-quick-code">
        <span>Enter in Jellyfin Quick Connect</span>
        <strong>{quick.code}</strong>
        <div class="jellyfin-quick-status">
          <small role="status">Waiting for approval…</small>
          <time datetime={`PT${quickSecondsRemaining}S`}>
            {formatCountdown(quickSecondsRemaining)}
          </time>
        </div>
      </div>
    {:else}
      <button
        class="ui-button ui-button--primary"
        type="button"
        disabled={action !== ""}
        onclick={() => void startQuickConnect()}
        data-od-id="start-jellyfin-quick-connect"
      >
        {action === "quick" ? "Starting…" : "Link with Quick Connect"}
      </button>
    {/if}

    <details class="jellyfin-password-fallback">
      <summary>Use username and password instead</summary>
      <form class="profile-form" onsubmit={linkWithPassword}>
        <label>
          <span>Jellyfin username</span>
          <input
            class="text-input"
            bind:value={username}
            maxlength="120"
            autocomplete="username"
            required
            data-od-id="jellyfin-username"
          />
        </label>
        <label>
          <span>Jellyfin password</span>
          <input
            class="text-input"
            type="password"
            bind:value={password}
            maxlength="1000"
            autocomplete="current-password"
            required
            data-od-id="jellyfin-password"
          />
        </label>
        <button
          class="ui-button ui-button--secondary"
          type="submit"
          disabled={action !== "" || !username.trim() || !password}
          data-od-id="link-jellyfin-password"
        >
          {action === "password" ? "Linking…" : "Link account"}
        </button>
      </form>
    </details>
  {/if}

  {#if notice}
    <p class="settings-page-notice" role="status">{notice}</p>
  {/if}
  {#if error}
    <p class="form-error" role="alert">{error}</p>
  {/if}
</section>

<style>
  .jellyfin-settings {
    display: grid;
    gap: 18px;
  }

  .jellyfin-config-form {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .jellyfin-connection-summary {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    min-width: 0;
    padding: 14px 0;
    border-block: 1px solid var(--border);
  }

  .jellyfin-connection-summary > div:first-child {
    display: grid;
    min-width: 0;
    gap: 4px;
  }

  .jellyfin-connection-summary strong,
  .jellyfin-connection-summary small {
    overflow-wrap: anywhere;
  }

  .jellyfin-connection-summary strong {
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 590;
  }

  .jellyfin-connection-summary small,
  .jellyfin-quick-code small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .jellyfin-settings-actions {
    display: flex;
    flex: 0 0 auto;
    gap: 8px;
  }

  .jellyfin-quick-code {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px 18px;
    padding: 18px;
    border: 1px solid var(--border);
    background: var(--fg-soft);
  }

  .jellyfin-quick-code > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .jellyfin-quick-code strong {
    grid-row: span 2;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: clamp(28px, 5vw, 44px);
    font-weight: 590;
    letter-spacing: 0.08em;
  }

  .jellyfin-quick-status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-width: min(220px, 100%);
  }

  .jellyfin-quick-status small::before {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 7px;
    border: 1px solid currentColor;
    border-radius: 50%;
    background: var(--fg);
    content: "";
  }

  .jellyfin-quick-status time {
    min-width: 4ch;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 16px;
    font-variant-numeric: tabular-nums;
    font-weight: 590;
    text-align: right;
  }

  .jellyfin-password-fallback {
    border-top: 1px solid var(--border);
    padding-top: 14px;
  }

  .jellyfin-password-fallback summary {
    min-height: 44px;
    color: var(--fg);
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 44px;
  }

  .jellyfin-password-fallback .profile-form {
    margin-top: 12px;
  }

  @media (max-width: 720px) {
    .jellyfin-config-form {
      grid-template-columns: 1fr;
    }

    .jellyfin-connection-summary {
      align-items: stretch;
      flex-direction: column;
    }

    .jellyfin-settings-actions {
      flex-wrap: wrap;
    }

    .jellyfin-quick-code {
      grid-template-columns: 1fr;
    }

    .jellyfin-quick-code strong {
      grid-row: auto;
    }
  }
</style>

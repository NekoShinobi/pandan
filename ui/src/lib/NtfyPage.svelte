<script lang="ts">
  import Bell from "lucide-svelte/icons/bell";
  import Clipboard from "lucide-svelte/icons/clipboard";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import Settings from "lucide-svelte/icons/settings";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import NtfyPriority from "$lib/NtfyPriority.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import { presentNtfyTags } from "$lib/ntfyPresentation";
  import {
    createNtfyTopic,
    deleteNtfyConnection,
    deleteNtfyNotification,
    deleteNtfyTopic,
    executeNtfyAction,
    fetchNtfy,
    markNtfySeen,
    openNtfyEventStream,
    updateNtfyConnection,
    updateNtfyTopic,
    type NtfyAction,
    type NtfyNotification,
    type NtfyRealtimeEvent,
    type NtfyResponse,
    type NtfyTopic,
  } from "$lib/api";

  let {
    focusNotificationId = "",
    onConfigurationChanged = () => {},
  }: {
    focusNotificationId?: string;
    onConfigurationChanged?: () => void;
  } = $props();

  let data = $state.raw<NtfyResponse | null>(null);
  let loading = $state(false);
  let error = $state("");
  let selectedTopicId = $state("");
  let settingsDialog = $state<HTMLDialogElement>();
  let serverUrl = $state("https://ntfy.sh");
  let token = $state("");
  let clearToken = $state(false);
  let savingConnection = $state(false);
  let connectionError = $state("");
  let disconnectPending = $state(false);
  let topicName = $state("");
  let topicLabel = $state("");
  let addingTopic = $state(false);
  let topicError = $state("");
  let deletingTopicId = $state("");
  let editingTopicId = $state("");
  let editingTopicLabel = $state("");
  let actionKey = $state("");
  const dismissingIds = new SvelteSet<string>();
  let hasEverLoaded = false;
  let loadPending = false;

  onMount(() => {
    let events: EventSource | undefined;
    let disposed = false;
    void load().finally(() => {
      if (disposed) return;
      events = openNtfyEventStream();
      events.onopen = () => void load();
      events.onmessage = receiveRealtimeNotification;
    });
    return () => {
      disposed = true;
      events?.close();
    };
  });

  function captureSettingsDialog(node: HTMLDialogElement) {
    settingsDialog = node;
    return () => {
      if (settingsDialog === node) settingsDialog = undefined;
    };
  }

  async function load() {
    if (loadPending) return;
    loadPending = true;
    if (!hasEverLoaded) loading = true;
    try {
      data = await fetchNtfy({
        topic_id: selectedTopicId || undefined,
        limit: 200,
      });
      hasEverLoaded = true;
      error = "";
      if ((data.unread_count ?? 0) > 0) {
        await markNtfySeen();
        data = { ...data, unread_count: 0 };
      }
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to load ntfy notifications";
    } finally {
      loadPending = false;
      loading = false;
    }
  }

  async function selectTopic(id: string) {
    if (selectedTopicId === id) return;
    selectedTopicId = id;
    await load();
  }

  function receiveRealtimeNotification(event: MessageEvent<string>) {
    let realtime: NtfyRealtimeEvent;
    try {
      realtime = JSON.parse(event.data) as NtfyRealtimeEvent;
    } catch {
      return;
    }
    if (realtime.kind === "status") {
      if (data?.connection) {
        data = {
          ...data,
          connection: { ...data.connection, last_error: realtime.last_error },
        };
      } else {
        void load();
      }
      return;
    }
    if (!data) {
      void load();
      return;
    }
    if (realtime.kind === "deleted") {
      data = {
        ...data,
        notifications: data.notifications.filter(
          (notification) => notification.id !== realtime.notification_id,
        ),
        unread_count: 0,
      };
      return;
    }
    const notification = realtime.notification;
    if (selectedTopicId && notification.topic_id !== selectedTopicId) return;
    data = {
      ...data,
      notifications: [
        notification,
        ...data.notifications.filter((item) => item.id !== notification.id),
      ].slice(0, 200),
      unread_count: 0,
    };
    void markNtfySeen();
  }

  function openSettings() {
    serverUrl = data?.connection?.base_url ?? "https://ntfy.sh";
    token = "";
    clearToken = false;
    connectionError = "";
    disconnectPending = false;
    settingsDialog?.showModal();
  }

  async function saveConnection(event: SubmitEvent) {
    event.preventDefault();
    if (savingConnection) return;
    savingConnection = true;
    connectionError = "";
    try {
      data = await updateNtfyConnection({
        base_url: serverUrl,
        token: token.trim() || undefined,
        clear_token: clearToken,
      });
      onConfigurationChanged();
      settingsDialog?.close();
      token = "";
      clearToken = false;
      await load();
    } catch (reason: unknown) {
      connectionError =
        reason instanceof Error
          ? reason.message
          : "Unable to save ntfy connection";
    } finally {
      savingConnection = false;
    }
  }

  async function disconnect() {
    if (!disconnectPending) {
      disconnectPending = true;
      return;
    }
    savingConnection = true;
    try {
      await deleteNtfyConnection();
      data = data
        ? {
            ...data,
            connection: null,
            topics: [],
            notifications: [],
            unread_count: 0,
          }
        : null;
      selectedTopicId = "";
      onConfigurationChanged();
      settingsDialog?.close();
    } catch (reason: unknown) {
      connectionError =
        reason instanceof Error ? reason.message : "Unable to disconnect ntfy";
    } finally {
      savingConnection = false;
      disconnectPending = false;
    }
  }

  async function addTopic(event: SubmitEvent) {
    event.preventDefault();
    if (addingTopic) return;
    addingTopic = true;
    topicError = "";
    try {
      const topic = await createNtfyTopic({
        topic: topicName,
        label: topicLabel,
      });
      if (data) data = { ...data, topics: [...data.topics, topic] };
      topicName = "";
      topicLabel = "";
      selectedTopicId = topic.id;
      onConfigurationChanged();
      await load();
    } catch (reason: unknown) {
      topicError =
        reason instanceof Error ? reason.message : "Unable to add topic";
    } finally {
      addingTopic = false;
    }
  }

  function beginTopicEdit(topic: NtfyTopic) {
    editingTopicId = topic.id;
    editingTopicLabel = topic.label;
    deletingTopicId = "";
  }

  async function saveTopicLabel(topic: NtfyTopic) {
    if (!editingTopicLabel.trim()) return;
    try {
      const updated = await updateNtfyTopic(topic.id, editingTopicLabel);
      if (data)
        data = {
          ...data,
          topics: data.topics.map((item) =>
            item.id === updated.id ? updated : item,
          ),
        };
      editingTopicId = "";
      onConfigurationChanged();
    } catch (reason: unknown) {
      topicError =
        reason instanceof Error ? reason.message : "Unable to rename topic";
    }
  }

  async function removeTopic(topic: NtfyTopic) {
    if (deletingTopicId !== topic.id) {
      deletingTopicId = topic.id;
      return;
    }
    try {
      await deleteNtfyTopic(topic.id);
      if (data) {
        data = {
          ...data,
          topics: data.topics.filter((item) => item.id !== topic.id),
          notifications: data.notifications.filter(
            (item) => item.topic_id !== topic.id,
          ),
        };
      }
      if (selectedTopicId === topic.id) selectedTopicId = "";
      deletingTopicId = "";
      onConfigurationChanged();
    } catch (reason: unknown) {
      topicError =
        reason instanceof Error ? reason.message : "Unable to remove topic";
    }
  }

  async function removeNotification(notification: NtfyNotification) {
    if (dismissingIds.has(notification.id)) return;
    dismissingIds.add(notification.id);
    try {
      const request = deleteNtfyNotification(notification.id);
      await Promise.all([request, motionDelay()]);
      if (data)
        data = {
          ...data,
          notifications: data.notifications.filter(
            (item) => item.id !== notification.id,
          ),
        };
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to delete notification";
    } finally {
      dismissingIds.delete(notification.id);
    }
  }

  async function runAction(
    notification: NtfyNotification,
    action: NtfyAction,
    index: number,
  ) {
    const key = `${notification.id}:${index}`;
    if (actionKey) return;
    actionKey = key;
    try {
      if (action.action === "copy") {
        await navigator.clipboard.writeText(action.value ?? "");
        if (action.clear) await deleteAfterAction(notification);
      } else if (action.action === "http") {
        const result = await executeNtfyAction(notification.id, index);
        if (result.deleted && data) {
          data = {
            ...data,
            notifications: data.notifications.filter(
              (item) => item.id !== notification.id,
            ),
          };
        }
      }
    } catch (reason: unknown) {
      error =
        reason instanceof Error ? reason.message : "Unable to run ntfy action";
    } finally {
      actionKey = "";
    }
  }

  async function deleteAfterAction(notification: NtfyNotification) {
    try {
      await deleteNtfyNotification(notification.id);
      if (data)
        data = {
          ...data,
          notifications: data.notifications.filter(
            (item) => item.id !== notification.id,
          ),
        };
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to delete notification";
    }
  }

  function motionDelay() {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches)
      return Promise.resolve();
    return new Promise<void>((resolve) => window.setTimeout(resolve, 170));
  }

  function safeUrl(value: string | null | undefined) {
    if (!value) return null;
    try {
      const url = new URL(value);
      return url.protocol === "http:" || url.protocol === "https:"
        ? url.href
        : null;
    } catch {
      return null;
    }
  }

  function formatTime(timestamp: number) {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(new Date(timestamp * 1_000));
  }
</script>

<section class="ntfy-page product-page" data-od-id="ntfy-notifications-page">
  <div class="ntfy-header page-header">
    <div>
      <TypedHeading text="$ notifications --inbox" odId="ntfy-heading" />
      <p>
        Messages arrive through Pandan's persistent ntfy connection and remain
        available until you delete them.
      </p>
    </div>
    <div class="ntfy-header-actions">
      <button
        class="ui-button ui-button--primary"
        type="button"
        onclick={openSettings}
        data-od-id="configure-ntfy"
      >
        <Settings size={16} strokeWidth={1.8} />
        Connection
      </button>
    </div>
  </div>

  <div class="ntfy-toolbar">
    <span>[ REALTIME ]</span>
    <span>{data?.connection?.base_url ?? "Not connected"}</span>
  </div>

  {#if error}
    <p class="ntfy-error" role="alert">{error}</p>
  {/if}
  {#if data?.connection?.last_error}
    <p class="ntfy-sync-error" role="status">
      Realtime status: {data.connection.last_error}
    </p>
  {/if}

  {#if loading && !data}
    <div class="ntfy-loading" aria-label="Loading notifications">
      {#each Array(5) as index (index)}<i></i>{/each}
    </div>
  {:else if !data?.connection}
    <div class="ntfy-empty large" data-od-id="ntfy-connect-empty-state">
      <Settings size={30} strokeWidth={1.4} aria-hidden="true" />
      <h3>Connect your ntfy server</h3>
      <p>
        Use ntfy.sh or a public HTTPS ntfy server. Tokens are encrypted before
        they are stored.
      </p>
      <button
        class="ui-button ui-button--primary"
        type="button"
        onclick={openSettings}>Set up ntfy</button
      >
    </div>
  {:else}
    <div class="ntfy-layout">
      <aside class="ntfy-topic-rail" data-od-id="ntfy-topic-subscriptions">
        <header>
          <div>
            <span>[ TOPICS ]</span><strong>{data.topics.length}/{32}</strong>
          </div>
          <button
            class="ui-button ui-button--ghost ui-button--icon"
            type="button"
            aria-label="Show notifications from all topics"
            aria-pressed={selectedTopicId === ""}
            onclick={() => selectTopic("")}
            ><Bell size={16} strokeWidth={1.8} /></button
          >
        </header>
        <div class="ntfy-topic-list">
          <button
            class="ntfy-topic-row all"
            type="button"
            aria-pressed={selectedTopicId === ""}
            onclick={() => selectTopic("")}
          >
            <span><strong>All topics</strong><small>Combined inbox</small></span
            >
            <b>{data.notifications.length}</b>
          </button>
          {#each data.topics as topic (topic.id)}
            <div class="ntfy-topic-record">
              {#if editingTopicId === topic.id}
                <form
                  onsubmit={(event) => {
                    event.preventDefault();
                    void saveTopicLabel(topic);
                  }}
                >
                  <label class="sr-only" for={`ntfy-topic-label-${topic.id}`}
                    >Topic label</label
                  >
                  <input
                    id={`ntfy-topic-label-${topic.id}`}
                    bind:value={editingTopicLabel}
                    maxlength="80"
                    required
                  />
                  <button class="ui-button ui-button--secondary" type="submit"
                    >Save</button
                  >
                  <button
                    class="ui-button ui-button--ghost ui-button--icon"
                    type="button"
                    aria-label="Cancel rename"
                    onclick={() => (editingTopicId = "")}
                    ><X size={15} /></button
                  >
                </form>
              {:else}
                <button
                  class="ntfy-topic-row"
                  type="button"
                  aria-pressed={selectedTopicId === topic.id}
                  onclick={() => selectTopic(topic.id)}
                >
                  <span
                    ><strong>{topic.label}</strong><small>{topic.topic}</small
                    ></span
                  >
                </button>
                <div class="ntfy-topic-actions">
                  <button
                    class="ui-button ui-button--ghost ui-button--icon"
                    type="button"
                    aria-label={`Rename ${topic.label}`}
                    onclick={() => beginTopicEdit(topic)}
                    ><Pencil size={14} /></button
                  >
                  <button
                    class={[
                      "ui-button",
                      "ui-button--danger",
                      "ui-button--icon",
                      deletingTopicId === topic.id && "confirm",
                    ]}
                    type="button"
                    aria-label={deletingTopicId === topic.id
                      ? `Confirm removal of ${topic.label}`
                      : `Remove ${topic.label}`}
                    onclick={() => removeTopic(topic)}
                    ><Trash2 size={14} /></button
                  >
                </div>
              {/if}
            </div>
          {/each}
        </div>
        <form class="ntfy-topic-form" onsubmit={addTopic}>
          <label for="ntfy-topic-name">Topic</label>
          <input
            id="ntfy-topic-name"
            bind:value={topicName}
            maxlength="64"
            placeholder="home-alerts"
            required
          />
          <label for="ntfy-topic-label"
            >Display label <small>Optional</small></label
          >
          <input
            id="ntfy-topic-label"
            bind:value={topicLabel}
            maxlength="80"
            placeholder="Home alerts"
          />
          {#if topicError}<p role="alert">{topicError}</p>{/if}
          <button
            class="ui-button ui-button--secondary"
            type="submit"
            disabled={addingTopic || data.topics.length >= 32}
            ><Plus size={15} />{addingTopic ? "Adding…" : "Add topic"}</button
          >
        </form>
      </aside>

      <section class="ntfy-feed" data-od-id="ntfy-inbox-feed">
        {#each data.notifications as notification (notification.id)}
          {@const destination = safeUrl(notification.click_url)}
          {@const tagPresentation = presentNtfyTags(notification.tags)}
          <article
            class={[
              dismissingIds.has(notification.id) && "is-dismissing",
              focusNotificationId === notification.id && "is-focused",
            ]}
            data-priority={notification.priority}
            aria-current={focusNotificationId === notification.id
              ? "true"
              : undefined}
            data-od-id={`ntfy-notification-${notification.id}`}
          >
            <div class="ntfy-notification-meta">
              <span>{notification.topic_label}</span>
              <time
                datetime={new Date(
                  notification.occurred_at * 1_000,
                ).toISOString()}>{formatTime(notification.occurred_at)}</time
              >
            </div>
            <div class="ntfy-notification-body">
              <div class="ntfy-notification-title">
                <NtfyPriority priority={notification.priority} />
                {#if tagPresentation.emojiTags.length}
                  <div class="ntfy-tag-emojis">
                    {#each tagPresentation.emojiTags as emojiTag, index (`${emojiTag.tag}:${index}`)}
                      <span role="img" aria-label={emojiTag.tag}
                        >{emojiTag.emoji}</span
                      >
                    {/each}
                  </div>
                {/if}
                <h3>{notification.title}</h3>
              </div>
              {#if notification.message}<p>{notification.message}</p>{/if}
              {#if tagPresentation.textTags.length}
                <div class="ntfy-tags">
                  {#each tagPresentation.textTags as tag, index (`${tag}:${index}`)}
                    <span>{tag}</span>
                  {/each}
                </div>
              {/if}
            </div>
            <div class="ntfy-notification-actions">
              {#if destination}
                <a
                  class="ui-button ui-button--secondary"
                  href={destination}
                  target="_blank"
                  rel="external noopener noreferrer"
                  >Open <ExternalLink size={14} /></a
                >
              {/if}
              {#each notification.actions as action, index (`${notification.id}:${index}`)}
                {@const actionUrl =
                  action.action === "view" ? safeUrl(action.url) : null}
                {#if actionUrl}
                  <a
                    class="ui-button ui-button--secondary"
                    href={actionUrl}
                    target="_blank"
                    rel="external noopener noreferrer"
                    onclick={() => {
                      if (action.clear) void deleteAfterAction(notification);
                    }}>{action.label || "Open"}<ExternalLink size={14} /></a
                  >
                {:else if action.action === "copy"}
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    disabled={Boolean(actionKey)}
                    onclick={() => runAction(notification, action, index)}
                    >{action.label || "Copy"}<Clipboard size={14} /></button
                  >
                {:else if action.action === "http"}
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    disabled={Boolean(actionKey)}
                    onclick={() => runAction(notification, action, index)}
                    >{actionKey === `${notification.id}:${index}`
                      ? "Running…"
                      : action.label || "Run action"}</button
                  >
                {/if}
              {/each}
              <button
                class="ui-button ui-button--danger dismiss-notification"
                type="button"
                disabled={dismissingIds.has(notification.id)}
                onclick={() => removeNotification(notification)}
              >
                <Trash2 size={14} />Delete
              </button>
            </div>
          </article>
        {:else}
          <div class="ntfy-empty">
            <Bell size={28} strokeWidth={1.4} />
            <h3>Inbox clear</h3>
            <p>
              {data.topics.length
                ? "No notifications match this topic."
                : "Add a topic to begin retrieving ntfy messages."}
            </p>
          </div>
        {/each}
      </section>
    </div>
  {/if}

  <dialog
    class="ui-dialog ntfy-dialog"
    {@attach captureSettingsDialog}
    onclick={(event) =>
      event.target === settingsDialog && settingsDialog?.close()}
    data-od-id="ntfy-connection-dialog"
  >
    <header>
      <div>
        <span>[ NTFY.CONNECTION ]</span>
        <h2>Connect a server</h2>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close ntfy connection settings"
        onclick={() => settingsDialog?.close()}><X size={18} /></button
      >
    </header>
    <form onsubmit={saveConnection}>
      <label for="ntfy-server-url">Server URL</label>
      <input
        id="ntfy-server-url"
        type="url"
        bind:value={serverUrl}
        maxlength="2048"
        placeholder="https://ntfy.sh"
        required
      />
      <p class="field-help">
        Custom servers must use a public, credential-free HTTPS address. One
        server is connected per Pandan account.
      </p>
      <label for="ntfy-access-token"
        >Access token <small
          >{data?.connection?.has_token
            ? "Stored token will be retained when blank"
            : "Optional for public topics"}</small
        ></label
      >
      <input
        id="ntfy-access-token"
        type="password"
        bind:value={token}
        maxlength="4096"
        autocomplete="new-password"
        placeholder={data?.connection?.has_token ? "••••••••••••" : "tk_…"}
        disabled={!data?.secret_storage_enabled && !data?.connection?.has_token}
      />
      {#if !data?.secret_storage_enabled}
        <p class="field-help">
          Set PANDAN_SECRET_KEY on the server to save an ntfy access token.
          Public topics remain available without one.
        </p>
      {/if}
      {#if data?.connection?.has_token}
        <button
          class="ui-toggle-button"
          type="button"
          aria-pressed={clearToken}
          onclick={() => {
            clearToken = !clearToken;
            if (clearToken) token = "";
          }}
        >
          <span class="ui-toggle-indicator" aria-hidden="true"></span><span
            >Remove stored token</span
          >
        </button>
      {/if}
      {#if connectionError}<p class="ntfy-dialog-error" role="alert">
          {connectionError}
        </p>{/if}
      <footer>
        {#if data?.connection}<button
            class="ui-button ui-button--danger"
            type="button"
            disabled={savingConnection}
            onclick={disconnect}
            >{disconnectPending ? "Confirm disconnect" : "Disconnect"}</button
          >{/if}
        <span></span>
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={() => settingsDialog?.close()}>Cancel</button
        >
        <button
          class="ui-button ui-button--primary"
          type="submit"
          disabled={savingConnection}
          >{savingConnection ? "Saving…" : "Save connection"}</button
        >
      </footer>
    </form>
  </dialog>
</section>

<style>
  .ntfy-page {
    display: flex;
    height: var(--product-view-height);
    min-height: 0;
    flex-direction: column;
    gap: 16px;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    padding: clamp(24px, 3vw, 42px);
  }
  .ntfy-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  .ntfy-header p {
    max-width: 68ch;
    margin: 7px 0 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.6;
  }
  .ntfy-header-actions {
    display: flex;
    gap: 8px;
  }
  .ntfy-header-actions .ui-button {
    gap: 8px;
  }
  .ntfy-toolbar {
    display: flex;
    min-height: 46px;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    padding: 5px;
  }
  .ntfy-toolbar > span {
    overflow: hidden;
    padding-right: 10px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ntfy-error,
  .ntfy-sync-error {
    margin: 0;
    border: 1px solid var(--border);
    padding: 10px 12px;
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.5;
  }
  .ntfy-error {
    color: var(--danger, var(--fg));
  }
  .ntfy-sync-error {
    color: var(--muted);
  }
  .ntfy-layout {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-columns: 290px minmax(0, 1fr);
    gap: 16px;
  }
  .ntfy-topic-rail,
  .ntfy-feed {
    min-width: 0;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
  }
  .ntfy-topic-rail {
    display: flex;
    min-height: 560px;
    flex-direction: column;
  }
  .ntfy-topic-rail > header {
    display: flex;
    min-height: 58px;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px 8px 14px;
    border-bottom: 1px solid var(--border);
  }
  .ntfy-topic-rail > header > div {
    display: flex;
    align-items: baseline;
    gap: 9px;
  }
  .ntfy-topic-rail > header span,
  .ntfy-topic-rail > header strong {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.07em;
  }
  .ntfy-topic-rail > header span {
    color: var(--muted);
  }
  .ntfy-topic-list {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    scrollbar-gutter: stable;
  }
  .ntfy-topic-record {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    border-bottom: 1px solid var(--border);
  }
  .ntfy-topic-row {
    display: flex;
    width: 100%;
    min-height: 58px;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    border: 0;
    background: transparent;
    color: var(--fg);
    padding: 8px 12px;
    text-align: left;
  }
  .ntfy-topic-row.all {
    border-bottom: 1px solid var(--border);
  }
  .ntfy-topic-row[aria-pressed="true"] {
    background: var(--fg-soft);
  }
  .ntfy-topic-row:hover {
    background: color-mix(in oklch, var(--fg) 8%, transparent);
  }
  .ntfy-topic-row span {
    display: grid;
    min-width: 0;
    gap: 4px;
  }
  .ntfy-topic-row strong {
    overflow: hidden;
    font-size: 12px;
    font-weight: 550;
    text-overflow: ellipsis;
  }
  .ntfy-topic-row small,
  .ntfy-topic-row b {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 450;
  }
  .ntfy-topic-actions {
    display: flex;
    align-items: center;
    padding-right: 5px;
  }
  .ntfy-topic-actions .confirm {
    color: var(--danger, var(--fg));
  }
  .ntfy-topic-record form {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 5px;
    padding: 7px;
  }
  .ntfy-topic-record input,
  .ntfy-topic-form input,
  .ntfy-dialog input {
    min-height: 42px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    padding: 0 10px;
    outline: 0;
  }
  .ntfy-topic-form {
    display: grid;
    gap: 7px;
    padding: 14px;
    border-top: 1px solid var(--border);
  }
  .ntfy-topic-form label,
  .ntfy-dialog label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.03em;
  }
  .ntfy-topic-form label small,
  .ntfy-dialog label small {
    color: var(--muted);
    font-weight: 400;
  }
  .ntfy-topic-form p {
    margin: 0;
    color: var(--danger, var(--fg));
    font-family: var(--font-mono);
    font-size: 9px;
    line-height: 1.5;
  }
  .ntfy-topic-form button {
    justify-self: start;
    gap: 7px;
    margin-top: 3px;
  }
  .ntfy-feed {
    overflow-y: auto;
    scrollbar-gutter: stable;
  }
  .ntfy-feed > article {
    padding: 18px;
    border-bottom: 1px solid var(--border);
    transition:
      opacity 150ms var(--ease-out),
      transform 150ms var(--ease-out);
  }
  .ntfy-feed > article.is-dismissing {
    opacity: 0;
    transform: translateX(24px);
  }
  .ntfy-feed > article.is-focused {
    outline: 1px solid color-mix(in oklch, var(--accent) 58%, var(--border));
    outline-offset: -4px;
    background: var(--accent-soft);
  }
  .ntfy-notification-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .ntfy-notification-body {
    max-width: 76ch;
    padding: 13px 0;
  }
  .ntfy-notification-title {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .ntfy-tag-emojis {
    display: inline-flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    gap: 3px;
    font-size: 17px;
    line-height: 1.35;
  }
  .ntfy-notification-body h3 {
    min-width: 0;
    margin: 0;
    font-size: 17px;
    font-weight: 580;
    line-height: 1.35;
  }
  .ntfy-notification-body p {
    margin: 7px 0 0;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.65;
    white-space: pre-wrap;
  }
  .ntfy-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 12px;
  }
  .ntfy-tags span {
    border: 1px solid var(--border);
    padding: 3px 6px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 8px;
  }
  .ntfy-notification-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 7px;
  }
  .ntfy-notification-actions .ui-button {
    gap: 6px;
  }
  .dismiss-notification {
    margin-left: auto;
  }
  .ntfy-empty,
  .ntfy-loading {
    display: grid;
    min-height: 420px;
    place-content: center;
    justify-items: center;
    gap: 9px;
    color: var(--muted);
    text-align: center;
  }
  .ntfy-empty.large {
    flex: 1;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
  }
  .ntfy-empty h3 {
    margin: 5px 0 0;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 14px;
  }
  .ntfy-empty p {
    max-width: 46ch;
    margin: 0;
    font-size: 11px;
    line-height: 1.6;
  }
  .ntfy-empty button {
    margin-top: 8px;
  }
  .ntfy-loading {
    grid-template-columns: minmax(240px, 640px);
    align-content: center;
  }
  .ntfy-loading i {
    width: 100%;
    height: 58px;
    border: 1px solid var(--border);
    background: var(--fg-soft);
  }
  .ntfy-dialog {
    width: min(620px, calc(100vw - 24px));
  }
  .ntfy-dialog > header {
    display: flex;
    min-height: 72px;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 12px 16px 12px 22px;
    border-bottom: 1px solid var(--border);
  }
  .ntfy-dialog > header > div {
    display: grid;
    gap: 5px;
  }
  .ntfy-dialog > header span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
  }
  .ntfy-dialog h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 580;
  }
  .ntfy-dialog form {
    display: grid;
    gap: 9px;
    padding: 22px;
  }
  .ntfy-dialog .field-help {
    margin: -2px 0 7px;
    color: var(--muted);
    font-size: 10px;
    line-height: 1.55;
  }
  .ntfy-dialog .ui-toggle-button {
    width: fit-content;
    margin-top: 4px;
  }
  .ntfy-dialog-error {
    margin: 5px 0 0;
    border: 1px solid var(--border);
    padding: 9px 10px;
    color: var(--danger, var(--fg));
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .ntfy-dialog footer {
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    gap: 8px;
    margin-top: 10px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  input:focus-visible,
  button:focus-visible,
  a:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  @media (max-width: 820px) {
    .ntfy-header {
      align-items: stretch;
      flex-direction: column;
    }
    .ntfy-header-actions {
      justify-content: flex-start;
    }
    .ntfy-layout {
      grid-template-columns: 1fr;
    }
    .ntfy-topic-rail {
      min-height: auto;
    }
    .ntfy-topic-list {
      max-height: 280px;
    }
    .ntfy-feed {
      max-height: none;
      overflow: visible;
    }
  }
  @media (max-width: 560px) {
    .ntfy-page {
      padding: 18px 14px 28px;
    }
    .ntfy-header-actions,
    .ntfy-notification-actions {
      align-items: stretch;
      flex-direction: column;
    }
    .ntfy-header-actions .ui-button,
    .ntfy-notification-actions .ui-button {
      width: 100%;
      justify-content: center;
    }
    .dismiss-notification {
      margin-left: 0;
    }
    .ntfy-toolbar {
      align-items: stretch;
      flex-direction: column;
    }
    .ntfy-toolbar > span {
      padding: 3px 8px 6px;
    }
    .ntfy-notification-meta {
      align-items: flex-start;
      flex-direction: column;
    }
    .ntfy-dialog footer {
      grid-template-columns: 1fr;
    }
    .ntfy-dialog footer span {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ntfy-feed > article {
      transition: opacity 100ms linear;
    }
    .ntfy-feed > article.is-dismissing {
      transform: none;
    }
  }
</style>

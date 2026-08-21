<script lang="ts">
  import Bell from "lucide-svelte/icons/bell";
  import ExternalLink from "lucide-svelte/icons/external-link";
  import Settings from "lucide-svelte/icons/settings";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { motionPopover } from "$lib/motion.svelte";
  import NtfyPriority from "$lib/NtfyPriority.svelte";
  import { presentNtfyTags } from "$lib/ntfyPresentation";
  import {
    fetchNtfy,
    markNtfySeen,
    openNtfyEventStream,
    deleteNtfyNotification,
    type NtfyNotification,
    type NtfyRealtimeEvent,
    type NtfyResponse,
  } from "$lib/api";

  const AUTO_SYNC_INTERVAL_MS = 5 * 60 * 1_000;
  const SWIPE_THRESHOLD_PX = 72;
  const SWIPE_MAX_OFFSET_PX = 136;
  const SWIPE_EXIT_MS = 170;

  let {
    onOpenAll,
    onNotification,
    onToast,
  }: {
    onOpenAll: (notificationId?: string) => void;
    onNotification: (notification: NtfyNotification, count: number) => void;
    onToast: (message: string) => void;
  } = $props();

  let open = $state(false);
  let loading = $state(false);
  let error = $state("");
  let response = $state.raw<NtfyResponse | null>(null);
  let popover = $state<HTMLDivElement>();
  const knownRemoteIds = new SvelteSet<string>();
  const dismissedPreviewIds = new SvelteSet<string>();
  let swipingId = $state("");
  let swipeOffset = $state(0);
  let swipeLeaving = $state(false);
  let swipeStartX = 0;
  let swipeStartY = 0;
  let swipePointerId: number | null = null;
  let swipeAxis: "pending" | "horizontal" | "vertical" = "pending";
  let suppressClicksUntil = 0;
  let hasBaseline = false;

  onMount(() => {
    let disposed = false;
    let events: EventSource | undefined;
    void load().finally(() => {
      if (disposed) return;
      events = openNtfyEventStream();
      events.onopen = () => void load();
      events.onmessage = receiveRealtimeNotification;
    });
    const timer = window.setInterval(
      () => void load(),
      AUTO_SYNC_INTERVAL_MS,
    );
    return () => {
      disposed = true;
      events?.close();
      window.clearInterval(timer);
    };
  });

  function capturePopover(node: HTMLDivElement) {
    popover = node;
    return () => {
      if (popover === node) popover = undefined;
    };
  }

  async function load() {
    if (loading) return;
    loading = true;
    try {
      const next = await fetchNtfy({ limit: 5 });
      const unseen = next.notifications.filter(
        (notification) => !knownRemoteIds.has(notification.remote_id),
      );
      next.notifications.forEach((notification) =>
        knownRemoteIds.add(notification.remote_id),
      );
      response = {
        ...next,
        notifications: next.notifications.filter(
          (notification) => !dismissedPreviewIds.has(notification.id),
        ),
      };
      if (hasBaseline && unseen.length) announceNotifications(unseen);
      hasBaseline = true;
      error = "";
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to load notifications";
    } finally {
      loading = false;
    }
  }

  async function toggle() {
    open = !open;
    if (!open) return;
    await load();
    if ((response?.unread_count ?? 0) > 0) {
      try {
        await markNtfySeen();
        if (response) response = { ...response, unread_count: 0 };
      } catch {
        // The preview remains usable even if seen state could not be persisted.
      }
    }
  }

  function receiveRealtimeNotification(event: MessageEvent<string>) {
    let realtime: NtfyRealtimeEvent;
    try {
      realtime = JSON.parse(event.data) as NtfyRealtimeEvent;
    } catch {
      return;
    }
    if (realtime.kind === "status") {
      if (response?.connection) {
        response = {
          ...response,
          connection: {
            ...response.connection,
            last_error: realtime.last_error,
          },
        };
      } else {
        void load();
      }
      return;
    }
    if (realtime.kind === "deleted") {
      removePreviewNotification(realtime.notification_id);
      if (response)
        response = { ...response, unread_count: realtime.unread_count };
      return;
    }
    const notification = realtime.notification;
    if (!notification?.remote_id || knownRemoteIds.has(notification.remote_id))
      return;
    knownRemoteIds.add(notification.remote_id);
    if (response) {
      response = {
        ...response,
        notifications: [
          notification,
          ...response.notifications.filter(
            (item) => item.remote_id !== notification.remote_id,
          ),
        ].slice(0, 5),
        unread_count: open ? 0 : realtime.unread_count,
      };
    } else {
      void load();
    }
    announceNotifications([notification]);
    if (open) void markNtfySeen();
  }

  function announceNotifications(notifications: NtfyNotification[]) {
    const newest = notifications[0];
    if (!newest) return;
    onNotification(newest, notifications.length);
  }

  function dismissPreview(notification: NtfyNotification) {
    dismissedPreviewIds.add(notification.id);
    removePreviewNotification(notification.id);
  }

  async function deleteNotification(notification: NtfyNotification) {
    const previousIndex =
      response?.notifications.findIndex((item) => item.id === notification.id) ??
      0;
    dismissedPreviewIds.add(notification.id);
    removePreviewNotification(notification.id);
    try {
      await deleteNtfyNotification(notification.id);
    } catch (reason: unknown) {
      dismissedPreviewIds.delete(notification.id);
      if (response && !response.notifications.some((item) => item.id === notification.id)) {
        const notifications = [...response.notifications];
        notifications.splice(
          Math.min(Math.max(previousIndex, 0), notifications.length),
          0,
          notification,
        );
        response = { ...response, notifications: notifications.slice(0, 5) };
      }
      onToast(
        reason instanceof Error
          ? reason.message
          : "Unable to delete notification",
      );
    }
  }

  function removePreviewNotification(notificationId: string) {
    if (!response) return;
    response = {
      ...response,
      notifications: response.notifications.filter(
        (item) => item.id !== notificationId,
      ),
    };
  }

  function beginSwipe(event: PointerEvent, notification: NtfyNotification) {
    if (
      !event.isPrimary ||
      (event.pointerType === "mouse" && event.button !== 0) ||
      (event.target as Element).closest(".ntfy-popover-actions") ||
      swipeLeaving
    )
      return;
    swipingId = notification.id;
    swipeOffset = 0;
    swipeStartX = event.clientX;
    swipeStartY = event.clientY;
    swipePointerId = event.pointerId;
    swipeAxis = "pending";
    (event.currentTarget as HTMLDivElement).setPointerCapture(event.pointerId);
  }

  function moveSwipe(event: PointerEvent, notification: NtfyNotification) {
    if (
      swipingId !== notification.id ||
      swipePointerId !== event.pointerId ||
      swipeLeaving
    )
      return;
    const deltaX = event.clientX - swipeStartX;
    const deltaY = event.clientY - swipeStartY;
    if (swipeAxis === "pending") {
      if (Math.max(Math.abs(deltaX), Math.abs(deltaY)) < 7) return;
      if (Math.abs(deltaY) > Math.abs(deltaX)) {
        swipeAxis = "vertical";
        resetSwipe();
        return;
      }
      swipeAxis = "horizontal";
    }
    if (swipeAxis !== "horizontal") return;
    event.preventDefault();
    swipeOffset = Math.sign(deltaX) * Math.min(Math.abs(deltaX), SWIPE_MAX_OFFSET_PX);
  }

  function finishSwipe(event: PointerEvent, notification: NtfyNotification) {
    if (swipingId !== notification.id || swipePointerId !== event.pointerId)
      return;
    releaseSwipeCapture(event);
    if (swipeAxis === "pending") {
      suppressClicksUntil = performance.now() + 450;
      resetSwipe();
      openAll(notification.id);
      return;
    }
    if (swipeAxis === "horizontal")
      suppressClicksUntil = performance.now() + 450;
    if (swipeAxis !== "horizontal" || Math.abs(swipeOffset) < SWIPE_THRESHOLD_PX) {
      resetSwipe();
      return;
    }
    void commitSwipe(
      notification,
      swipeOffset < 0 ? "left" : "right",
      (event.currentTarget as HTMLDivElement).clientWidth,
    );
  }

  function cancelSwipe(event: PointerEvent, notification: NtfyNotification) {
    if (swipingId !== notification.id || swipePointerId !== event.pointerId)
      return;
    releaseSwipeCapture(event);
    resetSwipe();
  }

  function releaseSwipeCapture(event: PointerEvent) {
    const target = event.currentTarget as HTMLDivElement;
    if (target.hasPointerCapture(event.pointerId))
      target.releasePointerCapture(event.pointerId);
  }

  async function commitSwipe(
    notification: NtfyNotification,
    direction: "left" | "right",
    width: number,
  ) {
    swipeLeaving = true;
    swipeOffset = direction === "left" ? -(width + 24) : width + 24;
    await swipeExitDelay();
    if (direction === "left") dismissPreview(notification);
    else await deleteNotification(notification);
    resetSwipe();
  }

  function resetSwipe() {
    swipingId = "";
    swipeOffset = 0;
    swipeLeaving = false;
    swipePointerId = null;
    swipeAxis = "pending";
  }

  function swipeExitDelay() {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches)
      return Promise.resolve();
    return new Promise<void>((resolve) =>
      window.setTimeout(resolve, SWIPE_EXIT_MS),
    );
  }

  function handleCardKeydown(
    event: KeyboardEvent,
    notification: NtfyNotification,
  ) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      openAll(notification.id);
      return;
    }
    if (!matchesSwipeKey(event.key) || swipeLeaving) return;
    event.preventDefault();
    const direction = event.key === "ArrowLeft" ? "left" : "right";
    const shell = event.currentTarget as HTMLDivElement;
    swipingId = notification.id;
    swipeOffset = direction === "left" ? -SWIPE_THRESHOLD_PX : SWIPE_THRESHOLD_PX;
    void commitSwipe(notification, direction, shell.clientWidth);
  }

  function matchesSwipeKey(key: string) {
    return key === "ArrowLeft" || key === "ArrowRight";
  }

  function guardDestinationClick(event: MouseEvent) {
    suppressSwipeClick(event);
  }

  function suppressSwipeClick(event: MouseEvent) {
    if (performance.now() >= suppressClicksUntil) return false;
    event.preventDefault();
    event.stopPropagation();
    return true;
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
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    }).format(new Date(timestamp * 1_000));
  }

  function openAll(notificationId?: string) {
    open = false;
    onOpenAll(notificationId);
  }
</script>

<div
  class="ntfy-popover-anchor"
  {@attach capturePopover}
  data-od-id="notification-center"
>
  <button
    class="ui-button ui-button--ghost ui-button--icon header-icon-button"
    class:has-unread={(response?.unread_count ?? 0) > 0}
    type="button"
    aria-label={(response?.unread_count ?? 0) > 0
      ? `${response?.unread_count} new notifications`
      : "Notifications"}
    aria-expanded={open}
    aria-controls="notification-preview"
    onclick={toggle}
    data-od-id="open-notification-center"
  >
    <Bell size={19} strokeWidth={1.7} aria-hidden="true" />
    {#if (response?.unread_count ?? 0) > 0}
      <span class="notification-count"
        >{Math.min(response?.unread_count ?? 0, 99)}</span
      >
    {/if}
  </button>

  <section
    id="notification-preview"
    class="ntfy-popover"
    aria-label="Recent notifications"
    aria-hidden={!open}
    inert={!open}
    data-od-id="notification-preview"
    {@attach motionPopover(open, { closedY: -6 })}
  >
    <header>
      <div>
        <span>[ NTFY.INBOX ]</span>
        <strong>Notifications</strong>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close notifications"
        onclick={() => (open = false)}><X size={17} strokeWidth={1.8} /></button
      >
    </header>

    {#if loading && !response}
      <div class="ntfy-popover-status">Checking subscribed topics…</div>
    {:else if error && !response}
      <div class="ntfy-popover-status error" role="alert">{error}</div>
    {:else if !response?.connection}
      <div class="ntfy-popover-empty">
        <Settings size={22} strokeWidth={1.5} aria-hidden="true" />
        <strong>Connect ntfy</strong>
        <p>
          Add your server, access token, and the topics you want Pandan to
          follow.
        </p>
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={() => openAll()}>Configure</button
        >
      </div>
    {:else}
      <div class="ntfy-popover-list overlay-scroll-region">
        {#each response.notifications as notification (notification.id)}
          {@const destination = safeUrl(notification.click_url)}
          {@const tagPresentation = presentNtfyTags(notification.tags)}
          {@const isSwiping = swipingId === notification.id}
          <div
            class={[
              "ntfy-popover-swipe-shell",
              isSwiping && "is-swiping",
              isSwiping && swipeLeaving && "is-leaving",
            ]}
            data-swipe-direction={isSwiping
              ? swipeOffset < 0
                ? "left"
                : swipeOffset > 0
                  ? "right"
                  : "none"
              : "none"}
            style:--swipe-progress={isSwiping
              ? Math.min(Math.abs(swipeOffset) / SWIPE_THRESHOLD_PX, 1)
              : 0}
            role="button"
            tabindex="0"
            aria-label={`Notification preview for ${notification.title}. Press Enter to view, swipe left to dismiss, or swipe right to delete.`}
            aria-keyshortcuts="Enter Space ArrowLeft ArrowRight"
            onpointerdown={(event) => beginSwipe(event, notification)}
            onpointermove={(event) => moveSwipe(event, notification)}
            onpointerup={(event) => finishSwipe(event, notification)}
            onpointercancel={(event) => cancelSwipe(event, notification)}
            onkeydown={(event) => handleCardKeydown(event, notification)}
          >
            <div class="ntfy-popover-swipe-actions" aria-hidden="true">
              <span class="delete"><Trash2 size={15} strokeWidth={1.8} />Delete</span>
              <span class="dismiss"><X size={15} strokeWidth={1.8} />Dismiss</span>
            </div>
            <article
              style:--swipe-offset={`${isSwiping ? swipeOffset : 0}px`}
              data-od-id={`notification-preview-${notification.id}`}
            >
              <div class="ntfy-popover-meta">
                <span>{notification.topic_label}</span>
                <time
                  datetime={new Date(
                    notification.occurred_at * 1_000,
                  ).toISOString()}>{formatTime(notification.occurred_at)}</time
                >
              </div>
              <div class="ntfy-popover-copy">
                <div class="ntfy-popover-title">
                  <NtfyPriority priority={notification.priority} />
                  {#if tagPresentation.emojiTags.length}
                    <span class="ntfy-popover-emojis">
                      {#each tagPresentation.emojiTags as emojiTag, index (`${emojiTag.tag}:${index}`)}
                        <span role="img" aria-label={emojiTag.tag}
                          >{emojiTag.emoji}</span
                        >
                      {/each}
                    </span>
                  {/if}
                  <strong>{notification.title}</strong>
                </div>
                {#if notification.message}<p>{notification.message}</p>{/if}
              </div>
              {#if destination}
                <div class="ntfy-popover-actions">
                  <a
                    class="ui-button ui-button--ghost ui-button--icon"
                    href={destination}
                    target="_blank"
                    rel="external noopener noreferrer"
                    aria-label={`Open linked destination for ${notification.title}`}
                    onclick={guardDestinationClick}
                  >
                    <ExternalLink size={15} strokeWidth={1.8} />
                  </a>
                </div>
              {/if}
            </article>
          </div>
        {:else}
          <div class="ntfy-popover-empty compact">
            <Bell size={20} strokeWidth={1.5} aria-hidden="true" />
            <strong>Inbox clear</strong>
            <p>New messages from subscribed topics will appear here.</p>
          </div>
        {/each}
      </div>
      {#if response.connection.last_error}
        <p class="ntfy-popover-sync-error" role="status">
          {response.connection.last_error}
        </p>
      {/if}
      <footer>
        <span
          >{response.topics.length}
          {response.topics.length === 1 ? "topic" : "topics"}</span
        >
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={() => openAll()}>See all</button
        >
      </footer>
    {/if}
  </section>
</div>

<svelte:document
  onpointerdown={(event) => {
    if (open && popover && !popover.contains(event.target as Node))
      open = false;
  }}
/>

<style>
  .ntfy-popover-anchor {
    position: relative;
  }
  .header-icon-button.has-unread {
    position: relative;
    color: var(--fg);
  }
  .notification-count {
    position: absolute;
    inset: 2px 1px auto auto;
    display: grid;
    min-width: 16px;
    height: 16px;
    place-items: center;
    border: 2px solid var(--bg);
    border-radius: 999px;
    background: var(--accent);
    color: var(--bg);
    font-family: var(--font-mono);
    font-size: 8px;
    font-weight: 600;
    line-height: 1;
  }
  .ntfy-popover {
    position: absolute;
    z-index: 40;
    top: calc(100% + 10px);
    right: 0;
    width: min(390px, calc(100vw - 28px));
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 96%, var(--bg));
    color: var(--fg);
    box-shadow: 0 24px 70px color-mix(in oklch, var(--bg) 78%, transparent);
    visibility: hidden;
    opacity: 0;
    pointer-events: none;
    transform: translateY(-6px);
    will-change: opacity, transform;
  }
  .ntfy-popover > header,
  .ntfy-popover > footer {
    display: flex;
    min-height: 58px;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 9px 12px;
  }
  .ntfy-popover > header {
    border-bottom: 1px solid var(--border);
  }
  .ntfy-popover > header > div {
    display: grid;
    gap: 4px;
  }
  .ntfy-popover > header span,
  .ntfy-popover > footer span,
  .ntfy-popover-meta {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .ntfy-popover > header strong {
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 550;
  }
  .ntfy-popover-list {
    max-height: min(460px, 60vh);
    overflow-y: auto;
  }
  .ntfy-popover-swipe-shell {
    --swipe-offset: 0px;
    --swipe-progress: 0;
    position: relative;
    overflow: hidden;
    border-bottom: 1px solid var(--border);
    touch-action: pan-y;
  }
  .ntfy-popover-swipe-actions {
    position: absolute;
    inset: 0;
    z-index: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--fg-soft);
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .ntfy-popover-swipe-actions span {
    display: inline-flex;
    min-width: 92px;
    height: 100%;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
    opacity: 0;
    transition: opacity 100ms var(--ease-out);
  }
  .ntfy-popover-swipe-actions .delete {
    justify-content: flex-start;
    color: var(--danger, var(--fg));
  }
  .ntfy-popover-swipe-actions .dismiss {
    justify-content: flex-end;
    color: var(--muted);
  }
  .ntfy-popover-swipe-shell[data-swipe-direction="right"]
    .ntfy-popover-swipe-actions
    .delete,
  .ntfy-popover-swipe-shell[data-swipe-direction="left"]
    .ntfy-popover-swipe-actions
    .dismiss {
    opacity: var(--swipe-progress);
  }
  .ntfy-popover-list article {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px 12px;
    padding: 13px 12px;
    background: color-mix(in oklch, var(--surface) 96%, var(--bg));
    cursor: pointer;
    transform: translateX(var(--swipe-offset));
    transition:
      background 100ms var(--ease-out),
      opacity 170ms var(--ease-out),
      transform 170ms var(--ease-out);
  }
  .ntfy-popover-swipe-shell:hover article {
    background: color-mix(in oklch, var(--fg) 5%, var(--surface));
  }
  .ntfy-popover-swipe-shell.is-swiping article {
    transition: none;
  }
  .ntfy-popover-swipe-shell.is-leaving article {
    opacity: 0.45;
  }
  .ntfy-popover-swipe-shell:focus-visible article {
    outline: 2px solid var(--accent);
    outline-offset: -3px;
  }
  .ntfy-popover-meta,
  .ntfy-popover-copy {
    grid-column: 1;
    min-width: 0;
  }
  .ntfy-popover-meta {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .ntfy-popover-title,
  .ntfy-popover-emojis {
    display: inline-flex;
    min-width: 0;
    align-items: center;
  }
  .ntfy-popover-copy {
    display: grid;
    gap: 4px;
  }
  .ntfy-popover-title {
    align-items: center;
    gap: 6px;
  }
  .ntfy-popover-emojis {
    flex: 0 0 auto;
    flex-wrap: wrap;
    gap: 2px;
    font-size: 13px;
    line-height: 1.45;
  }
  .ntfy-popover-copy strong {
    overflow-wrap: anywhere;
    font-size: 12px;
    font-weight: 550;
  }
  .ntfy-popover-copy p {
    display: -webkit-box;
    overflow: hidden;
    margin: 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.55;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }
  .ntfy-popover-actions {
    position: relative;
    z-index: 3;
    grid-column: 2;
    grid-row: 1 / span 2;
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .ntfy-popover > footer {
    border-top: 1px solid var(--border);
  }
  .ntfy-popover-status,
  .ntfy-popover-empty {
    display: grid;
    min-height: 180px;
    place-content: center;
    justify-items: center;
    gap: 8px;
    padding: 24px;
    color: var(--muted);
    text-align: center;
  }
  .ntfy-popover-status {
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .ntfy-popover-status.error,
  .ntfy-popover-sync-error {
    color: var(--danger, var(--fg));
  }
  .ntfy-popover-empty.compact {
    min-height: 150px;
  }
  .ntfy-popover-empty strong {
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
  }
  .ntfy-popover-empty p {
    max-width: 32ch;
    margin: 0;
    font-size: 11px;
    line-height: 1.6;
  }
  .ntfy-popover-sync-error {
    margin: 0;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: 9px;
    line-height: 1.5;
  }
  @media (max-width: 720px) {
    .ntfy-popover {
      position: fixed;
      top: 70px;
      right: 14px;
      left: 14px;
      width: auto;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ntfy-popover-swipe-actions span,
    .ntfy-popover-list article {
      transition: none;
    }
  }
</style>

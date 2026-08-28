<script lang="ts">
  import ChevronDown from "lucide-svelte/icons/chevron-down";
  import ChevronUp from "lucide-svelte/icons/chevron-up";
  import ListMusic from "lucide-svelte/icons/list-music";
  import Play from "lucide-svelte/icons/play";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import type { JellyfinMusicItem } from "$lib/api";
  import {
    formatPlaybackTime,
    podcastPlayer,
  } from "$lib/podcastPlayer.svelte";

  let {
    open,
    onClose,
  }: {
    open: boolean;
    onClose: () => void;
  } = $props();

  let dialog = $state<HTMLDialogElement>();

  function captureDialog(node: HTMLDialogElement) {
    dialog = node;
    return () => {
      if (dialog === node) dialog = undefined;
    };
  }

  $effect(() => {
    const node = dialog;
    if (!node) return;
    if (open && !node.open) node.showModal();
    else if (!open && node.open) node.close();
  });

  function close() {
    if (dialog?.open) dialog.close();
    else onClose();
  }

  async function playQueued(track: JellyfinMusicItem) {
    await podcastPlayer.playQueuedMusic(track);
    close();
  }

  function queueSubtitle(track: JellyfinMusicItem) {
    return [
      track.artist ?? "Unknown artist",
      track.album,
      formatPlaybackTime(track.duration_seconds ?? 0),
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<dialog
  class="ui-dialog music-queue-dialog"
  id="music-queue-dialog"
  data-od-id="music-queue-dialog"
  {@attach captureDialog}
  onclose={onClose}
  onclick={(event) => event.target === dialog && close()}
>
  <header>
    <div>
      <span>[ LOCAL.QUEUE ]</span>
      <h2>Up next</h2>
      <p>
        A session-local play backlog. Nothing here modifies a Jellyfin playlist.
      </p>
    </div>
    <button
      class="ui-button ui-button--ghost ui-button--icon"
      type="button"
      aria-label="Close music queue"
      onclick={close}
      data-od-id="close-music-queue"
    >
      <X size={18} strokeWidth={1.8} aria-hidden="true" />
    </button>
  </header>

  <div class="music-queue-body">
    {#if podcastPlayer.track}
      <section class="music-queue-now" aria-label="Now playing">
        <span>NOW PLAYING</span>
        <div>
          <strong>{podcastPlayer.track.name}</strong>
          <small>{podcastPlayer.subtitle}</small>
        </div>
      </section>
    {/if}

    {#if podcastPlayer.musicUpNext.length === 0}
      <div class="music-queue-empty">
        <ListMusic size={30} strokeWidth={1.4} aria-hidden="true" />
        <strong>The queue is empty</strong>
        <p>Use the list button beside any track to add it here.</p>
      </div>
    {:else}
      <ol class="music-queue-list" aria-label="Queued tracks">
        {#each podcastPlayer.musicUpNext as track, index (`${track.library_id}:${track.id}`)}
          <li data-od-id={"music-queue-item-" + track.id}>
            <span class="music-queue-position">{String(index + 1).padStart(2, "0")}</span>
            <div class="music-queue-copy">
              <strong>{track.name}</strong>
              <small>{queueSubtitle(track)}</small>
            </div>
            <div class="music-queue-actions">
              <button
                class="ui-button ui-button--ghost ui-button--icon"
                type="button"
                aria-label={"Play " + track.name + " now"}
                title="Play now"
                onclick={() => void playQueued(track)}
                data-od-id={"play-queued-music-" + track.id}
              >
                <Play size={16} fill="currentColor" aria-hidden="true" />
              </button>
              <button
                class="ui-button ui-button--ghost ui-button--icon"
                type="button"
                disabled={index === 0}
                aria-label={"Move " + track.name + " earlier"}
                title="Move earlier"
                onclick={() => podcastPlayer.moveQueuedMusic(track, -1)}
                data-od-id={"move-queued-music-earlier-" + track.id}
              >
                <ChevronUp size={17} strokeWidth={1.8} aria-hidden="true" />
              </button>
              <button
                class="ui-button ui-button--ghost ui-button--icon"
                type="button"
                disabled={index === podcastPlayer.musicUpNext.length - 1}
                aria-label={"Move " + track.name + " later"}
                title="Move later"
                onclick={() => podcastPlayer.moveQueuedMusic(track, 1)}
                data-od-id={"move-queued-music-later-" + track.id}
              >
                <ChevronDown size={17} strokeWidth={1.8} aria-hidden="true" />
              </button>
              <button
                class="ui-button ui-button--ghost ui-button--icon"
                type="button"
                aria-label={"Remove " + track.name + " from queue"}
                title="Remove from queue"
                onclick={() => podcastPlayer.removeQueuedMusic(track)}
                data-od-id={"remove-queued-music-" + track.id}
              >
                <Trash2 size={16} strokeWidth={1.8} aria-hidden="true" />
              </button>
            </div>
          </li>
        {/each}
      </ol>
    {/if}
  </div>

  <footer>
    <span>
      {podcastPlayer.musicUpNext.length}
      {podcastPlayer.musicUpNext.length === 1 ? "track" : "tracks"} waiting
    </span>
    <div>
      <button
        class="ui-button ui-button--danger"
        type="button"
        disabled={podcastPlayer.musicUpNext.length === 0}
        onclick={() => podcastPlayer.clearMusicQueue()}
        data-od-id="clear-music-queue"
      >
        Clear queue
      </button>
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={close}
        data-od-id="done-music-queue"
      >
        Done
      </button>
    </div>
  </footer>
</dialog>

<style>
  .music-queue-dialog {
    width: min(
      760px,
      calc(
        100vw - 24px - env(safe-area-inset-left) -
          env(safe-area-inset-right)
      )
    );
    max-height: calc(
      100dvh - max(28px, env(safe-area-inset-top)) -
        max(28px, env(safe-area-inset-bottom))
    );
    overflow: hidden;
  }

  .music-queue-dialog[open] {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
  }

  .music-queue-dialog > header {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 20px;
    padding: 20px;
    border-bottom: 1px solid var(--border);
  }

  .music-queue-dialog > header > div {
    min-width: 0;
  }

  .music-queue-dialog header span,
  .music-queue-now > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
  }

  .music-queue-dialog h2 {
    margin: 5px 0 0;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 22px;
    font-weight: 590;
  }

  .music-queue-dialog header p {
    max-width: 60ch;
    margin: 7px 0 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.6;
  }

  .music-queue-body {
    min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
  }

  .music-queue-now {
    display: grid;
    grid-template-columns: 92px minmax(0, 1fr);
    align-items: center;
    gap: 14px;
    padding: 14px 20px;
    border-bottom: 1px solid var(--border);
    background: var(--fg-soft);
  }

  .music-queue-now div,
  .music-queue-copy {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .music-queue-now strong,
  .music-queue-now small,
  .music-queue-copy strong,
  .music-queue-copy small {
    overflow: hidden;
    font-family: var(--font-mono);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .music-queue-now strong,
  .music-queue-copy strong {
    color: var(--fg);
    font-size: 12px;
    font-weight: 550;
  }

  .music-queue-now small,
  .music-queue-copy small {
    color: var(--muted);
    font-size: 9px;
  }

  .music-queue-list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .music-queue-list li {
    min-width: 0;
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    min-height: 66px;
    padding: 10px 16px 10px 20px;
    border-bottom: 1px solid var(--border);
  }

  .music-queue-position {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
  }

  .music-queue-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .music-queue-empty {
    min-height: 260px;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 8px;
    padding: 32px;
    color: var(--muted);
    text-align: center;
  }

  .music-queue-empty strong {
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 13px;
    font-weight: 550;
  }

  .music-queue-empty p {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.6;
  }

  .music-queue-dialog > footer {
    min-height: 66px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 16px 10px 20px;
    border-top: 1px solid var(--border);
  }

  .music-queue-dialog > footer > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .music-queue-dialog > footer > div {
    display: flex;
    gap: 8px;
  }

  @media (max-width: 560px) {
    .music-queue-dialog > header {
      padding: 16px;
    }

    .music-queue-now {
      grid-template-columns: 1fr;
      gap: 5px;
      padding-inline: 16px;
    }

    .music-queue-list li {
      grid-template-columns: 26px minmax(0, 1fr);
      padding-inline: 14px;
    }

    .music-queue-actions {
      grid-column: 2;
      justify-content: flex-start;
    }

    .music-queue-dialog > footer {
      align-items: stretch;
      flex-direction: column;
      padding: 12px 16px;
    }

    .music-queue-dialog > footer > div,
    .music-queue-dialog > footer button {
      flex: 1;
    }
  }

  @supports not (scrollbar-gutter: stable) {
    .music-queue-body {
      overflow-y: scroll;
    }
  }
</style>

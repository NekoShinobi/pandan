<script lang="ts">
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import FileText from "lucide-svelte/icons/file-text";
  import Globe2 from "lucide-svelte/icons/globe-2";
  import ImageIcon from "lucide-svelte/icons/image";
  import LockKeyhole from "lucide-svelte/icons/lock-keyhole";
  import MessageCircle from "lucide-svelte/icons/message-circle";
  import Paperclip from "lucide-svelte/icons/paperclip";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Send from "lucide-svelte/icons/send";
  import SmilePlus from "lucide-svelte/icons/smile-plus";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import {
    createLinePost,
    deleteLinePost,
    fetchLinePosts,
    linePostAttachmentUrl,
    setLinePostReaction,
    uploadLinePostAttachment,
    type LinePost,
    type LineVisibility,
  } from "$lib/api";

  let {
    viewerId,
    viewerRole,
    defaultVisibility,
  }: {
    viewerId: string;
    viewerRole: "administrator" | "member";
    defaultVisibility: LineVisibility;
  } = $props();

  const reactionChoices = ["👍", "❤️", "😂", "🎉", "😕", "👀"] as const;

  let posts = $state.raw<LinePost[]>([]);
  let scope = $state<"instance" | "mine">("instance");
  let searchQuery = $state("");
  let appliedSearch = $state("");
  let activeTag = $state("");
  let loading = $state(true);
  let error = $state("");
  let draftContent = $state("");
  let draftVisibility = $state<LineVisibility>("private");
  let pendingFiles = $state.raw<File[]>([]);
  let replyingTo = $state<LinePost | null>(null);
  let submitting = $state(false);
  let mutatingPostId = $state("");
  let reactionMenuPostId = $state("");
  let composer = $state<HTMLTextAreaElement>();
  let fileInput = $state<HTMLInputElement>();

  let characterCount = $derived(draftContent.length);
  let canSubmit = $derived(
    draftContent.trim().length > 0 && characterCount <= 2000 && !submitting,
  );
  let hashtagCounts = $derived.by(() => {
    const counts = new SvelteMap<string, number>();
    for (const post of posts) {
      for (const tag of post.tags) {
        counts.set(tag, (counts.get(tag) ?? 0) + 1);
      }
    }
    return [...counts.entries()]
      .sort(
        (left, right) => right[1] - left[1] || left[0].localeCompare(right[0]),
      )
      .slice(0, 12);
  });

  onMount(() => {
    draftVisibility = defaultVisibility;
    void loadPosts();
  });

  async function loadPosts() {
    loading = true;
    error = "";
    try {
      posts = await fetchLinePosts({
        scope,
        q: appliedSearch,
        tag: activeTag,
      });
    } catch (loadError) {
      error =
        loadError instanceof Error
          ? loadError.message
          : "Unable to load Lines.";
    } finally {
      loading = false;
    }
  }

  async function selectScope(nextScope: "instance" | "mine") {
    if (scope === nextScope) return;
    scope = nextScope;
    await loadPosts();
  }

  async function applySearch() {
    appliedSearch = searchQuery.trim();
    await loadPosts();
  }

  async function clearFilters() {
    searchQuery = "";
    appliedSearch = "";
    activeTag = "";
    await loadPosts();
  }

  async function selectTag(tag: string) {
    activeTag = activeTag === tag ? "" : tag;
    await loadPosts();
  }

  function chooseFiles(event: Event) {
    const target = event.currentTarget as HTMLInputElement;
    const selected = Array.from(target.files ?? []);
    const oversized = selected.find((file) => file.size > 10 * 1024 * 1024);
    if (oversized) {
      error = `${oversized.name} is larger than 10 MB.`;
    }
    pendingFiles = [
      ...pendingFiles,
      ...selected.filter(
        (file) => file.size > 0 && file.size <= 10 * 1024 * 1024,
      ),
    ];
    target.value = "";
  }

  function removePendingFile(index: number) {
    pendingFiles = pendingFiles.filter((_, fileIndex) => fileIndex !== index);
  }

  async function submitPost() {
    if (!canSubmit) return;
    submitting = true;
    error = "";
    const files = pendingFiles;
    try {
      const post = await createLinePost({
        content: draftContent,
        visibility: draftVisibility,
        reply_to_post_id: replyingTo?.id ?? null,
      });
      const failedUploads: string[] = [];
      for (const file of files) {
        try {
          await uploadLinePostAttachment(post.id, file);
        } catch {
          failedUploads.push(file.name);
        }
      }
      draftContent = "";
      draftVisibility = defaultVisibility;
      pendingFiles = [];
      replyingTo = null;
      reactionMenuPostId = "";
      await loadPosts();
      if (failedUploads.length) {
        error = `Post saved, but these files could not be attached: ${failedUploads.join(", ")}.`;
      }
    } catch (submitError) {
      error =
        submitError instanceof Error
          ? submitError.message
          : "Unable to save post.";
    } finally {
      submitting = false;
    }
  }

  async function startReply(post: LinePost) {
    replyingTo = post;
    if (post.visibility === "private") draftVisibility = "private";
    await tick();
    composer?.focus();
  }

  function cancelReply() {
    replyingTo = null;
    draftVisibility = defaultVisibility;
  }

  async function removePost(post: LinePost) {
    const ownPost = post.user_id === viewerId;
    const message = ownPost
      ? "Delete this post and its attachments? Replies will remain in the feed."
      : "Force-delete this public post as an administrator?";
    if (!window.confirm(message)) return;
    mutatingPostId = post.id;
    error = "";
    try {
      await deleteLinePost(post.id);
      posts = posts.filter((candidate) => candidate.id !== post.id);
    } catch (deleteError) {
      error =
        deleteError instanceof Error
          ? deleteError.message
          : "Unable to delete post.";
    } finally {
      mutatingPostId = "";
    }
  }

  async function toggleReaction(post: LinePost, emoji: string) {
    if (mutatingPostId) return;
    const current = post.reactions.find((reaction) => reaction.emoji === emoji);
    mutatingPostId = post.id;
    error = "";
    try {
      const updated = await setLinePostReaction(
        post.id,
        emoji,
        !current?.reacted_by_viewer,
      );
      posts = posts.map((candidate) =>
        candidate.id === updated.id ? updated : candidate,
      );
      reactionMenuPostId = "";
    } catch (reactionError) {
      error =
        reactionError instanceof Error
          ? reactionError.message
          : "Unable to update reaction.";
    } finally {
      mutatingPostId = "";
    }
  }

  function renderedMarkdown(content: string): string {
    const parsed = marked.parse(content, {
      async: false,
      breaks: true,
      gfm: true,
    });
    return DOMPurify.sanitize(String(parsed), { USE_PROFILES: { html: true } });
  }

  function attachRenderedPost(content: string) {
    return (node: HTMLElement) => {
      node.innerHTML = renderedMarkdown(content);
      return () => {
        node.textContent = "";
      };
    };
  }

  function initials(name: string): string {
    return (
      name
        .trim()
        .split(/\s+/)
        .slice(0, 2)
        .map((part) => part[0]?.toUpperCase())
        .join("") || "P>"
    );
  }

  function postDate(value: string): string {
    const date = new Date(value);
    return new Intl.DateTimeFormat("en", {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(date);
  }

  function fileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function imageAttachment(mimeType: string): boolean {
    return ["image/jpeg", "image/png", "image/webp", "image/avif"].includes(
      mimeType,
    );
  }

  function handleComposerKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      void submitPost();
    }
  }
</script>

<section class="lines-page" data-od-id="lines-page">
  <div class="lines-feed-column">
    <section class="lines-composer" data-od-id="lines-composer">
      <div class="lines-composer-heading">
        <div>
          <span>[ QUICK.CAPTURE ]</span>
          <h2>Write a line.</h2>
        </div>
        <div class="lines-visibility" data-od-id="lines-visibility-control">
          <button
            class={[draftVisibility === "private" && "is-active"]}
            type="button"
            aria-pressed={draftVisibility === "private"}
            onclick={() => (draftVisibility = "private")}
          >
            <LockKeyhole size={14} strokeWidth={1.8} aria-hidden="true" />
            Private
          </button>
          <button
            class={[draftVisibility === "public" && "is-active"]}
            type="button"
            aria-pressed={draftVisibility === "public"}
            disabled={replyingTo?.visibility === "private"}
            onclick={() => (draftVisibility = "public")}
          >
            <Globe2 size={14} strokeWidth={1.8} aria-hidden="true" />
            Instance
          </button>
        </div>
      </div>

      {#if replyingTo}
        <div class="lines-reply-context" data-od-id="lines-reply-context">
          <MessageCircle size={15} strokeWidth={1.8} aria-hidden="true" />
          <span>
            Replying to <strong>{replyingTo.author_name}</strong>
            <small>{replyingTo.content.slice(0, 120)}</small>
          </span>
          <button type="button" aria-label="Cancel reply" onclick={cancelReply}>
            <X size={16} strokeWidth={1.8} aria-hidden="true" />
          </button>
        </div>
      {/if}

      <textarea
        bind:this={composer}
        bind:value={draftContent}
        onkeydown={handleComposerKeydown}
        maxlength="2000"
        rows="5"
        placeholder="Markdown is supported. Add #hashtags to make this post discoverable."
        aria-label={replyingTo ? "Write a reply" : "Write a post"}
        data-od-id="lines-post-content"></textarea>

      {#if pendingFiles.length}
        <div class="lines-pending-files" data-od-id="lines-pending-files">
          {#each pendingFiles as file, index (`${file.name}-${file.lastModified}-${index}`)}
            <span>
              {#if imageAttachment(file.type)}
                <ImageIcon size={14} strokeWidth={1.8} aria-hidden="true" />
              {:else}
                <FileText size={14} strokeWidth={1.8} aria-hidden="true" />
              {/if}
              <span>{file.name}<small>{fileSize(file.size)}</small></span>
              <button
                type="button"
                aria-label={`Remove ${file.name}`}
                onclick={() => removePendingFile(index)}
              >
                <X size={14} strokeWidth={1.8} aria-hidden="true" />
              </button>
            </span>
          {/each}
        </div>
      {/if}

      <div class="lines-composer-actions">
        <div>
          <input
            bind:this={fileInput}
            class="lines-file-input"
            type="file"
            multiple
            onchange={chooseFiles}
            data-od-id="lines-file-input"
          />
          <button
            class="ui-button ui-button--ghost"
            type="button"
            onclick={() => fileInput?.click()}
            data-od-id="attach-lines-files"
          >
            <Paperclip size={16} strokeWidth={1.8} aria-hidden="true" />
            Attach
          </button>
          <span class:over-limit={characterCount > 2000}>
            {characterCount} / 2000
          </span>
        </div>
        <button
          class="ui-button ui-button--primary"
          type="button"
          disabled={!canSubmit}
          onclick={submitPost}
          data-od-id="publish-line-post"
        >
          <Send size={16} strokeWidth={1.8} aria-hidden="true" />
          {submitting ? "Posting…" : replyingTo ? "Reply" : "Post"}
        </button>
      </div>
    </section>

    <nav
      class="lines-feed-tabs"
      aria-label="Lines feeds"
      data-od-id="lines-feed-tabs"
    >
      <button
        class:active={scope === "instance"}
        type="button"
        aria-current={scope === "instance" ? "page" : undefined}
        onclick={() => selectScope("instance")}>Instance</button
      >
      <button
        class:active={scope === "mine"}
        type="button"
        aria-current={scope === "mine" ? "page" : undefined}
        onclick={() => selectScope("mine")}>Mine</button
      >
      <button
        class="lines-refresh"
        type="button"
        aria-label="Refresh Lines"
        disabled={loading}
        onclick={loadPosts}
      >
        <RefreshCw size={16} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </nav>

    {#if error}
      <p class="lines-error" role="alert">{error}</p>
    {/if}

    <section class="lines-feed" aria-busy={loading} data-od-id="lines-feed">
      {#if loading}
        <div class="lines-status">
          <span></span>
          Loading the timeline…
        </div>
      {:else if posts.length === 0}
        <div class="lines-empty">
          <strong>No posts found.</strong>
          <p>
            {appliedSearch || activeTag
              ? "Clear the current search or hashtag filter."
              : scope === "mine"
                ? "Your first post starts in the composer above."
                : "Be the first person to write on this instance."}
          </p>
          {#if appliedSearch || activeTag}
            <button
              class="ui-button ui-button--secondary"
              type="button"
              onclick={clearFilters}>Clear filters</button
            >
          {/if}
        </div>
      {:else}
        {#each posts as post (post.id)}
          <article class="line-post" data-od-id={`line-post-${post.id}`}>
            <div class="line-post-avatar" aria-hidden="true">
              {initials(post.author_name)}
            </div>
            <div class="line-post-body">
              <header>
                <div>
                  <strong>{post.author_name}</strong>
                  <time datetime={post.created_at}
                    >{postDate(post.created_at)}</time
                  >
                </div>
                <span
                  class="line-post-visibility"
                  title={post.visibility === "private"
                    ? "Private post — only you can view it"
                    : "Instance post — visible to signed-in users"}
                  data-od-id={`line-post-visibility-${post.id}`}
                >
                  {#if post.visibility === "private"}
                    <LockKeyhole
                      size={13}
                      strokeWidth={1.8}
                      aria-hidden="true"
                    />
                    <span class="sr-only">Private post</span>
                  {:else}
                    <Globe2 size={13} strokeWidth={1.8} aria-hidden="true" />
                    <span class="sr-only">Instance post</span>
                  {/if}
                </span>
              </header>

              {#if post.reply_to_post_id}
                <div class="line-post-reply-reference">
                  Replying to <strong
                    >{post.reply_to_author_name ?? "a removed post"}</strong
                  >
                  {#if post.reply_to_content}
                    <span>{post.reply_to_content.slice(0, 140)}</span>
                  {/if}
                </div>
              {/if}

              <div
                class="line-post-markdown"
                {@attach attachRenderedPost(post.content)}
              ></div>

              {#if post.attachments.length}
                <div class="line-post-attachments">
                  {#each post.attachments as attachment (attachment.id)}
                    {#if imageAttachment(attachment.mime_type)}
                      <a
                        class="line-post-image"
                        href={linePostAttachmentUrl(post.id, attachment.id)}
                        target="_blank"
                        rel="noreferrer"
                      >
                        <img
                          src={linePostAttachmentUrl(post.id, attachment.id)}
                          alt={attachment.file_name}
                          loading="lazy"
                        />
                      </a>
                    {:else}
                      <a
                        class="line-post-file"
                        href={linePostAttachmentUrl(post.id, attachment.id)}
                        download={attachment.file_name}
                      >
                        <FileText
                          size={18}
                          strokeWidth={1.7}
                          aria-hidden="true"
                        />
                        <span>
                          <strong>{attachment.file_name}</strong>
                          <small>{fileSize(attachment.byte_size)}</small>
                        </span>
                      </a>
                    {/if}
                  {/each}
                </div>
              {/if}

              {#if post.tags.length}
                <div class="line-post-tags" aria-label="Post hashtags">
                  {#each post.tags as tag (tag)}
                    <button type="button" onclick={() => selectTag(tag)}
                      >#{tag}</button
                    >
                  {/each}
                </div>
              {/if}

              <footer>
                <button
                  type="button"
                  onclick={() => startReply(post)}
                  data-od-id={`reply-line-post-${post.id}`}
                >
                  <MessageCircle
                    size={16}
                    strokeWidth={1.8}
                    aria-hidden="true"
                  />
                  Reply{post.reply_count ? ` · ${post.reply_count}` : ""}
                </button>

                <div class="line-post-reactions">
                  {#each post.reactions as reaction (reaction.emoji)}
                    <button
                      class:active={reaction.reacted_by_viewer}
                      type="button"
                      aria-pressed={reaction.reacted_by_viewer}
                      aria-label={`${reaction.emoji} reaction, ${reaction.count}`}
                      disabled={mutatingPostId === post.id}
                      onclick={() => toggleReaction(post, reaction.emoji)}
                    >
                      <span>{reaction.emoji}</span>{reaction.count}
                    </button>
                  {/each}
                  <button
                    type="button"
                    aria-label="Add a reaction"
                    aria-expanded={reactionMenuPostId === post.id}
                    onclick={() =>
                      (reactionMenuPostId =
                        reactionMenuPostId === post.id ? "" : post.id)}
                  >
                    <SmilePlus size={16} strokeWidth={1.8} aria-hidden="true" />
                  </button>
                  {#if reactionMenuPostId === post.id}
                    <div
                      class="line-reaction-picker"
                      aria-label="Choose a reaction"
                    >
                      {#each reactionChoices as emoji (emoji)}
                        <button
                          type="button"
                          aria-label={`React with ${emoji}`}
                          onclick={() => toggleReaction(post, emoji)}
                          >{emoji}</button
                        >
                      {/each}
                    </div>
                  {/if}
                </div>

                {#if post.user_id === viewerId || (viewerRole === "administrator" && post.visibility === "public")}
                  <button
                    class="line-post-delete"
                    type="button"
                    aria-label={post.user_id === viewerId
                      ? "Delete post"
                      : "Force-delete post"}
                    disabled={mutatingPostId === post.id}
                    onclick={() => removePost(post)}
                  >
                    <Trash2 size={16} strokeWidth={1.8} aria-hidden="true" />
                    {post.user_id === viewerId ? "Delete" : "Moderate"}
                  </button>
                {/if}
              </footer>
            </div>
          </article>
        {/each}
      {/if}
    </section>
  </div>

  <aside
    class="lines-discovery"
    aria-label="Lines discovery"
    data-od-id="lines-discovery"
  >
    <form
      class="lines-search"
      onsubmit={(event) => {
        event.preventDefault();
        void applySearch();
      }}
    >
      <label for="lines-search-input">Search Lines</label>
      <div>
        <Search size={17} strokeWidth={1.8} aria-hidden="true" />
        <input
          id="lines-search-input"
          type="search"
          bind:value={searchQuery}
          maxlength="100"
          placeholder="Text or phrase"
          data-od-id="lines-search-input"
        />
      </div>
      <button class="ui-button ui-button--secondary" type="submit"
        >Search posts</button
      >
    </form>

    <section class="lines-filter-summary">
      <span>[ ACTIVE.FILTER ]</span>
      {#if appliedSearch || activeTag}
        <strong>{activeTag ? `#${activeTag}` : `“${appliedSearch}”`}</strong>
        <button type="button" onclick={clearFilters}>Clear filter</button>
      {:else}
        <p>
          Showing the latest {scope === "mine"
            ? "posts you wrote"
            : "visible instance posts"}.
        </p>
      {/if}
    </section>

    <section class="lines-hashtags" data-od-id="lines-hashtag-filters">
      <header>
        <span>[ HASHTAGS ]</span>
        <small>{hashtagCounts.length}</small>
      </header>
      {#if hashtagCounts.length}
        <div>
          {#each hashtagCounts as [tag, count] (tag)}
            <button
              class:active={activeTag === tag}
              type="button"
              aria-pressed={activeTag === tag}
              onclick={() => selectTag(tag)}
            >
              <span>#{tag}</span><small>{count}</small>
            </button>
          {/each}
        </div>
      {:else}
        <p>Hashtags from visible posts will collect here.</p>
      {/if}
    </section>

  </aside>
</section>

<style>
  .lines-page {
    display: grid;
    grid-template-columns: minmax(0, 720px) minmax(240px, 320px);
    justify-content: center;
    gap: 24px;
    width: 100%;
    min-height: 100%;
    padding: 24px;
  }

  .lines-feed-column {
    min-width: 0;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--bg) 90%, transparent);
    backdrop-filter: blur(18px);
  }

  .lines-composer {
    padding: 22px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 78%, transparent);
  }

  .lines-composer-heading,
  .lines-composer-actions,
  .line-post header,
  .line-post footer,
  .lines-hashtags header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .lines-composer-heading > div:first-child > span,
  .lines-filter-summary > span,
  .lines-hashtags header > span {
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
  }

  .lines-composer h2 {
    margin: 3px 0 0;
    font-family: var(--font-display);
    font-size: clamp(24px, 3vw, 34px);
    font-weight: 610;
    letter-spacing: -0.02em;
  }

  .lines-visibility {
    display: inline-flex;
    border: 1px solid var(--border);
  }

  .lines-visibility button {
    min-height: 40px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
  }

  .lines-visibility button + button {
    border-left: 1px solid var(--border);
  }

  .lines-visibility button.is-active {
    background: var(--fg);
    color: var(--bg);
  }

  .lines-visibility button:disabled {
    cursor: not-allowed;
    opacity: 0.45;
  }

  .lines-reply-context {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    margin-top: 18px;
    padding: 12px;
    border: 1px solid var(--border);
    background: var(--fg-soft);
    color: var(--muted);
    font-size: 12px;
  }

  .lines-reply-context > span {
    flex: 1;
    min-width: 0;
  }

  .lines-reply-context strong {
    color: var(--fg);
  }

  .lines-reply-context small {
    display: block;
    overflow: hidden;
    margin-top: 3px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lines-reply-context button,
  .lines-pending-files button {
    width: 32px;
    height: 32px;
    border: 0;
    background: transparent;
  }

  textarea {
    width: 100%;
    min-height: 128px;
    margin-top: 18px;
    padding: 0;
    resize: vertical;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 14px;
    line-height: 1.7;
  }

  textarea::placeholder,
  input::placeholder {
    color: color-mix(in oklch, var(--muted) 72%, transparent);
  }

  textarea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 8px;
  }

  .lines-pending-files {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin: 8px 0 16px;
  }

  .lines-pending-files > span {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 100%;
    padding: 5px 5px 5px 10px;
    border: 1px solid var(--border);
    background: var(--surface);
    font-size: 12px;
  }

  .lines-pending-files > span > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lines-pending-files small {
    margin-left: 6px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .lines-composer-actions {
    margin-top: 15px;
    padding-top: 15px;
    border-top: 1px solid var(--border);
  }

  .lines-composer-actions > div {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .lines-composer-actions > div > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .lines-composer-actions > div > span.over-limit {
    color: var(--danger);
  }

  .lines-file-input {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    clip-path: inset(50%);
  }

  .lines-feed-tabs {
    display: flex;
    align-items: stretch;
    min-height: 50px;
    border-bottom: 1px solid var(--border);
  }

  .lines-feed-tabs button {
    position: relative;
    min-width: 92px;
    min-height: 50px;
    border: 0;
    border-right: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .lines-feed-tabs button.active {
    color: var(--fg);
  }

  .lines-feed-tabs button.active::after {
    position: absolute;
    right: 18px;
    bottom: -1px;
    left: 18px;
    height: 2px;
    background: var(--accent);
    content: "";
  }

  .lines-feed-tabs .lines-refresh {
    display: grid;
    align-self: stretch;
    place-items: center;
    min-width: 50px;
    margin-left: auto;
    padding: 0;
    border-right: 0;
    border-left: 1px solid var(--border);
  }

  .lines-feed-tabs .lines-refresh :global(svg) {
    display: block;
  }

  .lines-error {
    margin: 0;
    padding: 12px 18px;
    border-bottom: 1px solid
      color-mix(in oklch, var(--danger) 44%, var(--border));
    background: color-mix(in oklch, var(--danger) 10%, var(--surface));
    color: var(--danger);
    font-size: 12px;
  }

  .line-post {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr);
    gap: 14px;
    padding: 20px 22px;
    border-bottom: 1px solid var(--border);
  }

  .line-post:last-child {
    border-bottom: 0;
  }

  .line-post-avatar {
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border: 1px solid var(--border);
    background: var(--fg);
    color: var(--bg);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 750;
    letter-spacing: 0.05em;
  }

  .line-post-body {
    min-width: 0;
  }

  .line-post header > div {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .line-post header strong {
    overflow: hidden;
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .line-post time,
  .line-post-visibility {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .line-post-visibility {
    display: grid;
    flex: 0 0 auto;
    place-items: center;
    width: 28px;
    height: 28px;
    cursor: help;
  }

  .line-post-reply-reference {
    margin-top: 10px;
    padding: 9px 11px;
    border-left: 1px solid var(--border);
    color: var(--muted);
    font-size: 11px;
  }

  .line-post-reply-reference strong {
    color: var(--fg);
  }

  .line-post-reply-reference span {
    display: block;
    overflow: hidden;
    margin-top: 2px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .line-post-markdown {
    margin-top: 12px;
    overflow-wrap: anywhere;
    color: var(--fg);
    font-size: 14px;
    line-height: 1.7;
  }

  .line-post-markdown :global(:first-child) {
    margin-top: 0;
  }

  .line-post-markdown :global(:last-child) {
    margin-bottom: 0;
  }

  .line-post-markdown :global(p),
  .line-post-markdown :global(ul),
  .line-post-markdown :global(ol),
  .line-post-markdown :global(blockquote),
  .line-post-markdown :global(pre),
  .line-post-markdown :global(table) {
    margin: 0 0 10px;
  }

  .line-post-markdown :global(h1),
  .line-post-markdown :global(h2),
  .line-post-markdown :global(h3),
  .line-post-markdown :global(h4),
  .line-post-markdown :global(h5),
  .line-post-markdown :global(h6) {
    margin: 18px 0 8px;
    color: var(--fg);
    font-family: var(--font-display);
    font-weight: 620;
    line-height: 1.25;
  }

  .line-post-markdown :global(h1) {
    font-size: 24px;
    letter-spacing: -0.02em;
  }

  .line-post-markdown :global(h2) {
    font-size: 20px;
    letter-spacing: -0.015em;
  }

  .line-post-markdown :global(h3) {
    font-size: 17px;
  }

  .line-post-markdown :global(h4),
  .line-post-markdown :global(h5),
  .line-post-markdown :global(h6) {
    font-size: 14px;
  }

  .line-post-markdown :global(ul),
  .line-post-markdown :global(ol) {
    padding-left: 22px;
  }

  .line-post-markdown :global(ul) {
    list-style: square outside;
  }

  .line-post-markdown :global(ol) {
    list-style: decimal outside;
  }

  .line-post-markdown :global(li + li) {
    margin-top: 4px;
  }

  .line-post-markdown :global(blockquote) {
    padding-left: 12px;
    border-left: 2px solid var(--border);
    color: var(--muted);
  }

  .line-post-markdown :global(hr) {
    margin: 16px 0;
    border: 0;
    border-top: 1px solid var(--border);
  }

  .line-post-markdown :global(a) {
    color: var(--fg);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .line-post-markdown :global(code) {
    padding: 2px 5px;
    background: var(--fg-soft);
    font-family: var(--font-mono);
    font-size: 0.9em;
  }

  .line-post-markdown :global(pre) {
    max-width: 100%;
    overflow-x: auto;
    padding: 12px;
    border: 1px solid var(--border);
    background: var(--fg);
    color: var(--bg);
  }

  .line-post-markdown :global(pre code) {
    padding: 0;
    background: transparent;
  }

  .line-post-markdown :global(table) {
    display: block;
    max-width: 100%;
    overflow-x: auto;
    border-collapse: collapse;
  }

  .line-post-markdown :global(th),
  .line-post-markdown :global(td) {
    padding: 7px 9px;
    border: 1px solid var(--border);
    text-align: left;
    vertical-align: top;
  }

  .line-post-markdown :global(th) {
    background: var(--fg-soft);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
  }

  .line-post-markdown :global(img) {
    display: block;
    max-width: 100%;
    height: auto;
    margin: 10px 0;
    border: 1px solid var(--border);
  }

  .line-post-markdown :global(input[type="checkbox"]) {
    width: 15px;
    height: 15px;
    margin: 0 7px 0 0;
    vertical-align: -2px;
  }

  .line-post-attachments {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    margin-top: 14px;
  }

  .line-post-image {
    display: block;
    min-height: 120px;
    overflow: hidden;
    border: 1px solid var(--border);
    background: var(--fg-soft);
  }

  .line-post-image img {
    display: block;
    width: 100%;
    height: 100%;
    max-height: 360px;
    object-fit: cover;
  }

  .line-post-file {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    min-height: 56px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    color: var(--fg);
    text-decoration: none;
  }

  .line-post-file:hover {
    border-color: var(--fg);
    background: var(--fg-soft);
  }

  .line-post-file span {
    min-width: 0;
  }

  .line-post-file strong,
  .line-post-file small {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .line-post-file strong {
    font-size: 11px;
  }

  .line-post-file small {
    margin-top: 2px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }

  .line-post-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }

  .line-post-tags button {
    min-height: 30px;
    padding: 0 8px;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .line-post-tags button:hover {
    color: var(--fg);
  }

  .line-post footer {
    justify-content: flex-start;
    margin-top: 13px;
    padding-top: 11px;
    border-top: 1px solid var(--border);
  }

  .line-post footer > button,
  .line-post-reactions > button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 0 8px;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .line-post footer > button:hover,
  .line-post-reactions > button:hover,
  .line-post-reactions > button.active {
    background: var(--fg-soft);
    color: var(--fg);
  }

  .line-post-reactions {
    position: relative;
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .line-post-reactions > button span {
    font-size: 14px;
  }

  .line-reaction-picker {
    position: absolute;
    bottom: calc(100% + 7px);
    left: 0;
    z-index: 4;
    display: flex;
    padding: 6px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .line-reaction-picker button {
    width: 38px;
    height: 38px;
    border: 0;
    background: transparent;
    font-size: 18px;
  }

  .line-reaction-picker button:hover {
    background: var(--fg-soft);
  }

  .line-post footer .line-post-delete {
    margin-left: auto;
  }

  .line-post footer .line-post-delete:hover {
    background: color-mix(in oklch, var(--danger) 10%, transparent);
    color: var(--danger);
  }

  .lines-status,
  .lines-empty {
    display: grid;
    place-items: center;
    min-height: 220px;
    padding: 32px;
    color: var(--muted);
    text-align: center;
  }

  .lines-status span {
    width: 18px;
    height: 18px;
    margin-bottom: 10px;
    border: 2px solid var(--border);
    border-top-color: var(--fg);
    border-radius: 50%;
    animation: lines-spin 700ms linear infinite;
  }

  .lines-empty strong {
    color: var(--fg);
    font-family: var(--font-display);
    font-size: 20px;
  }

  .lines-empty p {
    max-width: 38ch;
    margin: 6px 0 18px;
    font-size: 13px;
  }

  .lines-discovery {
    align-self: start;
    position: sticky;
    top: 24px;
    display: grid;
    gap: 14px;
  }

  .lines-search,
  .lines-filter-summary,
  .lines-hashtags {
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--bg) 90%, transparent);
    backdrop-filter: blur(18px);
  }

  .lines-search {
    display: grid;
    gap: 10px;
    padding: 16px;
  }

  .lines-search label {
    font-family: var(--font-display);
    font-size: 16px;
    font-weight: 620;
  }

  .lines-search > div {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 44px;
    padding: 0 12px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--muted);
  }

  .lines-search input {
    min-width: 0;
    flex: 1;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: 12px;
  }

  .lines-search > div:focus-within {
    border-color: var(--accent);
    outline: 2px solid var(--accent-soft);
  }

  .lines-filter-summary,
  .lines-hashtags {
    padding: 16px;
  }

  .lines-filter-summary strong {
    display: block;
    margin-top: 8px;
    overflow-wrap: anywhere;
    font-size: 13px;
  }

  .lines-filter-summary p,
  .lines-hashtags > p {
    margin: 7px 0 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.55;
  }

  .lines-filter-summary button {
    min-height: 32px;
    margin-top: 10px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .lines-hashtags header small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .lines-hashtags > div {
    display: grid;
    margin-top: 10px;
  }

  .lines-hashtags button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 40px;
    padding: 0 8px;
    border: 0;
    border-top: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
    text-align: left;
  }

  .lines-hashtags button:hover,
  .lines-hashtags button.active {
    background: var(--fg-soft);
  }

  .lines-hashtags button small {
    color: var(--muted);
  }

  button:focus-visible,
  a:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  @keyframes lines-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (max-width: 980px) {
    .lines-page {
      grid-template-columns: minmax(0, 720px);
    }

    .lines-discovery {
      position: static;
      grid-row: 1;
      grid-template-columns: 1fr 1fr;
    }

    .lines-hashtags {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 640px) {
    .lines-page {
      grid-template-columns: 1fr;
      gap: 0;
      padding: 0;
    }

    .lines-feed-column {
      border: 0;
    }

    .lines-discovery {
      grid-template-columns: 1fr;
      gap: 0;
      padding-bottom: 12px;
    }

    .lines-filter-summary {
      display: none;
    }

    .lines-search,
    .lines-hashtags {
      border-width: 0 0 1px;
    }

    .lines-composer,
    .line-post {
      padding: 16px;
    }

    .lines-composer-heading {
      align-items: flex-start;
      flex-direction: column;
    }

    .lines-visibility {
      width: 100%;
    }

    .lines-visibility button {
      flex: 1;
    }

    .lines-composer-actions {
      align-items: flex-end;
    }

    .lines-composer-actions > div {
      align-items: flex-start;
      flex-direction: column;
      gap: 4px;
    }

    .line-post {
      grid-template-columns: 34px minmax(0, 1fr);
      gap: 10px;
    }

    .line-post-avatar {
      width: 34px;
      height: 34px;
      font-size: 9px;
    }

    .line-post header {
      align-items: flex-start;
    }

    .line-post header > div {
      align-items: flex-start;
      flex-direction: column;
      gap: 1px;
    }

    .line-post-attachments {
      grid-template-columns: 1fr;
    }

    .line-post footer {
      flex-wrap: wrap;
      gap: 4px;
    }

    .line-post footer .line-post-delete {
      margin-left: 0;
    }

    .line-reaction-picker {
      right: 0;
      left: auto;
      flex-wrap: wrap;
      width: 164px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .lines-status span {
      animation: none;
    }
  }
</style>

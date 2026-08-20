<script lang="ts">
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import FileText from "lucide-svelte/icons/file-text";
  import Globe2 from "lucide-svelte/icons/globe-2";
  import ImageIcon from "lucide-svelte/icons/image";
  import LockKeyhole from "lucide-svelte/icons/lock-keyhole";
  import MessageCircle from "lucide-svelte/icons/message-circle";
  import Paperclip from "lucide-svelte/icons/paperclip";
  import ArrowLeft from "lucide-svelte/icons/arrow-left";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Send from "lucide-svelte/icons/send";
  import SmilePlus from "lucide-svelte/icons/smile-plus";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { SvelteMap } from "svelte/reactivity";
  import { createViewSwap } from "$lib/viewSwap.svelte";
  import {
    createLinePost,
    deleteLinePost,
    fetchLineAuthorFeed,
    fetchLinePosts,
    fetchLineThread,
    lineAuthorAvatarUrl,
    linePostAttachmentUrl,
    setLinePostReaction,
    uploadLinePostAttachment,
    type LineAuthorFeed,
    type LinePost,
    type LineThread,
    type LineVisibility,
  } from "$lib/api";

  type LinesView =
    | { kind: "feed" }
    | { kind: "post"; postId: string }
    | { kind: "author"; userId: string; fallbackName: string };

  let {
    viewerId,
    viewerName,
    viewerRole,
    defaultVisibility,
    homeToken = 0,
  }: {
    viewerId: string;
    viewerName: string;
    viewerRole: "administrator" | "member";
    defaultVisibility: LineVisibility;
    /**
     * Bumped by the shell whenever Lines is chosen in the sidebar. The page owns three
     * screens, so choosing Lines while a thread or an author is open has to come back
     * to the timeline rather than leave the click looking inert.
     */
    homeToken?: number;
  } = $props();

  const reactionChoices = ["👍", "❤️", "😂", "🎉", "😕", "👀"] as const;
  const viewSwap = createViewSwap();

  type DetailPrefetch =
    | {
        kind: "post";
        postId: string;
        promise: Promise<LineThread>;
        value: LineThread | null;
      }
    | {
        kind: "author";
        userId: string;
        promise: Promise<LineAuthorFeed>;
        value: LineAuthorFeed | null;
      };

  // Screen changes request their data before the outgoing screen leaves, and
  // the loader below claims the result, so a screen normally arrives with
  // content rather than a loading line. Not reactive: only the loader reads it.
  let detailPrefetch: DetailPrefetch | null = null;

  let posts = $state.raw<LinePost[]>([]);
  let view = $state<LinesView>({ kind: "feed" });
  let viewStack = $state.raw<LinesView[]>([]);
  let thread = $state.raw<LineThread | null>(null);
  let authorFeed = $state.raw<LineAuthorFeed | null>(null);
  let detailLoading = $state(false);
  let detailError = $state("");
  let scope = $state<"instance" | "mine">("instance");
  let searchQuery = $state("");
  let appliedSearch = $state("");
  let activeTag = $state("");
  let loading = $state(true);
  /** A reload with posts already on screen, which must not blank the timeline. */
  let refreshing = $state(false);
  /** Whether posts have ever landed. Plain, so the loader never depends on itself. */
  let hasLoaded = false;
  let error = $state("");
  let draftContent = $state("");
  let draftVisibility = $state<LineVisibility>("private");
  let pendingFiles = $state.raw<File[]>([]);
  let replyingTo = $state<LinePost | null>(null);
  let replyContent = $state("");
  let replyVisibility = $state<LineVisibility>("private");
  let replyFiles = $state.raw<File[]>([]);
  let replySubmitting = $state(false);
  let submitting = $state(false);
  let mutatingPostId = $state("");
  let reactionMenuPostId = $state("");
  let composer = $state<HTMLTextAreaElement>();
  let fileInput = $state<HTMLInputElement>();
  let replyDialog = $state<HTMLDialogElement>();
  let replyComposer = $state<HTMLTextAreaElement>();
  let replyFileInput = $state<HTMLInputElement>();

  let characterCount = $derived(draftContent.length);
  let canSubmit = $derived(
    draftContent.trim().length > 0 && characterCount <= 2000 && !submitting,
  );
  let replyCharacterCount = $derived(replyContent.length);
  let canSubmitReply = $derived(
    replyContent.trim().length > 0 &&
      replyCharacterCount <= 2000 &&
      !replySubmitting,
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

  let authorName = $derived(
    authorFeed?.author.display_name ??
      (view.kind === "author" ? view.fallbackName : ""),
  );

  onMount(() => {
    draftVisibility = defaultVisibility;
    void loadPosts();
  });

  onDestroy(() => {
    viewSwap.cancel();
  });

  // Detail screens are conditionally rendered, so they load from an effect
  // rather than onMount, and cancel in flight work when the view changes.
  $effect(() => {
    const current = view;
    const prefetched = detailPrefetch;
    detailPrefetch = null;
    if (current.kind === "feed") return;

    const readyThread =
      current.kind === "post" &&
      prefetched?.kind === "post" &&
      prefetched.postId === current.postId
        ? prefetched.value
        : null;
    const readyAuthorFeed =
      current.kind === "author" &&
      prefetched?.kind === "author" &&
      prefetched.userId === current.userId
        ? prefetched.value
        : null;

    if (readyThread || readyAuthorFeed) {
      if (readyThread) thread = readyThread;
      if (readyAuthorFeed) authorFeed = readyAuthorFeed;
      detailLoading = false;
      detailError = "";
      return;
    }

    let cancelled = false;
    detailLoading = true;
    detailError = "";
    (async () => {
      try {
        if (current.kind === "post") {
          const request =
            prefetched?.kind === "post" && prefetched.postId === current.postId
              ? prefetched.promise
              : fetchLineThread(current.postId);
          const loaded = await request;
          if (cancelled) return;
          thread = loaded;
        } else {
          const request =
            prefetched?.kind === "author" &&
            prefetched.userId === current.userId
              ? prefetched.promise
              : fetchLineAuthorFeed(current.userId);
          const loaded = await request;
          if (cancelled) return;
          authorFeed = loaded;
        }
      } catch (loadError) {
        if (cancelled) return;
        detailError =
          loadError instanceof Error
            ? loadError.message
            : "Unable to open this screen.";
      } finally {
        if (!cancelled) detailLoading = false;
      }
    })();

    return () => {
      cancelled = true;
    };
  });

  /**
   * Starts the request the incoming screen needs and parks it for the loader.
   * The returned promise settles rather than rejects: the loader owns
   * reporting the failure.
   */
  function prefetchDetail(target: LinesView) {
    if (target.kind === "post") {
      const entry = {
        kind: "post" as const,
        postId: target.postId,
        promise: fetchLineThread(target.postId),
        value: null as LineThread | null,
      };
      detailPrefetch = entry;
      return entry.promise.then(
        (loaded) => {
          if (detailPrefetch === entry) entry.value = loaded;
        },
        () => undefined,
      );
    }

    if (target.kind === "author") {
      const entry = {
        kind: "author" as const,
        userId: target.userId,
        promise: fetchLineAuthorFeed(target.userId),
        value: null as LineAuthorFeed | null,
      };
      detailPrefetch = entry;
      return entry.promise.then(
        (loaded) => {
          if (detailPrefetch === entry) entry.value = loaded;
        },
        () => undefined,
      );
    }

    detailPrefetch = null;
    return null;
  }

  function openView(next: LinesView) {
    if (next.kind === "post" && view.kind === "post" && view.postId === next.postId)
      return;
    if (next.kind === "author" && view.kind === "author" && view.userId === next.userId)
      return;
    const current = view;
    void viewSwap.run({
      forward: true,
      pending: prefetchDetail(next),
      commit: () => {
        viewStack = [...viewStack, current];
        if (next.kind === "post") thread = null;
        if (next.kind === "author") authorFeed = null;
        reactionMenuPostId = "";
        error = "";
        view = next;
      },
    });
  }

  function openPost(post: LinePost) {
    openView({ kind: "post", postId: post.id });
  }

  function openParentPost(post: LinePost) {
    if (!post.reply_to_post_id) return;
    openView({ kind: "post", postId: post.reply_to_post_id });
  }

  function openAuthor(post: LinePost) {
    openView({
      kind: "author",
      userId: post.user_id,
      fallbackName: post.author_name,
    });
  }

  function goBack() {
    const previous = viewStack.at(-1) ?? { kind: "feed" as const };
    void viewSwap.run({
      forward: false,
      pending: prefetchDetail(previous),
      commit: () => {
        viewStack = viewStack.slice(0, -1);
        reactionMenuPostId = "";
        error = "";
        view = previous;
      },
    });
  }

  $effect(() => {
    // Only the token is a dependency: `returnToFeed` reads `view`, and tracking that
    // would send the reader back to the timeline on every screen change.
    void homeToken;
    untrack(() => returnToFeed());
  });

  function returnToFeed() {
    // Filter changes call this from the timeline itself, where there is no
    // screen change to cover.
    if (view.kind === "feed") {
      viewStack = [];
      reactionMenuPostId = "";
      return;
    }
    void viewSwap.run({
      forward: false,
      commit: () => {
        viewStack = [];
        reactionMenuPostId = "";
        view = { kind: "feed" };
      },
    });
  }

  // Re-assigning the view hands the loader a fresh reference, which re-runs it.
  function reloadDetail() {
    if (view.kind !== "feed") view = { ...view };
  }

  /**
   * Loads the timeline. Only the first load blanks the feed: a later one keeps the
   * current posts up until the new set lands, and leaves them alone when it fails.
   */
  async function loadPosts() {
    if (hasLoaded) refreshing = true;
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
      hasLoaded = true;
      loading = false;
      refreshing = false;
    }
  }

  async function selectScope(nextScope: "instance" | "mine") {
    if (scope === nextScope && view.kind === "feed") return;
    scope = nextScope;
    returnToFeed();
    await loadPosts();
  }

  async function applySearch() {
    appliedSearch = searchQuery.trim();
    returnToFeed();
    await loadPosts();
  }

  async function clearFilters() {
    searchQuery = "";
    appliedSearch = "";
    activeTag = "";
    returnToFeed();
    await loadPosts();
  }

  async function selectTag(tag: string) {
    activeTag = activeTag === tag ? "" : tag;
    returnToFeed();
    await loadPosts();
  }

  function replacePost(updated: LinePost) {
    posts = posts.map((candidate) =>
      candidate.id === updated.id ? updated : candidate,
    );
    if (thread) {
      thread = {
        parent:
          thread.parent?.id === updated.id ? updated : (thread.parent ?? null),
        post: thread.post.id === updated.id ? updated : thread.post,
        replies: thread.replies.map((candidate) =>
          candidate.id === updated.id ? updated : candidate,
        ),
      };
    }
    if (authorFeed) {
      authorFeed = {
        ...authorFeed,
        posts: authorFeed.posts.map((candidate) =>
          candidate.id === updated.id ? updated : candidate,
        ),
      };
    }
  }

  function dropPost(postId: string) {
    posts = posts.filter((candidate) => candidate.id !== postId);
    if (thread) {
      thread = {
        ...thread,
        replies: thread.replies.filter((candidate) => candidate.id !== postId),
      };
    }
    if (authorFeed) {
      authorFeed = {
        ...authorFeed,
        posts: authorFeed.posts.filter((candidate) => candidate.id !== postId),
      };
    }
  }

  function acceptFiles(event: Event): File[] {
    const target = event.currentTarget as HTMLInputElement;
    const selected = Array.from(target.files ?? []);
    const oversized = selected.find((file) => file.size > 10 * 1024 * 1024);
    if (oversized) {
      error = `${oversized.name} is larger than 10 MB.`;
    }
    target.value = "";
    return selected.filter(
      (file) => file.size > 0 && file.size <= 10 * 1024 * 1024,
    );
  }

  function chooseFiles(event: Event) {
    pendingFiles = [...pendingFiles, ...acceptFiles(event)];
  }

  function chooseReplyFiles(event: Event) {
    replyFiles = [...replyFiles, ...acceptFiles(event)];
  }

  function removePendingFile(index: number) {
    pendingFiles = pendingFiles.filter((_, fileIndex) => fileIndex !== index);
  }

  function removeReplyFile(index: number) {
    replyFiles = replyFiles.filter((_, fileIndex) => fileIndex !== index);
  }

  async function publishPost(input: {
    content: string;
    visibility: LineVisibility;
    files: File[];
    replyToPostId: string | null;
  }): Promise<string[]> {
    const post = await createLinePost({
      content: input.content,
      visibility: input.visibility,
      reply_to_post_id: input.replyToPostId,
    });
    const failedUploads: string[] = [];
    for (const file of input.files) {
      try {
        await uploadLinePostAttachment(post.id, file);
      } catch {
        failedUploads.push(file.name);
      }
    }
    return failedUploads;
  }

  async function submitPost() {
    if (!canSubmit) return;
    submitting = true;
    error = "";
    try {
      const failedUploads = await publishPost({
        content: draftContent,
        visibility: draftVisibility,
        files: pendingFiles,
        replyToPostId: null,
      });
      draftContent = "";
      draftVisibility = defaultVisibility;
      pendingFiles = [];
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

  async function submitReply() {
    if (!canSubmitReply || !replyingTo) return;
    replySubmitting = true;
    error = "";
    try {
      const failedUploads = await publishPost({
        content: replyContent,
        visibility: replyVisibility,
        files: replyFiles,
        replyToPostId: replyingTo.id,
      });
      closeReply();
      reactionMenuPostId = "";
      await loadPosts();
      if (view.kind !== "feed") reloadDetail();
      if (failedUploads.length) {
        error = `Reply saved, but these files could not be attached: ${failedUploads.join(", ")}.`;
      }
    } catch (submitError) {
      error =
        submitError instanceof Error
          ? submitError.message
          : "Unable to save reply.";
    } finally {
      replySubmitting = false;
    }
  }

  async function startReply(post: LinePost) {
    replyingTo = post;
    replyContent = "";
    replyFiles = [];
    replyVisibility =
      post.visibility === "private" ? "private" : defaultVisibility;
    replyDialog?.showModal();
    await tick();
    replyComposer?.focus();
  }

  function closeReply() {
    replyDialog?.close();
  }

  function resetReply() {
    replyingTo = null;
    replyContent = "";
    replyFiles = [];
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
      dropPost(post.id);
      if (view.kind === "post" && thread?.post.id === post.id) {
        goBack();
      }
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
      replacePost(updated);
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

  function handleReplyKeydown(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      void submitReply();
    }
  }

  function hideBrokenAvatar(event: Event) {
    if (event.currentTarget instanceof HTMLImageElement) {
      event.currentTarget.remove();
    }
  }
</script>

{#snippet postCard(post: LinePost, focused: boolean)}
  <article
    class={["line-post", focused && "is-focused"]}
    data-od-id={`line-post-${post.id}`}
  >
    <button
      class="line-post-avatar-link"
      type="button"
      aria-label={`Posts by ${post.author_name}`}
      onclick={() => openAuthor(post)}
    >
      {@render lineAvatar(post.author_name, lineAuthorAvatarUrl(post.user_id))}
    </button>
    <div class="line-post-body">
      <header>
        <div>
          <button
            class="line-post-author"
            type="button"
            onclick={() => openAuthor(post)}
            data-od-id={`line-post-author-${post.id}`}
          >
            {post.author_name}
          </button>
          <button
            class="line-post-timestamp"
            type="button"
            aria-label={`Open this post from ${postDate(post.created_at)}`}
            onclick={() => openPost(post)}
            data-od-id={`line-post-permalink-${post.id}`}
          >
            <time datetime={post.created_at}>{postDate(post.created_at)}</time>
          </button>
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
        {#if post.reply_to_author_name}
          <button
            class="line-post-reply-reference"
            type="button"
            aria-label={`Open the post by ${post.reply_to_author_name} that this replies to`}
            onclick={() => openParentPost(post)}
            data-od-id={`line-post-parent-${post.id}`}
          >
            Replying to <strong>{post.reply_to_author_name}</strong>
            {#if post.reply_to_content}
              <span>{post.reply_to_content.slice(0, 140)}</span>
            {/if}
          </button>
        {:else}
          <div class="line-post-reply-reference is-static">
            Replying to <strong>a removed post</strong>
          </div>
        {/if}
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
          <MessageCircle size={16} strokeWidth={1.8} aria-hidden="true" />
          Reply
        </button>

        {#if post.reply_count}
          <button
            class="line-post-thread-link"
            type="button"
            onclick={() => openPost(post)}
            data-od-id={`open-line-thread-${post.id}`}
          >
            {post.reply_count}
            {post.reply_count === 1 ? "reply" : "replies"}
          </button>
        {/if}

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
{/snippet}

{#snippet lineAvatar(name: string, source: string)}
  <span class="line-avatar" aria-hidden="true">
    <span>{initials(name)}</span>
    <img src={source} alt="" loading="lazy" onerror={hideBrokenAvatar} />
  </span>
{/snippet}

{#snippet visibilityChoice(
  value: LineVisibility,
  select: (next: LineVisibility) => void,
  lockPrivate: boolean,
  odId: string,
)}
  <div class="lines-visibility" data-od-id={odId}>
    <button
      class={[value === "private" && "is-active"]}
      type="button"
      aria-pressed={value === "private"}
      onclick={() => select("private")}
    >
      <LockKeyhole size={14} strokeWidth={1.8} aria-hidden="true" />
      Private
    </button>
    <button
      class={[value === "public" && "is-active"]}
      type="button"
      aria-pressed={value === "public"}
      disabled={lockPrivate}
      onclick={() => select("public")}
    >
      <Globe2 size={14} strokeWidth={1.8} aria-hidden="true" />
      Instance
    </button>
  </div>
{/snippet}

{#snippet pendingFileList(
  files: File[],
  remove: (index: number) => void,
  odId: string,
)}
  <div class="lines-pending-files" data-od-id={odId}>
    {#each files as file, index (`${file.name}-${file.lastModified}-${index}`)}
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
          onclick={() => remove(index)}
        >
          <X size={14} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </span>
    {/each}
  </div>
{/snippet}

<section class="lines-page" data-od-id="lines-page">
  <div
    class="lines-feed-column view-swap"
    data-view-phase={viewSwap.phase}
    data-view-direction={viewSwap.direction}
    {@attach viewSwap.attach}
  >
    {#if view.kind === "feed"}
    <section class="lines-composer" data-od-id="lines-composer">
      <div class="lines-composer-entry">
        {@render lineAvatar(viewerName, "/api/settings/avatar")}
        <div class="lines-composer-field">
          <textarea
            bind:this={composer}
            bind:value={draftContent}
            onkeydown={handleComposerKeydown}
            maxlength="2000"
            rows="3"
            placeholder="Markdown is supported. Add #hashtags to make this post discoverable."
            aria-label="Write a post"
            data-od-id="lines-post-content"></textarea>

          {#if pendingFiles.length}
            {@render pendingFileList(
              pendingFiles,
              removePendingFile,
              "lines-pending-files",
            )}
          {/if}
        </div>
      </div>

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
        <div class="lines-composer-publish">
          {@render visibilityChoice(
            draftVisibility,
            (next) => (draftVisibility = next),
            false,
            "lines-visibility-control",
          )}
          <button
            class="ui-button ui-button--primary"
            type="button"
            disabled={!canSubmit}
            onclick={submitPost}
            data-od-id="publish-line-post"
          >
            <Send size={16} strokeWidth={1.8} aria-hidden="true" />
            {submitting ? "Posting…" : "Post"}
          </button>
        </div>
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
        disabled={loading || refreshing}
        onclick={loadPosts}
      >
        <RefreshCw
          class={refreshing ? "spinning" : undefined}
          size={16}
          strokeWidth={1.8}
          aria-hidden="true"
        />
      </button>
    </nav>

    {#if error}
      <p class="lines-error" role="alert">{error}</p>
    {/if}

    <section
      class="lines-feed"
      aria-busy={loading || refreshing}
      data-od-id="lines-feed"
    >
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
          {@render postCard(post, false)}
        {/each}
      {/if}
    </section>
    {:else}
      <header class="lines-screen-head" data-od-id="lines-screen-head">
        <button
          class="lines-back"
          type="button"
          onclick={goBack}
          data-od-id="lines-back"
        >
          <ArrowLeft size={17} strokeWidth={1.8} aria-hidden="true" />
          Back
        </button>
        <div>
          <span>{view.kind === "post" ? "[ THREAD ]" : "[ AUTHOR ]"}</span>
          <h2>
            {view.kind === "post"
              ? thread
                ? `${thread.post.author_name}'s post`
                : "Post"
              : authorName || "Author"}
          </h2>
        </div>
        <button
          class="lines-refresh"
          type="button"
          aria-label="Refresh this screen"
          disabled={detailLoading}
          onclick={reloadDetail}
        >
          <RefreshCw size={16} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </header>

      {#if error}
        <p class="lines-error" role="alert">{error}</p>
      {/if}

      <section
        class="lines-feed"
        aria-busy={detailLoading}
        data-od-id={view.kind === "post" ? "lines-thread" : "lines-author-feed"}
      >
        {#if detailLoading}
          <div class="lines-status">
            <span></span>
            {view.kind === "post" ? "Loading the thread…" : "Loading posts…"}
          </div>
        {:else if detailError}
          <div class="lines-empty">
            <strong>This screen is unavailable.</strong>
            <p>{detailError}</p>
            <button
              class="ui-button ui-button--secondary"
              type="button"
              onclick={returnToFeed}>Back to the timeline</button
            >
          </div>
        {:else if view.kind === "post" && thread}
          {#if thread.parent}
            {@render postCard(thread.parent, false)}
          {/if}
          {@render postCard(thread.post, true)}
          {#if thread.replies.length}
            <div class="lines-thread-divider">
              <span
                >{thread.replies.length}
                {thread.replies.length === 1 ? "reply" : "replies"}</span
              >
            </div>
            {#each thread.replies as reply (reply.id)}
              {@render postCard(reply, false)}
            {/each}
          {:else}
            <div class="lines-empty">
              <strong>No replies yet.</strong>
              <p>Use Reply on the post above to start the thread.</p>
            </div>
          {/if}
        {:else if view.kind === "author" && authorFeed}
          <div class="lines-author-card" data-od-id="lines-author-card">
            {@render lineAvatar(
              authorFeed.author.display_name,
              lineAuthorAvatarUrl(authorFeed.author.user_id),
            )}
            <div>
              <strong>{authorFeed.author.display_name}</strong>
              <small>
                {authorFeed.author.post_count}
                {authorFeed.author.post_count === 1 ? "post" : "posts"}
                visible to you{authorFeed.author.first_post_at
                  ? ` · since ${postDate(authorFeed.author.first_post_at)}`
                  : ""}
              </small>
            </div>
          </div>
          {#each authorFeed.posts as post (post.id)}
            {@render postCard(post, false)}
          {/each}
        {/if}
      </section>
    {/if}
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

<dialog
  class="ui-dialog lines-reply-dialog"
  bind:this={replyDialog}
  onclose={resetReply}
  aria-label="Reply to a post"
  data-od-id="lines-reply-dialog"
>
  <header class="lines-reply-head">
    <span>[ REPLY ]</span>
    <button
      class="ui-button ui-button--ghost ui-button--icon"
      type="button"
      aria-label="Close reply"
      onclick={closeReply}
      data-od-id="close-lines-reply"
    >
      <X size={18} strokeWidth={1.8} aria-hidden="true" />
    </button>
  </header>

  {#if replyingTo}
    <article class="lines-reply-source" data-od-id="lines-reply-source">
      {@render lineAvatar(
        replyingTo.author_name,
        lineAuthorAvatarUrl(replyingTo.user_id),
      )}
      <div>
        <header>
          <strong>{replyingTo.author_name}</strong>
          <time datetime={replyingTo.created_at}
            >{postDate(replyingTo.created_at)}</time
          >
          {#if replyingTo.visibility === "private"}
            <LockKeyhole size={13} strokeWidth={1.8} aria-hidden="true" />
          {:else}
            <Globe2 size={13} strokeWidth={1.8} aria-hidden="true" />
          {/if}
        </header>
        <div
          class="line-post-markdown"
          {@attach attachRenderedPost(replyingTo.content)}
        ></div>
      </div>
    </article>

    <div class="lines-composer-entry lines-reply-entry">
      {@render lineAvatar(viewerName, "/api/settings/avatar")}
      <div class="lines-composer-field">
        <textarea
          bind:this={replyComposer}
          bind:value={replyContent}
          onkeydown={handleReplyKeydown}
          maxlength="2000"
          rows="4"
          placeholder={`Reply to ${replyingTo.author_name}…`}
          aria-label="Write a reply"
          data-od-id="lines-reply-content"></textarea>

        {#if replyFiles.length}
          {@render pendingFileList(
            replyFiles,
            removeReplyFile,
            "lines-reply-pending-files",
          )}
        {/if}
      </div>
    </div>

    <div class="lines-composer-actions">
      <div>
        <input
          bind:this={replyFileInput}
          class="lines-file-input"
          type="file"
          multiple
          onchange={chooseReplyFiles}
          data-od-id="lines-reply-file-input"
        />
        <button
          class="ui-button ui-button--ghost"
          type="button"
          onclick={() => replyFileInput?.click()}
          data-od-id="attach-lines-reply-files"
        >
          <Paperclip size={16} strokeWidth={1.8} aria-hidden="true" />
          Attach
        </button>
        <span class:over-limit={replyCharacterCount > 2000}>
          {replyCharacterCount} / 2000
        </span>
      </div>
      <div class="lines-composer-publish">
        {@render visibilityChoice(
          replyVisibility,
          (next) => (replyVisibility = next),
          replyingTo.visibility === "private",
          "lines-reply-visibility-control",
        )}
        <button
          class="ui-button ui-button--primary"
          type="button"
          disabled={!canSubmitReply}
          onclick={submitReply}
          data-od-id="publish-line-reply"
        >
          <Send size={16} strokeWidth={1.8} aria-hidden="true" />
          {replySubmitting ? "Replying…" : "Reply"}
        </button>
      </div>
    </div>
  {/if}
</dialog>

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

  .lines-composer-actions,
  .line-post header,
  .line-post footer,
  .lines-hashtags header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .lines-filter-summary > span,
  .lines-hashtags header > span,
  .lines-reply-head > span {
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
  }

  .lines-composer-entry {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr);
    align-items: start;
    gap: 14px;
  }

  .lines-composer-field {
    min-width: 0;
  }

  .lines-composer-publish {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .lines-visibility {
    display: inline-flex;
    border: 1px solid var(--border);
  }

  .lines-visibility button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
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

  .lines-pending-files button {
    width: 32px;
    height: 32px;
    border: 0;
    background: transparent;
  }

  textarea {
    width: 100%;
    min-height: 96px;
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
    margin: 10px 0 0;
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

  .lines-screen-head {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 66px;
    padding: 12px 12px 12px 16px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 78%, transparent);
  }

  .lines-screen-head > div {
    min-width: 0;
    flex: 1;
  }

  .lines-screen-head > div > span {
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
  }

  .lines-screen-head h2 {
    overflow: hidden;
    margin: 2px 0 0;
    font-family: var(--font-display);
    font-size: 20px;
    font-weight: 610;
    letter-spacing: -0.01em;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lines-back {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 40px;
    padding: 0 12px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .lines-back:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .lines-screen-head .lines-refresh {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    padding: 0;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
  }

  .lines-thread-divider {
    padding: 10px 22px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 55%, transparent);
  }

  .lines-thread-divider span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .lines-author-card {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr);
    align-items: center;
    gap: 14px;
    padding: 20px 22px;
    border-bottom: 1px solid var(--border);
  }

  .lines-author-card > div {
    min-width: 0;
  }

  .lines-author-card strong {
    display: block;
    overflow: hidden;
    font-size: 16px;
    font-weight: 650;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lines-author-card small {
    display: block;
    margin-top: 3px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
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

  .line-avatar {
    position: relative;
    display: grid;
    overflow: hidden;
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

  .line-avatar > img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
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

  .line-post-author {
    overflow: hidden;
    max-width: 100%;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--fg);
    font-size: 13px;
    font-weight: 650;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .line-post-author:hover,
  .line-post-timestamp:hover time {
    text-decoration: underline;
  }

  .line-post-timestamp {
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    background: transparent;
  }

  .line-post-avatar-link {
    align-self: start;
    padding: 0;
    border: 0;
    background: transparent;
    line-height: 0;
  }

  .line-post-avatar-link:hover .line-avatar,
  .line-post-avatar-link:focus-visible .line-avatar {
    border-color: var(--accent);
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
    display: block;
    width: 100%;
    margin-top: 10px;
    padding: 9px 11px;
    border: 0;
    border-left: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    font-size: 11px;
    text-align: left;
  }

  .line-post-reply-reference:not(.is-static):hover {
    border-left-color: var(--accent);
    background: var(--fg-soft);
  }

  .line-post-thread-link {
    padding: 0 8px;
    border: 0;
    background: transparent;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .line-post-thread-link:hover {
    color: var(--accent);
  }

  .line-post.is-focused {
    background: var(--fg-soft);
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

    .lines-composer-entry,
    .lines-author-card {
      grid-template-columns: 34px minmax(0, 1fr);
      gap: 10px;
    }

    .lines-screen-head,
    .lines-author-card,
    .lines-thread-divider {
      padding-right: 16px;
      padding-left: 16px;
    }

    .lines-screen-head h2 {
      font-size: 17px;
    }

    .lines-visibility button {
      flex: 1;
    }

    .lines-composer-actions {
      align-items: flex-start;
      flex-direction: column;
      gap: 12px;
    }

    .lines-composer-actions > div {
      align-items: center;
      gap: 10px;
    }

    .lines-composer-publish {
      width: 100%;
      justify-content: space-between;
    }

    .line-post {
      grid-template-columns: 34px minmax(0, 1fr);
      gap: 10px;
    }

    .line-avatar {
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

  .lines-reply-dialog {
    width: min(640px, calc(100vw - 24px));
    max-height: min(760px, calc(100dvh - 40px));
  }

  .lines-reply-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 56px;
    padding: 0 12px 0 22px;
    border-bottom: 1px solid var(--border);
  }

  .lines-reply-source {
    position: relative;
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr);
    gap: 14px;
    padding: 20px 22px 16px;
  }

  .lines-reply-source > div {
    min-width: 0;
  }

  .lines-reply-source header {
    display: flex;
    align-items: baseline;
    gap: 8px;
    color: var(--muted);
  }

  .lines-reply-source header strong {
    color: var(--fg);
    font-size: 14px;
    font-weight: 640;
  }

  .lines-reply-source header time {
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .lines-reply-source header :global(svg) {
    align-self: center;
  }

  .lines-reply-source::after {
    position: absolute;
    top: 68px;
    bottom: 0;
    left: 43px;
    width: 1px;
    background: var(--border);
    content: "";
  }

  .lines-reply-entry {
    padding: 0 22px 4px;
  }

  .lines-reply-dialog .lines-composer-actions {
    margin: 0 22px;
    padding: 15px 0 22px;
    border-top: 1px solid var(--border);
  }

  @media (max-width: 900px) {
    .lines-reply-source,
    .lines-reply-entry,
    .lines-reply-dialog .lines-composer-actions {
      padding-right: 16px;
      padding-left: 16px;
    }

    .lines-reply-dialog .lines-composer-actions {
      margin: 0 16px;
      padding-right: 0;
      padding-left: 0;
    }

    .lines-reply-source {
      grid-template-columns: 34px minmax(0, 1fr);
      gap: 10px;
    }

    .lines-reply-source::after {
      top: 60px;
      left: 33px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .lines-status span {
      animation: none;
    }
  }
</style>

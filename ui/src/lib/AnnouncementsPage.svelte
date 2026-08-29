<script lang="ts">
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import Ellipsis from "lucide-svelte/icons/ellipsis";
  import ImageIcon from "lucide-svelte/icons/image";
  import ImagePlus from "lucide-svelte/icons/image-plus";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Send from "lucide-svelte/icons/send";
  import SmilePlus from "lucide-svelte/icons/smile-plus";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount } from "svelte";
  import { motionPopover } from "$lib/motion.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    announcementAuthorAvatarUrl,
    announcementImageUrl,
    createAnnouncement,
    deleteAnnouncement,
    deleteAnnouncementImage,
    fetchAnnouncements,
    setAnnouncementReaction,
    updateAnnouncement,
    uploadAnnouncementImage,
    type Announcement,
    type AnnouncementImage,
  } from "$lib/api";

  let {
    viewerRole,
  }: {
    viewerRole: "administrator" | "member";
  } = $props();

  const reactionChoices = ["👍", "❤️", "😂", "🎉", "😕", "👀"] as const;
  const allowedImageTypes = new Set([
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/avif",
  ]);
  const maxImageBytes = 10 * 1024 * 1024;

  let announcements = $state.raw<Announcement[]>([]);
  let loading = $state(true);
  let refreshing = $state(false);
  let error = $state("");
  let editorOpen = $state(false);
  let editingId = $state("");
  let editorMode = $state<"write" | "preview">("write");
  let draftTitle = $state("");
  let draftContent = $state("");
  let pendingImages = $state.raw<File[]>([]);
  let imageInput: HTMLInputElement | undefined;
  let submitting = $state(false);
  let mutatingId = $state("");
  let adminMenuId = $state("");
  let reactionMenuId = $state("");

  let isAdministrator = $derived(viewerRole === "administrator");
  let editingAnnouncement = $derived(
    announcements.find((announcement) => announcement.id === editingId) ?? null,
  );
  let canSubmit = $derived(
    !submitting &&
      draftTitle.trim().length > 0 &&
      draftTitle.trim().length <= 160 &&
      draftContent.trim().length > 0 &&
      draftContent.length <= 50_000,
  );

  onMount(() => {
    void loadAnnouncements();
  });

  async function loadAnnouncements(refresh = false) {
    if (refresh) refreshing = true;
    else loading = true;
    error = "";
    try {
      announcements = await fetchAnnouncements();
    } catch (reason) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to load announcements.";
    } finally {
      loading = false;
      refreshing = false;
    }
  }

  function openNewEditor() {
    if (editorOpen) {
      closeEditor();
      return;
    }
    editingId = "";
    draftTitle = "";
    draftContent = "";
    pendingImages = [];
    editorMode = "write";
    editorOpen = true;
    error = "";
    moveEditorIntoView();
  }

  function moveEditorIntoView() {
    queueMicrotask(() => {
      document.querySelector<HTMLElement>(".dashboard-main")?.scrollTo({
        top: 0,
        behavior: "auto",
      });
    });
  }

  function editAnnouncement(announcement: Announcement) {
    editingId = announcement.id;
    draftTitle = announcement.title;
    draftContent = announcement.content;
    pendingImages = [];
    editorMode = "write";
    editorOpen = true;
    adminMenuId = "";
    reactionMenuId = "";
    error = "";
    moveEditorIntoView();
  }

  function closeEditor() {
    editorOpen = false;
    editingId = "";
    draftTitle = "";
    draftContent = "";
    pendingImages = [];
    editorMode = "write";
    if (imageInput) imageInput.value = "";
  }

  function chooseImages(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    for (const file of files) {
      if (!allowedImageTypes.has(file.type)) {
        error = `${file.name} is not a supported image. Use JPEG, PNG, WebP, or AVIF.`;
        input.value = "";
        return;
      }
      if (file.size === 0 || file.size > maxImageBytes) {
        error = `${file.name} must be between 1 byte and 10 MB.`;
        input.value = "";
        return;
      }
    }
    pendingImages = [...pendingImages, ...files];
    input.value = "";
    error = "";
  }

  function captureImageInput(node: HTMLInputElement) {
    imageInput = node;
    return () => {
      if (imageInput === node) imageInput = undefined;
    };
  }

  function removePendingImage(index: number) {
    pendingImages = pendingImages.filter((_, candidate) => candidate !== index);
  }

  async function saveAnnouncement() {
    if (!canSubmit) return;
    submitting = true;
    error = "";
    try {
      const saved = editingId
        ? await updateAnnouncement(editingId, {
            title: draftTitle.trim(),
            content: draftContent.trimEnd(),
          })
        : await createAnnouncement({
            title: draftTitle.trim(),
            content: draftContent.trimEnd(),
          });
      editingId = saved.id;
      announcements = announcements.some(
        (candidate) => candidate.id === saved.id,
      )
        ? announcements.map((candidate) =>
            candidate.id === saved.id ? saved : candidate,
          )
        : [saved, ...announcements];
      for (const file of [...pendingImages]) {
        const uploaded = await uploadAnnouncementImage(saved.id, file);
        announcements = announcements.map((candidate) =>
          candidate.id === saved.id
            ? { ...candidate, images: [...candidate.images, uploaded] }
            : candidate,
        );
        pendingImages = pendingImages.filter((candidate) => candidate !== file);
      }
      closeEditor();
      announcements = await fetchAnnouncements();
    } catch (reason) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to save the announcement.";
    } finally {
      submitting = false;
    }
  }

  async function removeAnnouncement(announcement: Announcement) {
    adminMenuId = "";
    if (!window.confirm(`Delete “${announcement.title}” and its images?`)) {
      return;
    }
    mutatingId = announcement.id;
    reactionMenuId = "";
    error = "";
    try {
      await deleteAnnouncement(announcement.id);
      announcements = announcements.filter(
        (candidate) => candidate.id !== announcement.id,
      );
      if (editingId === announcement.id) closeEditor();
    } catch (reason) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to delete the announcement.";
    } finally {
      mutatingId = "";
    }
  }

  async function removeStoredImage(
    announcement: Announcement,
    image: AnnouncementImage,
  ) {
    if (!window.confirm(`Remove ${image.file_name} from this announcement?`)) {
      return;
    }
    mutatingId = announcement.id;
    error = "";
    try {
      await deleteAnnouncementImage(announcement.id, image.id);
      announcements = announcements.map((candidate) =>
        candidate.id === announcement.id
          ? {
              ...candidate,
              images: candidate.images.filter(
                (candidateImage) => candidateImage.id !== image.id,
              ),
            }
          : candidate,
      );
    } catch (reason) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to remove the image.";
    } finally {
      mutatingId = "";
    }
  }

  async function toggleReaction(announcement: Announcement, emoji: string) {
    const reaction = announcement.reactions.find(
      (candidate) => candidate.emoji === emoji,
    );
    mutatingId = announcement.id;
    error = "";
    try {
      const updated = await setAnnouncementReaction(
        announcement.id,
        emoji,
        !reaction?.reacted_by_viewer,
      );
      announcements = announcements.map((candidate) =>
        candidate.id === updated.id ? updated : candidate,
      );
      reactionMenuId = "";
    } catch (reason) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to update the reaction.";
    } finally {
      mutatingId = "";
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

  function attachRenderedMarkdown(content: string) {
    return (node: HTMLElement) => {
      node.innerHTML = renderedMarkdown(content);
      for (const link of node.querySelectorAll("a")) {
        link.target = "_blank";
        link.rel = "noreferrer";
      }
      return () => node.replaceChildren();
    };
  }

  function announcementDate(value: string): string {
    return new Intl.DateTimeFormat("en", {
      month: "short",
      day: "numeric",
      year: "numeric",
      hour: "numeric",
      minute: "2-digit",
    }).format(new Date(value));
  }

  function authorInitials(name: string): string {
    const initials = name
      .trim()
      .split(/\s+/)
      .slice(0, 2)
      .map((part) => part.slice(0, 1))
      .join("");
    return initials.toUpperCase() || "?";
  }

  function hideBrokenAvatar(event: Event) {
    if (event.currentTarget instanceof HTMLImageElement) {
      event.currentTarget.remove();
    }
  }

  function toggleAdminMenu(announcementId: string) {
    adminMenuId = adminMenuId === announcementId ? "" : announcementId;
    reactionMenuId = "";
  }

  function closeAdminMenuOnFocusOut(event: FocusEvent, announcementId: string) {
    const anchor = event.currentTarget;
    const nextTarget = event.relatedTarget;
    if (
      anchor instanceof HTMLElement &&
      nextTarget instanceof Node &&
      anchor.contains(nextTarget)
    ) {
      return;
    }
    if (adminMenuId === announcementId) adminMenuId = "";
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    adminMenuId = "";
    reactionMenuId = "";
  }

  function fileSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function wasEdited(announcement: Announcement): boolean {
    return (
      Math.abs(
        new Date(announcement.updated_at).getTime() -
          new Date(announcement.created_at).getTime(),
      ) > 1_000
    );
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<section
  class="announcements-page product-page"
  data-od-id="announcements-page"
>
  <header class="announcements-header page-header">
    <div>
      <TypedHeading
        text="$ announcements --instance"
        odId="announcements-heading"
      />
      <p>
        Service notes, maintenance windows, and other updates from this
        homeserver.
      </p>
    </div>
    <div class="announcements-header-actions" data-od-id="announcement-actions">
      <button
        class="ui-button ui-button--secondary"
        type="button"
        aria-label="Refresh announcements"
        disabled={loading || refreshing}
        onclick={() => loadAnnouncements(true)}
        data-od-id="refresh-announcements"
      >
        <RefreshCw
          class={refreshing ? "spinning" : undefined}
          size={16}
          strokeWidth={1.8}
          aria-hidden="true"
        />
        Refresh
      </button>
      {#if isAdministrator}
        <button
          class={[
            "ui-button",
            editorOpen ? "ui-button--secondary" : "ui-button--primary",
          ]}
          type="button"
          aria-expanded={editorOpen}
          onclick={openNewEditor}
          data-od-id="new-announcement"
        >
          {#if editorOpen}
            <X size={16} strokeWidth={1.8} aria-hidden="true" />
            Close editor
          {:else}
            <Plus size={16} strokeWidth={1.8} aria-hidden="true" />
            New announcement
          {/if}
        </button>
      {/if}
    </div>
  </header>

  {#if error}
    <p
      class="announcements-error"
      role="alert"
      data-od-id="announcements-error"
    >
      {error}
    </p>
  {/if}

  {#if isAdministrator && editorOpen}
    <section class="announcement-editor" data-od-id="announcement-editor">
      <header>
        <div>
          <span>[ ADMIN / PUBLISH ]</span>
          <h2 data-od-id="announcement-editor-heading">
            {editingId ? "Edit announcement" : "Compose an announcement"}
          </h2>
        </div>
        <div
          class="announcement-editor-tabs"
          role="group"
          aria-label="Editor mode"
        >
          <button
            type="button"
            aria-pressed={editorMode === "write"}
            onclick={() => (editorMode = "write")}>Write</button
          >
          <button
            type="button"
            aria-pressed={editorMode === "preview"}
            onclick={() => (editorMode = "preview")}>Preview</button
          >
        </div>
      </header>

      <label class="announcement-title-field">
        <span>Title</span>
        <input
          type="text"
          bind:value={draftTitle}
          maxlength="160"
          placeholder="Maintenance window or homeserver update"
          data-od-id="announcement-title-input"
        />
        <small>{draftTitle.length} / 160</small>
      </label>

      {#if editorMode === "write"}
        <label class="announcement-content-field">
          <span>Markdown</span>
          <textarea
            bind:value={draftContent}
            rows="12"
            maxlength="50000"
            placeholder="Write the update. Headings, links, lists, code, and quotes are supported."
            data-od-id="announcement-content-input"></textarea>
          <small>{draftContent.length.toLocaleString()} / 50,000</small>
        </label>
      {:else}
        <article class="announcement-preview" data-od-id="announcement-preview">
          <h2>{draftTitle.trim() || "Untitled announcement"}</h2>
          {#if draftContent.trim()}
            <div
              class="announcement-markdown"
              {@attach attachRenderedMarkdown(draftContent)}
            ></div>
          {:else}
            <p class="announcement-preview-empty">
              Add Markdown in Write mode to preview the announcement.
            </p>
          {/if}
        </article>
      {/if}

      {#if editingAnnouncement?.images.length}
        <div
          class="announcement-existing-images"
          data-od-id="announcement-existing-images"
        >
          <span>Published images</span>
          <div>
            {#each editingAnnouncement.images as image (image.id)}
              <figure>
                <img
                  src={announcementImageUrl(editingAnnouncement.id, image.id)}
                  alt={image.file_name}
                  loading="lazy"
                />
                <figcaption>
                  <span>{image.file_name}</span>
                  <button
                    type="button"
                    aria-label={`Remove ${image.file_name}`}
                    disabled={mutatingId === editingAnnouncement.id}
                    onclick={() =>
                      removeStoredImage(editingAnnouncement, image)}
                  >
                    <Trash2 size={14} strokeWidth={1.8} aria-hidden="true" />
                  </button>
                </figcaption>
              </figure>
            {/each}
          </div>
        </div>
      {/if}

      {#if pendingImages.length}
        <div
          class="announcement-pending-images"
          data-od-id="announcement-pending-images"
        >
          {#each pendingImages as file, index (`${file.name}-${file.lastModified}-${index}`)}
            <span>
              <ImageIcon size={15} strokeWidth={1.8} aria-hidden="true" />
              <span>{file.name}<small>{fileSize(file.size)}</small></span>
              <button
                type="button"
                aria-label={`Remove ${file.name}`}
                onclick={() => removePendingImage(index)}
              >
                <X size={14} strokeWidth={1.8} aria-hidden="true" />
              </button>
            </span>
          {/each}
        </div>
      {/if}

      <footer>
        <div>
          <input
            class="announcement-image-input"
            type="file"
            accept="image/jpeg,image/png,image/webp,image/avif"
            multiple
            onchange={chooseImages}
            {@attach captureImageInput}
            data-od-id="announcement-image-input"
          />
          <button
            class="ui-button ui-button--ghost"
            type="button"
            onclick={() => imageInput?.click()}
            data-od-id="attach-announcement-images"
          >
            <ImagePlus size={16} strokeWidth={1.8} aria-hidden="true" />
            Add images
          </button>
          <small>JPEG, PNG, WebP, or AVIF · 10 MB each</small>
        </div>
        <div>
          <button
            class="ui-button ui-button--secondary"
            type="button"
            disabled={submitting}
            onclick={closeEditor}>Cancel</button
          >
          <button
            class="ui-button ui-button--primary"
            type="button"
            disabled={!canSubmit}
            onclick={saveAnnouncement}
            data-od-id="publish-announcement"
          >
            <Send size={16} strokeWidth={1.8} aria-hidden="true" />
            {submitting
              ? editingId
                ? "Saving…"
                : "Publishing…"
              : editingId
                ? "Save changes"
                : "Publish"}
          </button>
        </div>
      </footer>
    </section>
  {/if}

  <section
    class="announcements-feed"
    aria-busy={loading || refreshing}
    data-od-id="announcements-feed"
  >
    {#if loading}
      <div class="announcements-state" role="status">
        <span></span>
        Loading announcements…
      </div>
    {:else if announcements.length === 0}
      <div class="announcements-empty" data-od-id="announcements-empty">
        <strong>No announcements yet.</strong>
        <p>
          {isAdministrator
            ? "Publish the first homeserver note when there is something everyone should know."
            : "Administrator updates will appear here."}
        </p>
        {#if isAdministrator && !editorOpen}
          <button
            class="ui-button ui-button--secondary"
            type="button"
            onclick={openNewEditor}>Compose the first note</button
          >
        {/if}
      </div>
    {:else}
      {#each announcements as announcement (announcement.id)}
        <article
          class="announcement-card"
          data-od-id={`announcement-card-${announcement.id}`}
        >
          <header>
            <div class="announcement-title-row">
              <h2 data-od-id={`announcement-title-${announcement.id}`}>
                {announcement.title}
              </h2>
              {#if isAdministrator}
                <div
                  class="announcement-admin-menu"
                  onfocusout={(event) =>
                    closeAdminMenuOnFocusOut(event, announcement.id)}
                >
                  <button
                    class="announcement-admin-menu-trigger"
                    type="button"
                    aria-label={`Actions for ${announcement.title}`}
                    aria-expanded={adminMenuId === announcement.id}
                    aria-controls={`announcement-admin-menu-${announcement.id}`}
                    disabled={mutatingId === announcement.id}
                    onclick={() => toggleAdminMenu(announcement.id)}
                    data-od-id={`manage-announcement-${announcement.id}`}
                  >
                    <Ellipsis size={18} strokeWidth={1.8} aria-hidden="true" />
                  </button>
                  <div
                    id={`announcement-admin-menu-${announcement.id}`}
                    class="announcement-admin-menu-popover"
                    role="menu"
                    aria-label={`${announcement.title} actions`}
                    aria-hidden={adminMenuId !== announcement.id}
                    inert={adminMenuId !== announcement.id}
                    data-od-id={`announcement-menu-${announcement.id}`}
                    {@attach motionPopover(adminMenuId === announcement.id)}
                  >
                    <button
                      type="button"
                      role="menuitem"
                      onclick={() => editAnnouncement(announcement)}
                      data-od-id={`edit-announcement-${announcement.id}`}
                    >
                      <Pencil size={15} strokeWidth={1.8} aria-hidden="true" />
                      Edit
                    </button>
                    <button
                      class="announcement-admin-menu-delete"
                      type="button"
                      role="menuitem"
                      onclick={() => removeAnnouncement(announcement)}
                      data-od-id={`delete-announcement-${announcement.id}`}
                    >
                      <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
                      Delete
                    </button>
                  </div>
                </div>
              {/if}
            </div>
            <div class="announcement-meta">
              <span class="announcement-author-avatar" aria-hidden="true">
                <span>{authorInitials(announcement.author_name)}</span>
                {#if announcement.author_id}
                  <img
                    src={announcementAuthorAvatarUrl(announcement.id)}
                    alt=""
                    loading="lazy"
                    onerror={hideBrokenAvatar}
                  />
                {/if}
              </span>
              <strong>{announcement.author_name}</strong>
              <time datetime={announcement.created_at}
                >Published {announcementDate(announcement.created_at)}</time
              >
              {#if wasEdited(announcement)}
                <span>Edited</span>
              {/if}
            </div>
          </header>
          <div
            class="announcement-markdown"
            {@attach attachRenderedMarkdown(announcement.content)}
          ></div>

          {#if announcement.images.length}
            <div
              class={[
                "announcement-images",
                announcement.images.length === 1 && "single",
              ]}
              data-od-id={`announcement-images-${announcement.id}`}
            >
              {#each announcement.images as image (image.id)}
                <!-- eslint-disable svelte/no-navigation-without-resolve -- authenticated API image -->
                <a
                  href={announcementImageUrl(announcement.id, image.id)}
                  target="_blank"
                  rel="noreferrer"
                  aria-label={`Open ${image.file_name}`}
                >
                  <img
                    src={announcementImageUrl(announcement.id, image.id)}
                    alt={image.file_name}
                    loading="lazy"
                  />
                </a>
              {/each}
            </div>
          {/if}

          <footer>
            <div class="announcement-reactions">
              {#each announcement.reactions as reaction (reaction.emoji)}
                <button
                  class:active={reaction.reacted_by_viewer}
                  type="button"
                  aria-pressed={reaction.reacted_by_viewer}
                  aria-label={`${reaction.emoji} reaction, ${reaction.count}`}
                  disabled={mutatingId === announcement.id}
                  onclick={() => toggleReaction(announcement, reaction.emoji)}
                >
                  <span>{reaction.emoji}</span>{reaction.count}
                </button>
              {/each}
              <button
                type="button"
                aria-label="Add a reaction"
                aria-expanded={reactionMenuId === announcement.id}
                aria-controls={`announcement-reaction-picker-${announcement.id}`}
                onclick={() =>
                  (reactionMenuId =
                    reactionMenuId === announcement.id ? "" : announcement.id)}
              >
                <SmilePlus size={16} strokeWidth={1.8} aria-hidden="true" />
              </button>
              <div
                id={`announcement-reaction-picker-${announcement.id}`}
                class="announcement-reaction-picker"
                aria-label="Choose a reaction"
                aria-hidden={reactionMenuId !== announcement.id}
                inert={reactionMenuId !== announcement.id}
                data-od-id={`announcement-reaction-picker-${announcement.id}`}
                {@attach motionPopover(reactionMenuId === announcement.id)}
              >
                {#each reactionChoices as emoji (emoji)}
                  <button
                    type="button"
                    aria-label={`React with ${emoji}`}
                    onclick={() => toggleReaction(announcement, emoji)}
                    >{emoji}</button
                  >
                {/each}
              </div>
            </div>
          </footer>
        </article>
      {/each}
    {/if}
  </section>
</section>

<style>
  .announcements-page {
    min-height: var(--product-view-height);
    display: flex;
    flex-direction: column;
    gap: 22px;
    color: var(--fg);
  }

  .announcements-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 24px;
    border-bottom: 1px solid var(--border);
  }

  .announcements-header > div:first-child {
    min-width: 0;
  }

  .announcements-header p {
    max-width: 65ch;
    margin: 8px 0 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
  }

  .announcements-header-actions {
    display: flex;
    flex: 0 0 auto;
    gap: 8px;
  }

  .announcements-error {
    margin: 0;
    padding: 12px 14px;
    border: 1px solid var(--danger);
    background: color-mix(in oklch, var(--danger) 10%, var(--surface));
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.5;
  }

  .announcement-editor,
  .announcements-feed {
    width: min(100%, 980px);
    margin-inline: auto;
  }

  .announcement-editor {
    display: grid;
    gap: 18px;
    padding: clamp(18px, 2.4vw, 28px);
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 92%, transparent);
  }

  .announcement-editor > header,
  .announcement-editor > footer,
  .announcement-card > footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .announcement-editor > header span,
  .announcement-existing-images > span {
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.08em;
  }

  .announcement-editor h2 {
    margin: 6px 0 0;
    font-family: var(--font-mono);
    font-size: 20px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .announcement-editor-tabs {
    display: flex;
    border: 1px solid var(--border);
  }

  .announcement-editor-tabs button {
    min-height: 44px;
    padding: 0 16px;
    border: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.02em;
  }

  .announcement-editor-tabs button + button {
    border-left: 1px solid var(--border);
  }

  .announcement-editor-tabs button[aria-pressed="true"] {
    background: var(--fg);
    color: var(--surface);
  }

  .announcement-title-field,
  .announcement-content-field {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.02em;
  }

  .announcement-title-field input,
  .announcement-content-field textarea {
    grid-column: 1 / -1;
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 0;
    background: color-mix(in oklch, var(--bg) 78%, transparent);
    color: var(--fg);
    font: inherit;
    line-height: 1.6;
  }

  .announcement-title-field input {
    min-height: 46px;
    padding: 0 13px;
    font-size: 14px;
  }

  .announcement-content-field textarea {
    min-height: 260px;
    resize: vertical;
    padding: 13px;
    font-size: 13px;
  }

  .announcement-title-field input:focus-visible,
  .announcement-content-field textarea:focus-visible,
  .announcement-editor-tabs button:focus-visible,
  .announcement-card button:focus-visible,
  .announcement-images a:focus-visible,
  .announcement-existing-images button:focus-visible,
  .announcement-pending-images button:focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }

  .announcement-preview {
    min-height: 260px;
    padding: clamp(18px, 2.4vw, 28px);
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--bg) 78%, transparent);
  }

  .announcement-preview > h2 {
    margin: 0 0 18px;
    color: var(--fg);
    font-size: clamp(22px, 3vw, 30px);
    line-height: 1.2;
  }

  .announcement-preview-empty {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .announcement-existing-images {
    display: grid;
    gap: 10px;
  }

  .announcement-existing-images > div {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 10px;
  }

  .announcement-existing-images figure {
    min-width: 0;
    margin: 0;
    border: 1px solid var(--border);
    background: var(--bg);
  }

  .announcement-existing-images img {
    width: 100%;
    height: 120px;
    display: block;
    object-fit: cover;
  }

  .announcement-existing-images figcaption {
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-left: 10px;
    border-top: 1px solid var(--border);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .announcement-existing-images figcaption span {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .announcement-existing-images button,
  .announcement-pending-images button {
    min-width: 44px;
    min-height: 44px;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--fg);
  }

  .announcement-pending-images {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .announcement-pending-images > span {
    min-height: 44px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding-left: 11px;
    border: 1px solid var(--border);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .announcement-pending-images small {
    display: block;
    margin-top: 2px;
    color: var(--muted);
    font-size: 9px;
  }

  .announcement-image-input {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }

  .announcement-editor > footer > div {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .announcement-editor > footer small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .announcements-feed {
    display: grid;
    gap: 18px;
    padding-bottom: 42px;
  }

  .announcement-card {
    position: relative;
    min-width: 0;
    padding: clamp(20px, 3vw, 34px);
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--surface) 88%, transparent);
  }

  .announcement-card > header {
    display: grid;
    gap: 12px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }

  .announcement-title-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 18px;
    min-width: 0;
  }

  .announcement-title-row h2 {
    max-width: 32ch;
    min-width: 0;
    margin: 0;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: clamp(24px, 4vw, 38px);
    font-weight: 600;
    line-height: 1.2;
    letter-spacing: -0.02em;
    overflow-wrap: anywhere;
    text-wrap: balance;
  }

  .announcement-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.5;
    letter-spacing: 0.02em;
  }

  .announcement-meta strong {
    color: var(--fg);
    font-weight: 600;
  }

  .announcement-author-avatar {
    position: relative;
    width: 34px;
    height: 34px;
    display: grid;
    overflow: hidden;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--border);
    background: var(--fg);
    color: var(--surface);
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.05em;
  }

  .announcement-author-avatar > img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .announcement-admin-menu {
    position: relative;
    flex: 0 0 auto;
  }

  .announcement-admin-menu-trigger,
  .announcement-admin-menu-popover button,
  .announcement-reactions > button,
  .announcement-reaction-picker button {
    min-width: 44px;
    min-height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1;
  }

  .announcement-admin-menu-trigger {
    padding: 0;
    border-color: var(--border);
  }

  .announcement-admin-menu-trigger:hover:not(:disabled),
  .announcement-admin-menu-popover button:hover:not(:disabled),
  .announcement-reactions > button:hover:not(:disabled) {
    border-color: var(--border);
    background: color-mix(in oklch, var(--fg) 8%, transparent);
    color: var(--fg);
  }

  .announcement-admin-menu-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 5;
    width: 158px;
    display: grid;
    padding: 5px;
    border: 1px solid var(--border);
    background: var(--surface);
    box-shadow: 0 14px 34px color-mix(in oklch, var(--bg) 68%, transparent);
  }

  .announcement-admin-menu-popover button {
    justify-content: flex-start;
    width: 100%;
    padding: 0 11px;
  }

  .announcement-admin-menu-popover .announcement-admin-menu-delete {
    color: var(--danger);
  }

  .announcement-card > .announcement-markdown {
    margin-top: clamp(22px, 3vw, 30px);
  }

  .announcement-markdown {
    max-width: 72ch;
    color: var(--fg);
    font-family: var(--font-body);
    font-size: 15px;
    line-height: 1.65;
    overflow-wrap: anywhere;
  }

  .announcement-markdown :global(:first-child) {
    margin-top: 0;
  }

  .announcement-markdown :global(:last-child) {
    margin-bottom: 0;
  }

  .announcement-markdown :global(h1),
  .announcement-markdown :global(h2),
  .announcement-markdown :global(h3) {
    margin: 1.5em 0 0.55em;
    color: var(--fg);
    font-family: var(--font-mono);
    line-height: 1.25;
  }

  .announcement-markdown :global(h1) {
    font-size: 24px;
  }

  .announcement-markdown :global(h2) {
    font-size: 20px;
  }

  .announcement-markdown :global(h3) {
    font-size: 17px;
  }

  .announcement-markdown :global(p),
  .announcement-markdown :global(ul),
  .announcement-markdown :global(ol),
  .announcement-markdown :global(blockquote),
  .announcement-markdown :global(pre) {
    margin: 0 0 1em;
  }

  .announcement-markdown :global(a) {
    color: var(--fg);
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .announcement-markdown :global(blockquote) {
    padding-left: 16px;
    border-left: 2px solid var(--border);
    color: var(--muted);
  }

  .announcement-markdown :global(code) {
    padding: 0.15em 0.35em;
    background: var(--bg);
    font-family: var(--font-mono);
    font-size: 0.9em;
  }

  .announcement-markdown :global(pre) {
    max-width: 100%;
    overflow-x: auto;
    padding: 14px;
    border: 1px solid var(--border);
    background: var(--bg);
  }

  .announcement-markdown :global(pre code) {
    padding: 0;
    background: transparent;
  }

  .announcement-images {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 1px;
    margin-top: 26px;
    border: 1px solid var(--border);
    background: var(--border);
  }

  .announcement-images.single {
    grid-template-columns: 1fr;
  }

  .announcement-images a {
    min-width: 0;
    display: block;
    background: var(--bg);
  }

  .announcement-images img {
    width: 100%;
    height: clamp(220px, 38vw, 430px);
    display: block;
    object-fit: cover;
  }

  .announcement-images.single img {
    object-fit: contain;
  }

  .announcement-card > footer {
    justify-content: flex-start;
    margin-top: 24px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }

  .announcement-reactions {
    position: relative;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .announcement-reactions > button {
    min-width: 44px;
    padding: 0 10px;
    border-color: var(--border);
  }

  .announcement-reactions > button.active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }

  .announcement-reaction-picker {
    position: absolute;
    left: 0;
    bottom: calc(100% + 8px);
    z-index: 4;
    display: flex;
    padding: 6px;
    border: 1px solid var(--border);
    background: var(--surface);
  }

  .announcement-reaction-picker button {
    min-width: 44px;
    font-size: 18px;
  }

  .announcement-reaction-picker button:hover {
    border-color: var(--border);
    background: color-mix(in oklch, var(--fg) 8%, transparent);
    color: var(--fg);
  }

  .announcements-state,
  .announcements-empty {
    min-height: 190px;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 10px;
    padding: 28px;
    border: 1px solid var(--border);
    color: var(--muted);
    text-align: center;
  }

  .announcements-state {
    grid-template-columns: auto auto;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .announcements-state > span {
    width: 8px;
    height: 8px;
    background: var(--fg);
  }

  .announcements-empty strong {
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 16px;
  }

  .announcements-empty p {
    max-width: 54ch;
    margin: 0;
    font-size: 13px;
    line-height: 1.6;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }

  @media (max-width: 720px) {
    .announcements-page {
      gap: 18px;
    }

    .announcements-header,
    .announcement-editor > header,
    .announcement-editor > footer {
      align-items: stretch;
      flex-direction: column;
    }

    .announcements-header-actions,
    .announcement-editor-tabs,
    .announcement-editor > footer > div {
      width: 100%;
    }

    .announcements-header-actions > button,
    .announcement-editor-tabs > button,
    .announcement-editor > footer > div > .ui-button {
      flex: 1;
    }

    .announcement-editor > footer small {
      display: none;
    }

    .announcement-card {
      padding: 20px;
    }

    .announcement-images {
      grid-template-columns: 1fr;
    }

    .announcement-images img {
      height: min(72vw, 360px);
    }

    .announcement-reaction-picker {
      max-width: min(100%, calc(100vw - 72px));
      overflow-x: auto;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(.spinning) {
      animation: none;
    }
  }
</style>

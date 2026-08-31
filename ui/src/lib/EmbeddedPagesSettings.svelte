<script lang="ts">
  import ArrowDown from "lucide-svelte/icons/arrow-down";
  import ArrowRightLeft from "lucide-svelte/icons/arrow-right-left";
  import ArrowUp from "lucide-svelte/icons/arrow-up";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import EmbeddedPageIcon from "$lib/EmbeddedPageIcon.svelte";
  import {
    createGlobalEmbeddedPage,
    createPersonalEmbeddedPage,
    deleteGlobalEmbeddedPage,
    deletePersonalEmbeddedPage,
    moveEmbeddedPageScope,
    reorderGlobalEmbeddedPages,
    reorderPersonalEmbeddedPages,
    updateGlobalEmbeddedPage,
    updatePersonalEmbeddedPage,
    type EmbeddedPage,
    type EmbeddedPageIconKind,
    type EmbeddedPageInput,
    type EmbeddedPagesResponse,
    type EmbeddedPageScope,
  } from "$lib/api";

  const DEFAULT_IFRAME_HEIGHT = 720;
  const lucideIconNames = [
    "bell",
    "book-open",
    "bookmark",
    "briefcase",
    "calendar-days",
    "cloud",
    "code",
    "database",
    "folder",
    "gamepad-2",
    "git-branch",
    "globe",
    "heart",
    "house",
    "image",
    "link",
    "lock",
    "mail",
    "music",
    "podcast",
    "rocket",
    "rss",
    "shopping-bag",
    "star",
    "terminal",
    "video",
    "wrench",
  ];
  type HeightOption = "480" | "720" | "1080" | "custom";

  type Props = {
    pages: EmbeddedPagesResponse;
    isAdministrator: boolean;
    onPagesChange: (pages: EmbeddedPagesResponse) => void;
    onPageDeleted: (pageId: string) => void;
  };

  let {
    pages,
    isAdministrator,
    onPagesChange,
    onPageDeleted,
  }: Props = $props();

  let formOpen = $state(false);
  let formScope = $state<EmbeddedPageScope>("user");
  let editingId = $state("");
  let formTitle = $state("");
  let formDescription = $state("");
  let formUrl = $state("");
  let formIconKind = $state<EmbeddedPageIconKind>("favicon");
  let formIconValue = $state("");
  let formAllowScripts = $state(false);
  let formAllowSameOrigin = $state(false);
  let formIframeHeight = $state(DEFAULT_IFRAME_HEIGHT);
  let formHeightOption = $state<HeightOption>("720");
  let formError = $state("");
  let listError = $state("");
  let busyAction = $state("");
  let pendingDeleteId = $state("");
  let pendingScopeMoveId = $state("");

  function collectionKey(scope: EmbeddedPageScope): "global" | "personal" {
    return scope === "global" ? "global" : "personal";
  }

  function pageHost(page: EmbeddedPage) {
    try {
      return new URL(page.url).host;
    } catch {
      return page.url;
    }
  }

  function heightOptionFor(height: number): HeightOption {
    if (height === 480 || height === 720 || height === 1080) {
      return String(height) as HeightOption;
    }
    return "custom";
  }

  function chooseHeightOption(option: HeightOption) {
    formHeightOption = option;
    if (option !== "custom") formIframeHeight = Number(option);
  }

  function chooseIconKind(kind: EmbeddedPageIconKind) {
    if (formIconKind === kind) return;
    formIconKind = kind;
    formIconValue = kind === "lucide" ? "bookmark" : "";
  }

  function openCreate(scope: EmbeddedPageScope) {
    if (scope === "global" && !isAdministrator) return;
    formScope = scope;
    editingId = "";
    formTitle = "";
    formDescription = "";
    formUrl = "";
    formIconKind = "favicon";
    formIconValue = "";
    formAllowScripts = false;
    formAllowSameOrigin = false;
    formIframeHeight = DEFAULT_IFRAME_HEIGHT;
    formHeightOption = "720";
    formError = "";
    pendingDeleteId = "";
    pendingScopeMoveId = "";
    formOpen = true;
  }

  function openEdit(page: EmbeddedPage) {
    formScope = page.scope;
    editingId = page.id;
    formTitle = page.title;
    formDescription = page.description;
    formUrl = page.url;
    formIconKind = page.icon_kind;
    formIconValue = page.icon_value ?? "";
    formAllowScripts = page.allow_scripts;
    formAllowSameOrigin = page.allow_same_origin;
    formIframeHeight = page.iframe_height;
    formHeightOption = heightOptionFor(page.iframe_height);
    formError = "";
    pendingDeleteId = "";
    pendingScopeMoveId = "";
    formOpen = true;
  }

  function closeForm() {
    if (busyAction) return;
    formOpen = false;
    editingId = "";
    formError = "";
  }

  async function savePage(event: SubmitEvent) {
    event.preventDefault();
    if (busyAction) return;
    busyAction = editingId ? `edit:${editingId}` : `create:${formScope}`;
    formError = "";
    const input: EmbeddedPageInput = {
      title: formTitle,
      description: formDescription,
      url: formUrl,
      icon_kind: formIconKind,
      icon_value:
        formIconKind === "favicon" ? null : formIconValue.trim() || null,
      allow_scripts: formAllowScripts,
      allow_same_origin: formAllowSameOrigin,
      iframe_height: formIframeHeight,
    };
    try {
      const saved = editingId
        ? formScope === "global"
          ? await updateGlobalEmbeddedPage(editingId, input)
          : await updatePersonalEmbeddedPage(editingId, input)
        : formScope === "global"
          ? await createGlobalEmbeddedPage(input)
          : await createPersonalEmbeddedPage(input);
      const key = collectionKey(formScope);
      const current = pages[key];
      const next = editingId
        ? current.map((page) => (page.id === saved.id ? saved : page))
        : [...current, saved];
      onPagesChange({ ...pages, [key]: next });
      formOpen = false;
      editingId = "";
    } catch (reason: unknown) {
      formError =
        reason instanceof Error ? reason.message : "Unable to save embedded page";
    } finally {
      busyAction = "";
    }
  }

  async function movePage(
    scope: EmbeddedPageScope,
    pageId: string,
    direction: -1 | 1,
  ) {
    if (busyAction) return;
    const key = collectionKey(scope);
    const current = pages[key];
    const index = current.findIndex((page) => page.id === pageId);
    const nextIndex = index + direction;
    if (index < 0 || nextIndex < 0 || nextIndex >= current.length) return;
    const next = [...current];
    [next[index], next[nextIndex]] = [next[nextIndex], next[index]];
    busyAction = `order:${scope}`;
    listError = "";
    try {
      const ordered =
        scope === "global"
          ? await reorderGlobalEmbeddedPages(next.map((page) => page.id))
          : await reorderPersonalEmbeddedPages(next.map((page) => page.id));
      onPagesChange({ ...pages, [key]: ordered });
    } catch (reason: unknown) {
      listError =
        reason instanceof Error
          ? reason.message
          : "Unable to reorder embedded pages";
    } finally {
      busyAction = "";
    }
  }

  async function removePage(page: EmbeddedPage) {
    if (busyAction) return;
    busyAction = `delete:${page.id}`;
    listError = "";
    try {
      if (page.scope === "global") {
        await deleteGlobalEmbeddedPage(page.id);
      } else {
        await deletePersonalEmbeddedPage(page.id);
      }
      const key = collectionKey(page.scope);
      onPagesChange({
        ...pages,
        [key]: pages[key].filter((candidate) => candidate.id !== page.id),
      });
      pendingDeleteId = "";
      if (editingId === page.id) {
        formOpen = false;
        editingId = "";
        formError = "";
      }
      onPageDeleted(page.id);
    } catch (reason: unknown) {
      listError =
        reason instanceof Error ? reason.message : "Unable to delete embedded page";
    } finally {
      busyAction = "";
    }
  }

  async function transferPage(page: EmbeddedPage) {
    if (busyAction || !isAdministrator) return;
    const targetScope: EmbeddedPageScope =
      page.scope === "global" ? "user" : "global";
    const sourceKey = collectionKey(page.scope);
    const targetKey = collectionKey(targetScope);
    busyAction = `scope:${page.id}`;
    listError = "";
    try {
      const moved = await moveEmbeddedPageScope(page.id, targetScope);
      onPagesChange({
        ...pages,
        [sourceKey]: pages[sourceKey].filter(
          (candidate) => candidate.id !== page.id,
        ),
        [targetKey]: [...pages[targetKey], moved],
      });
      pendingScopeMoveId = "";
      if (editingId === page.id) {
        formOpen = false;
        editingId = "";
        formError = "";
      }
    } catch (reason: unknown) {
      listError =
        reason instanceof Error
          ? reason.message
          : "Unable to change embedded page scope";
    } finally {
      busyAction = "";
    }
  }
</script>

{#snippet pageGroup(
  scope: EmbeddedPageScope,
  title: string,
  description: string,
  entries: EmbeddedPage[],
)}
  <section
    class="embedded-pages-group"
    aria-labelledby={`embedded-pages-${scope}-heading`}
    data-od-id={`embedded-pages-${scope}-group`}
  >
    <div class="embedded-pages-group-heading">
      <div>
        <p class="widget-kicker">
          [ {scope === "global" ? "INSTANCE" : "ACCOUNT"} ]
        </p>
        <h3 id={`embedded-pages-${scope}-heading`}>{title}</h3>
        <p>{description}</p>
      </div>
      <button
        class="ui-button ui-button--secondary"
        type="button"
        disabled={busyAction !== ""}
        onclick={() => openCreate(scope)}
        data-od-id={`add-${scope}-embedded-page`}
      >
        <Plus size={16} strokeWidth={1.8} aria-hidden="true" />
        Add page
      </button>
    </div>

    <div class="embedded-pages-list">
      {#each entries as page, index (page.id)}
        <article
          class="embedded-page-settings-row"
          data-od-id={`embedded-page-settings-${page.id}`}
        >
          <div class="embedded-page-settings-icon" aria-hidden="true">
            <EmbeddedPageIcon
              pageId={page.id}
              updatedAt={page.updated_at}
              iconKind={page.icon_kind}
              iconValue={page.icon_value}
              size={18}
            />
          </div>
          <div class="embedded-page-settings-copy">
            <div>
              <strong>{page.title}</strong>
              <span class="embedded-page-scope-badge">
                {scope === "global" ? "GLOBAL · CUSTOM" : "PERSONAL · CUSTOM"}
              </span>
              {#if page.allow_scripts}
                <span class="embedded-page-permission-badge">SCRIPTS</span>
              {/if}
              {#if page.allow_same_origin}
                <span class="embedded-page-permission-badge">SAME ORIGIN</span>
              {/if}
            </div>
            <span>{pageHost(page)}</span>
            <small>
              {page.description || "No description"} · {page.iframe_height}px high
            </small>
          </div>
          <div class="embedded-page-settings-controls">
            <button
              class="ui-button ui-button--ghost ui-button--icon"
              type="button"
              aria-label={`Move ${page.title} up`}
              disabled={index === 0 || busyAction !== ""}
              onclick={() => void movePage(scope, page.id, -1)}
              data-od-id={`move-embedded-page-${page.id}-up`}
            >
              <ArrowUp size={16} strokeWidth={1.8} aria-hidden="true" />
            </button>
            <button
              class="ui-button ui-button--ghost ui-button--icon"
              type="button"
              aria-label={`Move ${page.title} down`}
              disabled={index === entries.length - 1 || busyAction !== ""}
              onclick={() => void movePage(scope, page.id, 1)}
              data-od-id={`move-embedded-page-${page.id}-down`}
            >
              <ArrowDown size={16} strokeWidth={1.8} aria-hidden="true" />
            </button>
            {#if isAdministrator}
              <button
                class="ui-button ui-button--ghost ui-button--icon"
                type="button"
                aria-label={page.scope === "global"
                  ? `Move ${page.title} to my pages`
                  : `Make ${page.title} global`}
                title={page.scope === "global"
                  ? "Move to My pages"
                  : "Make global"}
                disabled={busyAction !== ""}
                onclick={() => {
                  pendingDeleteId = "";
                  pendingScopeMoveId = page.id;
                }}
                data-od-id={`change-embedded-page-${page.id}-scope`}
              >
                <ArrowRightLeft size={16} strokeWidth={1.8} aria-hidden="true" />
              </button>
            {/if}
            <button
              class="ui-button ui-button--ghost ui-button--icon"
              type="button"
              aria-label={`Edit ${page.title}`}
              disabled={busyAction !== ""}
              onclick={() => openEdit(page)}
              data-od-id={`edit-embedded-page-${page.id}`}
            >
              <Pencil size={16} strokeWidth={1.8} aria-hidden="true" />
            </button>
            <button
              class="ui-button ui-button--danger ui-button--icon"
              type="button"
              aria-label={`Delete ${page.title}`}
              disabled={busyAction !== ""}
              onclick={() => {
                pendingScopeMoveId = "";
                pendingDeleteId = page.id;
              }}
              data-od-id={`delete-embedded-page-${page.id}`}
            >
              <Trash2 size={16} strokeWidth={1.8} aria-hidden="true" />
            </button>
          </div>
          {#if pendingScopeMoveId === page.id}
            <div
              class="embedded-page-delete-confirmation embedded-page-scope-confirmation"
              data-od-id={`confirm-change-embedded-page-${page.id}-scope`}
            >
              <p>
                <strong>
                  {page.scope === "global"
                    ? `Move ${page.title} to My pages?`
                    : `Make ${page.title} global?`}
                </strong>
                <span>
                  {page.scope === "global"
                    ? "It will disappear from every other account and become private to you."
                    : "It will become available to every signed-in account."}
                </span>
              </p>
              <div>
                <button
                  class="ui-button ui-button--secondary"
                  type="button"
                  disabled={busyAction !== ""}
                  onclick={() => (pendingScopeMoveId = "")}
                  data-od-id={`keep-embedded-page-${page.id}-scope`}
                >
                  Keep current scope
                </button>
                <button
                  class="ui-button ui-button--primary"
                  type="button"
                  disabled={busyAction !== ""}
                  onclick={() => void transferPage(page)}
                  data-od-id={`commit-change-embedded-page-${page.id}-scope`}
                >
                  {busyAction === `scope:${page.id}`
                    ? "Moving…"
                    : page.scope === "global"
                      ? "Move to My pages"
                      : "Make global"}
                </button>
              </div>
            </div>
          {/if}
          {#if pendingDeleteId === page.id}
            <div
              class="embedded-page-delete-confirmation"
              data-od-id={`confirm-delete-embedded-page-${page.id}`}
            >
              <p>
                <strong>Delete {page.title}?</strong>
                <span>This removes the sidebar entry for its entire scope.</span>
              </p>
              <div>
                <button
                  class="ui-button ui-button--secondary"
                  type="button"
                  disabled={busyAction !== ""}
                  onclick={() => (pendingDeleteId = "")}>Keep page</button
                >
                <button
                  class="ui-button ui-button--danger"
                  type="button"
                  disabled={busyAction !== ""}
                  onclick={() => void removePage(page)}
                >
                  {busyAction === `delete:${page.id}`
                    ? "Deleting…"
                    : "Confirm delete"}
                </button>
              </div>
            </div>
          {/if}
        </article>
      {:else}
        <p class="empty-state embedded-pages-empty">
          No {scope === "global" ? "global" : "personal"} custom pages yet.
        </p>
      {/each}
    </div>
  </section>
{/snippet}

<div
  class="embedded-pages-dialog-body embedded-pages-settings-body"
  data-od-id="custom-pages-settings"
>
    {#if listError}
      <p class="form-error" role="alert">{listError}</p>
    {/if}

    {#if isAdministrator}
      {@render pageGroup(
        "global",
        "Global pages",
        "Available to every signed-in account and ordered by administrators.",
        pages.global,
      )}
    {/if}
    {@render pageGroup(
      "user",
      "My pages",
      "Private sidebar destinations available only to this account.",
      pages.personal,
    )}

    {#if formOpen}
      <form
        class="embedded-page-form"
        onsubmit={savePage}
        data-od-id="embedded-page-form"
      >
        <div class="embedded-page-form-heading">
          <div>
            <p class="widget-kicker">[ {editingId ? "EDIT" : "NEW"} ]</p>
            <h3>{editingId ? "Update embedded page" : "Add embedded page"}</h3>
          </div>
          <span class="embedded-page-scope-badge">
            {formScope === "global" ? "GLOBAL · CUSTOM" : "PERSONAL · CUSTOM"}
          </span>
        </div>

        <label for="embedded-page-title">Header and sidebar label</label>
        <input
          id="embedded-page-title"
          class="text-input"
          bind:value={formTitle}
          maxlength="80"
          required
        />

        <label for="embedded-page-description">Description</label>
        <textarea
          id="embedded-page-description"
          class="text-input"
          bind:value={formDescription}
          maxlength="280"
          rows="3"
          placeholder="Shown below the page header and in sidebar hints"
        ></textarea>

        <label for="embedded-page-url">HTTPS webpage URL</label>
        <input
          id="embedded-page-url"
          class="text-input"
          type="url"
          bind:value={formUrl}
          maxlength="2000"
          pattern="https://.*"
          placeholder="https://example.com/embed"
          required
        />
        <p class="field-note">
          Pages use a restricted sandbox by default. Some websites refuse
          embedding; those pages can still be opened externally.
        </p>

        <fieldset class="embedded-page-icon-source">
          <legend>Sidebar icon</legend>
          <div>
            <button
              type="button"
              aria-pressed={formIconKind === "favicon"}
              disabled={busyAction !== ""}
              onclick={() => chooseIconKind("favicon")}
              data-od-id="embedded-page-icon-favicon">Favicon</button
            >
            <button
              type="button"
              aria-pressed={formIconKind === "lucide"}
              disabled={busyAction !== ""}
              onclick={() => chooseIconKind("lucide")}
              data-od-id="embedded-page-icon-lucide">Lucide</button
            >
            <button
              type="button"
              aria-pressed={formIconKind === "custom"}
              disabled={busyAction !== ""}
              onclick={() => chooseIconKind("custom")}
              data-od-id="embedded-page-icon-custom">Custom URL</button
            >
          </div>
        </fieldset>

        {#if formIconKind === "lucide"}
          <label for="embedded-page-lucide-icon">Lucide icon name</label>
          <select
            id="embedded-page-lucide-icon"
            class="select-input embedded-page-icon-value"
            bind:value={formIconValue}
            required
            data-od-id="embedded-page-lucide-select"
          >
            {#each lucideIconNames as iconName (iconName)}
              <option value={iconName}>{iconName}</option>
            {/each}
          </select>
        {:else if formIconKind === "custom"}
          <label for="embedded-page-icon-url">Custom icon URL</label>
          <input
            id="embedded-page-icon-url"
            class="text-input"
            type="url"
            bind:value={formIconValue}
            maxlength="2000"
            pattern="https://.*"
            placeholder="https://example.com/icon.svg"
            required
            data-od-id="embedded-page-custom-icon-url"
          />
          <p class="field-note">
            Use a direct credential-free HTTPS image URL. Pandan fetches and
            stores a local copy.
          </p>
        {:else}
          <p class="field-note">
            Pandan discovers the site's favicon and stores a local copy.
          </p>
        {/if}
        <p class="field-note">
          If a remote image cannot be cached, Pandan shows the default custom-page
          icon.
        </p>

        <label for="embedded-page-height-preset">Iframe height</label>
        <select
          id="embedded-page-height-preset"
          class="select-input embedded-page-height-preset"
          value={formHeightOption}
          onchange={(event) =>
            chooseHeightOption(event.currentTarget.value as HeightOption)}
          data-od-id="embedded-page-height-preset"
        >
          <option value="480">Compact · 480px</option>
          <option value="720">Standard · 720px</option>
          <option value="1080">Tall · 1080px</option>
          <option value="custom">Custom</option>
        </select>
        {#if formHeightOption === "custom"}
          <label for="embedded-page-height">Custom iframe height</label>
          <div class="embedded-page-height-control">
            <input
              id="embedded-page-height"
              class="text-input"
              type="number"
              bind:value={formIframeHeight}
              min="320"
              max="2400"
              step="40"
              inputmode="numeric"
              required
              data-od-id="embedded-page-height-custom"
            />
            <span aria-hidden="true">px</span>
          </div>
        {/if}
        <p class="field-note">
          Custom heights may be set between 320 and 2400 pixels. The iframe width
          remains responsive.
        </p>

        <fieldset class="embedded-page-permissions">
          <legend>Iframe permissions</legend>
          <button
            class="ui-toggle-button embedded-page-permission-toggle"
            type="button"
            aria-pressed={formAllowScripts}
            aria-describedby="embedded-page-scripts-warning"
            disabled={busyAction !== ""}
            onclick={() => (formAllowScripts = !formAllowScripts)}
            data-od-id="toggle-embedded-page-scripts"
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>
              <strong>Allow scripts</strong>
              <small>
                {formAllowScripts ? "Script execution enabled" : "Scripts blocked"}
              </small>
            </span>
          </button>
          <p id="embedded-page-scripts-warning" class="field-note">
            Enables JavaScript inside the embedded page. Interactive applications
            may require this permission.
          </p>

          <button
            class="ui-toggle-button embedded-page-permission-toggle"
            type="button"
            aria-pressed={formAllowSameOrigin}
            aria-describedby="embedded-page-origin-warning"
            disabled={busyAction !== ""}
            onclick={() => (formAllowSameOrigin = !formAllowSameOrigin)}
            data-od-id="toggle-embedded-page-same-origin"
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>
              <strong>Allow same-origin</strong>
              <small>
                {formAllowSameOrigin
                  ? "Same-origin access enabled"
                  : "Opaque sandbox origin"}
              </small>
            </span>
          </button>
          <p id="embedded-page-origin-warning" class="field-note">
            Preserves the embedded site's real origin for storage, cookies, and
            origin checks. It cannot override a site's embedding policy.
          </p>

          {#if formAllowScripts && formAllowSameOrigin}
            <p class="embedded-page-permission-warning" role="status">
              Both permissions are enabled. This is the least isolated sandbox
              mode; use it only for a page you trust.
            </p>
          {/if}
        </fieldset>

        {#if formError}
          <p class="form-error" role="alert">{formError}</p>
        {/if}

        <div class="embedded-page-form-actions">
          <button
            class="ui-button ui-button--ghost"
            type="button"
            disabled={busyAction !== ""}
            onclick={closeForm}>Cancel</button
          >
          <button
            class="ui-button ui-button--primary"
            type="submit"
            disabled={busyAction !== ""}
            data-od-id="save-embedded-page"
          >
            {busyAction.startsWith("create:") ||
            busyAction.startsWith("edit:")
              ? "Saving…"
              : editingId
                ? "Save page"
                : "Add page"}
          </button>
        </div>
      </form>
    {/if}
</div>

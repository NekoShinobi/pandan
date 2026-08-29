<script lang="ts">
  import Ellipsis from "lucide-svelte/icons/ellipsis";
  import FolderPlus from "lucide-svelte/icons/folder-plus";
  import Link2 from "lucide-svelte/icons/link-2";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import Search from "lucide-svelte/icons/search";
  import X from "lucide-svelte/icons/x";
  import { onMount } from "svelte";
  import "./BookmarksPage.css";
  import BookmarkLibraryIcon from "$lib/BookmarkLibraryIcon.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    createBookmarkLibraryCategory,
    createBookmarkLibraryItem,
    deleteBookmarkLibraryCategory,
    deleteBookmarkLibraryItem,
    fetchBookmarkLibrary,
    updateBookmarkLibraryCategory,
    updateBookmarkLibraryItem,
    type BookmarkLibraryCategory,
    type BookmarkLibraryIconKind,
    type BookmarkLibraryItem,
    type BookmarkLibraryResponse,
    type BookmarkLibraryScope,
  } from "$lib/api";

  let {
    administrator,
    onToast,
  }: {
    administrator: boolean;
    onToast: (message: string) => void;
  } = $props();

  const emptyLibrary: BookmarkLibraryResponse = {
    global: [],
    personal: [],
  };
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

  let library = $state.raw<BookmarkLibraryResponse>(emptyLibrary);
  let loaded = $state(false);
  let loading = $state(false);
  let loadError = $state("");
  let searchQuery = $state("");
  let createMenuOpen = $state(false);
  let bookmarkMenuId = $state("");
  let categoryMenuId = $state("");

  let bookmarkDialog = $state<HTMLDialogElement>();
  let editingBookmark = $state.raw<BookmarkLibraryItem | null>(null);
  let bookmarkScope = $state<BookmarkLibraryScope>("personal");
  let bookmarkCategoryId = $state("");
  let bookmarkTitle = $state("");
  let bookmarkUrl = $state("");
  let bookmarkIconKind = $state<BookmarkLibraryIconKind>("favicon");
  let bookmarkIconValue = $state("");
  let bookmarkSaving = $state(false);
  let bookmarkDeleting = $state(false);
  let bookmarkDeletePending = $state(false);
  let bookmarkFormError = $state("");

  let categoryDialog = $state<HTMLDialogElement>();
  let editingCategory = $state.raw<BookmarkLibraryCategory | null>(null);
  let categoryScope = $state<BookmarkLibraryScope>("personal");
  let categoryName = $state("");
  let categorySaving = $state(false);
  let categoryDeleting = $state(false);
  let categoryDeletePending = $state(false);
  let categoryFormError = $state("");

  let allCategories = $derived([...library.global, ...library.personal]);
  let bookmarkCategoryOptions = $derived(
    bookmarkScope === "global" ? library.global : library.personal,
  );
  let visibleCategories = $derived.by(() => {
    const query = searchQuery.trim().toLocaleLowerCase();
    if (!query) return allCategories;
    return allCategories
      .map((category) => ({
        ...category,
        bookmarks: category.bookmarks.filter((bookmark) =>
          `${bookmark.title} ${bookmark.url} ${category.name}`
            .toLocaleLowerCase()
            .includes(query),
        ),
      }))
      .filter((category) => category.bookmarks.length > 0);
  });

  onMount(() => {
    void loadLibrary(true);
  });

  async function loadLibrary(initial: boolean) {
    if (loading) return;
    loading = true;
    if (initial) loadError = "";
    try {
      library = await fetchBookmarkLibrary();
      loaded = true;
      loadError = "";
    } catch (reason: unknown) {
      loadError =
        reason instanceof Error
          ? reason.message
          : "Unable to load bookmarks";
    } finally {
      loading = false;
    }
  }

  function closeMenus() {
    createMenuOpen = false;
    bookmarkMenuId = "";
    categoryMenuId = "";
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") closeMenus();
  }

  function canEditCategory(category: BookmarkLibraryCategory) {
    return category.scope === "personal" || administrator;
  }

  function openBookmarkEditor(
    bookmark: BookmarkLibraryItem | null = null,
    category: BookmarkLibraryCategory | null = null,
  ) {
    closeMenus();
    if (!bookmark && allCategories.length === 0) {
      openCategoryEditor();
      onToast("Create a category before adding a bookmark");
      return;
    }
    editingBookmark = bookmark;
    bookmarkDeletePending = false;
    bookmarkFormError = "";
    if (bookmark && category) {
      bookmarkScope = category.scope;
      bookmarkCategoryId = bookmark.category_id;
      bookmarkTitle = bookmark.title;
      bookmarkUrl = bookmark.url;
      bookmarkIconKind = bookmark.icon_kind;
      bookmarkIconValue = bookmark.icon_value ?? "";
    } else {
      bookmarkScope =
        library.personal.length > 0 || !administrator ? "personal" : "global";
      const categories =
        bookmarkScope === "global" ? library.global : library.personal;
      bookmarkCategoryId = categories[0]?.id ?? "";
      bookmarkTitle = "";
      bookmarkUrl = "";
      bookmarkIconKind = "favicon";
      bookmarkIconValue = "";
    }
    bookmarkDialog?.showModal();
  }

  function closeBookmarkEditor() {
    if (bookmarkSaving || bookmarkDeleting) return;
    bookmarkDialog?.close();
    bookmarkDeletePending = false;
    bookmarkFormError = "";
  }

  function selectBookmarkScope(scope: BookmarkLibraryScope) {
    bookmarkScope = scope;
    const categories = scope === "global" ? library.global : library.personal;
    bookmarkCategoryId = categories[0]?.id ?? "";
  }

  function selectBookmarkIconKind(kind: BookmarkLibraryIconKind) {
    if (bookmarkIconKind === kind) return;
    bookmarkIconKind = kind;
    bookmarkIconValue = kind === "lucide" ? "bookmark" : "";
  }

  async function saveBookmark(event: SubmitEvent) {
    event.preventDefault();
    if (bookmarkSaving || bookmarkDeleting) return;
    if (!bookmarkCategoryId) {
      bookmarkFormError = "Choose a category or create one first.";
      return;
    }
    if (bookmarkIconKind !== "favicon" && !bookmarkIconValue.trim()) {
      bookmarkFormError =
        bookmarkIconKind === "lucide"
          ? "Choose a Lucide icon."
          : "Enter a custom HTTPS icon URL.";
      return;
    }
    bookmarkSaving = true;
    bookmarkFormError = "";
    try {
      const input = {
        category_id: bookmarkCategoryId,
        title: bookmarkTitle,
        url: bookmarkUrl,
        icon_kind: bookmarkIconKind,
        icon_value:
          bookmarkIconKind === "favicon" ? null : bookmarkIconValue.trim(),
      };
      if (editingBookmark) {
        await updateBookmarkLibraryItem(
          bookmarkScope,
          editingBookmark.id,
          input,
        );
        onToast(`Updated ${bookmarkTitle.trim()}`);
      } else {
        await createBookmarkLibraryItem(bookmarkScope, input);
        onToast(`Added ${bookmarkTitle.trim()}`);
      }
      bookmarkDialog?.close();
      await loadLibrary(false);
    } catch (reason: unknown) {
      bookmarkFormError =
        reason instanceof Error ? reason.message : "Unable to save bookmark";
    } finally {
      bookmarkSaving = false;
    }
  }

  async function removeBookmark() {
    if (!editingBookmark || bookmarkDeleting || bookmarkSaving) return;
    if (!bookmarkDeletePending) {
      bookmarkDeletePending = true;
      return;
    }
    bookmarkDeleting = true;
    bookmarkFormError = "";
    try {
      await deleteBookmarkLibraryItem(bookmarkScope, editingBookmark.id);
      onToast(`Removed ${editingBookmark.title}`);
      bookmarkDialog?.close();
      await loadLibrary(false);
    } catch (reason: unknown) {
      bookmarkFormError =
        reason instanceof Error ? reason.message : "Unable to delete bookmark";
    } finally {
      bookmarkDeleting = false;
    }
  }

  function openCategoryEditor(category: BookmarkLibraryCategory | null = null) {
    closeMenus();
    editingCategory = category;
    categoryDeletePending = false;
    categoryFormError = "";
    categoryScope = category?.scope ?? "personal";
    categoryName = category?.name ?? "";
    categoryDialog?.showModal();
  }

  function closeCategoryEditor() {
    if (categorySaving || categoryDeleting) return;
    categoryDialog?.close();
    categoryDeletePending = false;
    categoryFormError = "";
  }

  async function saveCategory(event: SubmitEvent) {
    event.preventDefault();
    if (categorySaving || categoryDeleting) return;
    categorySaving = true;
    categoryFormError = "";
    try {
      if (editingCategory) {
        await updateBookmarkLibraryCategory(
          editingCategory.scope,
          editingCategory.id,
          categoryName,
        );
        onToast(`Renamed category to ${categoryName.trim()}`);
      } else {
        await createBookmarkLibraryCategory(categoryScope, categoryName);
        onToast(`Created ${categoryName.trim()}`);
      }
      categoryDialog?.close();
      await loadLibrary(false);
    } catch (reason: unknown) {
      categoryFormError =
        reason instanceof Error ? reason.message : "Unable to save category";
    } finally {
      categorySaving = false;
    }
  }

  async function removeCategory() {
    if (!editingCategory || categoryDeleting || categorySaving) return;
    if (!categoryDeletePending) {
      categoryDeletePending = true;
      return;
    }
    categoryDeleting = true;
    categoryFormError = "";
    try {
      await deleteBookmarkLibraryCategory(
        editingCategory.scope,
        editingCategory.id,
      );
      onToast(`Removed ${editingCategory.name}`);
      categoryDialog?.close();
      await loadLibrary(false);
    } catch (reason: unknown) {
      categoryFormError =
        reason instanceof Error ? reason.message : "Unable to delete category";
    } finally {
      categoryDeleting = false;
    }
  }

  function handleDialogBackdrop(
    event: MouseEvent,
    close: () => void,
  ) {
    if (event.target === event.currentTarget) close();
  }
</script>

<svelte:window onclick={closeMenus} onkeydown={handleWindowKeydown} />

<section
  class="bookmarks-page feature-page product-page"
  data-od-id="bookmarks-page"
>
  <div class="bookmarks-page-header page-header">
    <div class="bookmarks-heading">
      <TypedHeading text="$ bookmarks --all" odId="bookmarks-heading" />
      <p>Shared and personal shortcuts.</p>
    </div>
    <div class="bookmark-create-menu-anchor">
      <button
        class="ui-button ui-button--primary ui-button--icon bookmark-add"
        type="button"
        aria-label="Add bookmark or category"
        aria-expanded={createMenuOpen}
        onclick={(event) => {
          event.stopPropagation();
          createMenuOpen = !createMenuOpen;
        }}
        data-od-id="add-bookmark-menu"
      >
        <Plus size={19} strokeWidth={1.8} aria-hidden="true" />
      </button>
      {#if createMenuOpen}
        <div
          class="bookmark-popover-menu bookmark-create-action-menu"
          data-od-id="bookmark-create-menu"
        >
          <button
            type="button"
            onclick={() => openBookmarkEditor()}
            data-od-id="add-bookmark-action"
          >
            <Link2 size={16} strokeWidth={1.7} aria-hidden="true" />
            Add bookmark
          </button>
          <button
            type="button"
            onclick={() => openCategoryEditor()}
            data-od-id="add-category-action"
          >
            <FolderPlus size={16} strokeWidth={1.7} aria-hidden="true" />
            Add category
          </button>
        </div>
      {/if}
    </div>
  </div>

  <label class="bookmark-search">
    <span class="sr-only">Search bookmarks</span>
    <span class="bookmark-search-icon">
      <Search size={17} strokeWidth={1.7} aria-hidden="true" />
    </span>
    <input
      type="search"
      bind:value={searchQuery}
      placeholder="Search bookmarks"
      autocomplete="off"
      data-od-id="search-bookmarks"
    />
  </label>

  {#if !loaded && loading}
    <div class="bookmark-state" role="status">Loading bookmarks…</div>
  {:else if !loaded && loadError}
    <div class="bookmark-state is-error" role="alert">
      <p>{loadError}</p>
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={() => loadLibrary(true)}
      >
        Try again
      </button>
    </div>
  {:else if visibleCategories.length > 0}
    <div class="bookmark-categories" data-od-id="bookmark-categories">
      {#each visibleCategories as category (category.id)}
        <section
          class="bookmark-category"
          data-od-id={`bookmark-category-${category.id}`}
        >
          <header class="bookmark-category-header">
            <h2>{category.name}</h2>
            {#if canEditCategory(category)}
              <div class="bookmark-category-menu-anchor">
                <button
                  class="bookmark-category-menu-trigger"
                  type="button"
                  aria-label={`Manage ${category.name}`}
                  aria-expanded={categoryMenuId === category.id}
                  data-od-id={`manage-bookmark-category-${category.id}`}
                  onclick={(event) => {
                    event.stopPropagation();
                    categoryMenuId =
                      categoryMenuId === category.id ? "" : category.id;
                  }}
                >
                  <Ellipsis size={18} strokeWidth={1.7} aria-hidden="true" />
                </button>
                {#if categoryMenuId === category.id}
                  <div
                    class="bookmark-popover-menu bookmark-category-action-menu"
                  >
                    <button
                      type="button"
                      onclick={() => openCategoryEditor(category)}
                      data-od-id={`edit-bookmark-category-${category.id}`}
                    >
                      <Pencil size={15} strokeWidth={1.7} aria-hidden="true" />
                      Edit category
                    </button>
                  </div>
                {/if}
              </div>
            {/if}
          </header>

          {#if category.bookmarks.length > 0}
            <div class="bookmark-grid">
              {#each category.bookmarks as bookmark (bookmark.id)}
                <article
                  class="bookmark-tile"
                  data-od-id={`bookmark-tile-${bookmark.id}`}
                >
                  <!-- eslint-disable svelte/no-navigation-without-resolve -- user-configured external destination -->
                  <a
                    class="bookmark-launch"
                    href={bookmark.url}
                    target="_blank"
                    rel="noreferrer"
                    aria-label={`Open ${bookmark.title}`}
                    data-od-id={`open-bookmark-${bookmark.id}`}
                  >
                    <span class="bookmark-launch-square">
                      {#key `${bookmark.id}:${bookmark.updated_at}`}
                        <BookmarkLibraryIcon {bookmark} />
                      {/key}
                      <span class="bookmark-url-tooltip">{bookmark.url}</span>
                    </span>
                    <strong>{bookmark.title}</strong>
                  </a>
                  <!-- eslint-enable svelte/no-navigation-without-resolve -->

                  {#if canEditCategory(category)}
                    <div class="bookmark-menu-anchor">
                      <button
                        class="bookmark-menu-trigger"
                        type="button"
                        aria-label={`Manage ${bookmark.title}`}
                        aria-expanded={bookmarkMenuId === bookmark.id}
                        data-od-id={`manage-bookmark-${bookmark.id}`}
                        onclick={(event) => {
                          event.stopPropagation();
                          bookmarkMenuId =
                            bookmarkMenuId === bookmark.id ? "" : bookmark.id;
                        }}
                      >
                        <Ellipsis
                          size={17}
                          strokeWidth={1.8}
                          aria-hidden="true"
                        />
                      </button>
                      {#if bookmarkMenuId === bookmark.id}
                        <div
                          class="bookmark-popover-menu bookmark-action-menu"
                        >
                          <button
                            type="button"
                            onclick={() =>
                              openBookmarkEditor(bookmark, category)}
                            data-od-id={`edit-bookmark-${bookmark.id}`}
                          >
                            <Pencil
                              size={15}
                              strokeWidth={1.7}
                              aria-hidden="true"
                            />
                            Edit
                          </button>
                        </div>
                      {/if}
                    </div>
                  {/if}
                </article>
              {/each}
            </div>
          {:else if !searchQuery}
            <p class="bookmark-category-empty">No bookmarks yet.</p>
          {/if}
        </section>
      {/each}
    </div>
  {:else}
    <div class="bookmark-state">
      <p>{searchQuery ? "No bookmarks match this search." : "No bookmark categories yet."}</p>
    </div>
  {/if}
</section>

<dialog
  class="bookmark-dialog"
  bind:this={bookmarkDialog}
  onclick={(event) => handleDialogBackdrop(event, closeBookmarkEditor)}
  data-od-id="bookmark-editor-dialog"
>
  <form onsubmit={saveBookmark}>
    <div class="bookmark-dialog-heading">
      <div>
        <span class="bookmark-dialog-eyebrow"
          >{editingBookmark ? "Edit link" : "New link"}</span
        >
        <h2>{editingBookmark ? "Edit bookmark" : "Add bookmark"}</h2>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close bookmark editor"
        onclick={closeBookmarkEditor}
      >
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>

    <div class="bookmark-dialog-fields">
      {#if administrator && !editingBookmark}
        <fieldset class="bookmark-segmented-field">
          <legend>Visibility</legend>
          <div>
            <button
              type="button"
              aria-pressed={bookmarkScope === "personal"}
              data-od-id="bookmark-scope-personal"
              onclick={() => selectBookmarkScope("personal")}>Personal</button
            >
            <button
              type="button"
              aria-pressed={bookmarkScope === "global"}
              data-od-id="bookmark-scope-global"
              onclick={() => selectBookmarkScope("global")}>Global</button
            >
          </div>
        </fieldset>
      {/if}

      <label class="bookmark-form-field">
        <span>Category</span>
        <select
          bind:value={bookmarkCategoryId}
          required
          data-od-id="bookmark-category-select"
        >
          {#each bookmarkCategoryOptions as category (category.id)}
            <option value={category.id}>{category.name}</option>
          {/each}
        </select>
      </label>

      <label class="bookmark-form-field">
        <span>Name</span>
        <input
          type="text"
          bind:value={bookmarkTitle}
          maxlength="120"
          autocomplete="off"
          required
          data-od-id="bookmark-name-input"
        />
      </label>

      <label class="bookmark-form-field">
        <span>URL</span>
        <input
          type="url"
          bind:value={bookmarkUrl}
          maxlength="2048"
          inputmode="url"
          placeholder="https://example.com"
          required
          data-od-id="bookmark-url-input"
        />
      </label>

      <fieldset class="bookmark-segmented-field">
        <legend>Icon</legend>
        <div>
          <button
            type="button"
            aria-pressed={bookmarkIconKind === "favicon"}
            data-od-id="bookmark-icon-favicon"
            onclick={() => selectBookmarkIconKind("favicon")}>Favicon</button
          >
          <button
            type="button"
            aria-pressed={bookmarkIconKind === "lucide"}
            data-od-id="bookmark-icon-lucide"
            onclick={() => selectBookmarkIconKind("lucide")}>Lucide</button
          >
          <button
            type="button"
            aria-pressed={bookmarkIconKind === "custom"}
            data-od-id="bookmark-icon-custom"
            onclick={() => selectBookmarkIconKind("custom")}>Custom URL</button
          >
        </div>
      </fieldset>

      {#if bookmarkIconKind === "lucide"}
        <label class="bookmark-form-field">
          <span>Lucide icon name</span>
          <select
            bind:value={bookmarkIconValue}
            required
            data-od-id="bookmark-lucide-select"
          >
            <option value="" disabled>Choose an icon</option>
            {#each lucideIconNames as iconName (iconName)}
              <option value={iconName}>{iconName}</option>
            {/each}
          </select>
        </label>
      {:else if bookmarkIconKind === "custom"}
        <label class="bookmark-form-field">
          <span>Custom icon URL</span>
          <input
            type="url"
            bind:value={bookmarkIconValue}
            maxlength="2048"
            inputmode="url"
            placeholder="https://example.com/icon.svg"
            required
            data-od-id="bookmark-custom-icon-url"
          />
          <small>HTTPS SVG, PNG, JPEG, WebP, AVIF, or ICO.</small>
        </label>
      {/if}

      {#if bookmarkFormError}
        <p class="bookmark-form-error" role="alert">{bookmarkFormError}</p>
      {/if}
    </div>

    <div class="bookmark-dialog-actions">
      {#if editingBookmark}
        <button
          class="ui-button ui-button--danger bookmark-delete-action"
          type="button"
          disabled={bookmarkSaving || bookmarkDeleting}
          onclick={removeBookmark}
          data-od-id="delete-bookmark"
        >
          {bookmarkDeleting
            ? "Deleting…"
            : bookmarkDeletePending
              ? "Confirm delete"
              : "Delete"}
        </button>
      {/if}
      <button
        class="ui-button ui-button--secondary"
        type="button"
        disabled={bookmarkSaving || bookmarkDeleting}
        onclick={closeBookmarkEditor}
        data-od-id="cancel-bookmark-editor">Cancel</button
      >
      <button
        class="ui-button ui-button--primary"
        type="submit"
        disabled={bookmarkSaving || bookmarkDeleting}
        data-od-id="save-bookmark"
      >
        {bookmarkSaving ? "Saving…" : editingBookmark ? "Save changes" : "Add"}
      </button>
    </div>
  </form>
</dialog>

<dialog
  class="bookmark-dialog bookmark-category-dialog"
  bind:this={categoryDialog}
  onclick={(event) => handleDialogBackdrop(event, closeCategoryEditor)}
  data-od-id="bookmark-category-dialog"
>
  <form onsubmit={saveCategory}>
    <div class="bookmark-dialog-heading">
      <div>
        <span class="bookmark-dialog-eyebrow"
          >{editingCategory ? "Edit group" : "New group"}</span
        >
        <h2>{editingCategory ? "Edit category" : "Add category"}</h2>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon"
        type="button"
        aria-label="Close category editor"
        onclick={closeCategoryEditor}
      >
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>

    <div class="bookmark-dialog-fields">
      {#if administrator && !editingCategory}
        <fieldset class="bookmark-segmented-field">
          <legend>Visibility</legend>
          <div>
            <button
              type="button"
              aria-pressed={categoryScope === "personal"}
              data-od-id="category-scope-personal"
              onclick={() => (categoryScope = "personal")}>Personal</button
            >
            <button
              type="button"
              aria-pressed={categoryScope === "global"}
              data-od-id="category-scope-global"
              onclick={() => (categoryScope = "global")}>Global</button
            >
          </div>
        </fieldset>
      {/if}

      <label class="bookmark-form-field">
        <span>Name</span>
        <input
          type="text"
          bind:value={categoryName}
          maxlength="80"
          autocomplete="off"
          required
          data-od-id="category-name-input"
        />
      </label>

      {#if categoryFormError}
        <p class="bookmark-form-error" role="alert">{categoryFormError}</p>
      {/if}
    </div>

    <div class="bookmark-dialog-actions">
      {#if editingCategory}
        <button
          class="ui-button ui-button--danger bookmark-delete-action"
          type="button"
          disabled={categorySaving || categoryDeleting}
          onclick={removeCategory}
          data-od-id="delete-bookmark-category"
        >
          {categoryDeleting
            ? "Deleting…"
            : categoryDeletePending
              ? "Delete category and links"
              : "Delete"}
        </button>
      {/if}
      <button
        class="ui-button ui-button--secondary"
        type="button"
        disabled={categorySaving || categoryDeleting}
        onclick={closeCategoryEditor}
        data-od-id="cancel-category-editor">Cancel</button
      >
      <button
        class="ui-button ui-button--primary"
        type="submit"
        disabled={categorySaving || categoryDeleting}
        data-od-id="save-bookmark-category"
      >
        {categorySaving ? "Saving…" : editingCategory ? "Save changes" : "Add"}
      </button>
    </div>
  </form>
</dialog>

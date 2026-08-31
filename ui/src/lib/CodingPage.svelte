<script lang="ts">
  import ArrowUpRight from "lucide-svelte/icons/arrow-up-right";
  import Ellipsis from "lucide-svelte/icons/ellipsis";
  import GitBranch from "lucide-svelte/icons/git-branch";
  import GitMerge from "lucide-svelte/icons/git-merge";
  import KeyRound from "lucide-svelte/icons/key-round";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import { motionPopover, motionSurfaceEnter } from "$lib/motion.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    createCodingCategory,
    createCodingProject,
    deleteCodingCategory,
    deleteCodingProject,
    fetchCoding,
    fetchCodingCategories,
    updateCodingProjectCategories,
    updateCodingCredential,
    type CodingCategory,
    type CodingCategoryState,
    type CodingCredential,
    type CodingOwnedRepository,
    type CodingProject,
    type CodingProvider,
    type CodingResponse,
  } from "$lib/api";

  const emptyCoding: CodingResponse = {
    projects: [],
    releases: [],
    merge_requests: [],
    owned_repositories: [],
    pipelines: [],
    credentials: [],
    secret_storage_enabled: false,
    provider_errors: [],
    cached_at: null,
  };
  const emptyCategoryState: CodingCategoryState = {
    categories: [],
    assignments: [],
  };
  const providerHosts: Record<CodingProvider, string> = {
    github: "github.com",
    gitlab: "gitlab.com",
    codeberg: "codeberg.org",
    gitea: "",
    forgejo: "",
  };
  const includeArchivedStorageKey = "pandan:coding:include-archived-repositories";

  let coding = $state.raw<CodingResponse>(emptyCoding);
  let categoryState = $state.raw<CodingCategoryState>(emptyCategoryState);
  let hasLoaded = false;
  let loading = $state(true);
  let refreshing = $state(false);
  let initialLoadFailed = $state(false);
  let pageError = $state("");
  let query = $state("");
  let categoryFilter = $state("all");
  let repository = $state("");
  let repositoryError = $state("");
  let savingRepository = $state(false);
  let deleteId = $state("");
  let projectMenuId = $state("");
  let projectDialog = $state<HTMLDialogElement>();
  let credentialDialog = $state<HTMLDialogElement>();
  let projectEditDialog = $state<HTMLDialogElement>();
  let repositoryInput = $state<HTMLInputElement>();
  let credentialProvider = $state<CodingProvider>("gitlab");
  let credentialHost = $state("gitlab.com");
  let credentialToken = $state("");
  let credentialError = $state("");
  let savingCredential = $state(false);
  let includeArchivedRepositories = $state(false);
  let editingProject = $state.raw<CodingProject | null>(null);
  let draftCategoryIds = $state.raw<string[]>([]);
  let newCategoryName = $state("");
  let categoryError = $state("");
  let creatingCategory = $state(false);
  let savingProjectEdits = $state(false);
  let deleteCategoryId = $state("");

  let categoryIdsByProject = $derived.by(() => {
    const categoryIds: Record<string, string[]> = {};
    for (const assignment of categoryState.assignments) {
      const projectIds = categoryIds[assignment.project_id] ?? [];
      projectIds.push(assignment.category_id);
      categoryIds[assignment.project_id] = projectIds;
    }
    return categoryIds;
  });

  let filteredProjects = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    const categoryProjects = coding.projects.filter((project) => {
      const categoryIds = categoryIdsByProject[project.id] ?? [];
      if (categoryFilter === "all") return true;
      if (categoryFilter === "uncategorized") return categoryIds.length === 0;
      return categoryIds.includes(categoryFilter);
    });
    const matchingProjects = needle
      ? categoryProjects.filter((project) =>
          [project.provider, project.host, project.repository].some((value) =>
            value.toLowerCase().includes(needle),
          ),
        )
      : categoryProjects;
    return [...matchingProjects].sort((left, right) => {
      const leftRelease = releaseTimestamp(left.id);
      const rightRelease = releaseTimestamp(right.id);
      if (leftRelease === rightRelease) {
        return left.repository.localeCompare(right.repository);
      }
      return rightRelease - leftRelease;
    });
  });
  let profileActivityConnected = $derived(
    coding.credentials.some((credential) => credential.connected),
  );
  let visibleOwnedRepositories = $derived(
    includeArchivedRepositories
      ? coding.owned_repositories
      : coding.owned_repositories.filter((repository) => !repository.archived),
  );
  let openPullRequestCount = $derived(
    visibleOwnedRepositories.reduce(
      (total, repository) => total + (repository.open_pull_requests ?? 0),
      0,
    ),
  );
  let pendingRepositories = $derived(
    visibleOwnedRepositories.filter(
      (repository) => (repository.open_pull_requests ?? 0) > 0,
    ),
  );
  let archivedPendingRepositoryCount = $derived(
    coding.owned_repositories.filter(
      (repository) => repository.archived && (repository.open_pull_requests ?? 0) > 0,
    ).length,
  );
  let filteringProjects = $derived(
    query.trim().length > 0 || categoryFilter !== "all",
  );

  onMount(() => {
    includeArchivedRepositories =
      localStorage.getItem(includeArchivedStorageKey) === "true";
    void loadCoding();
  });

  function toggleArchivedRepositories() {
    includeArchivedRepositories = !includeArchivedRepositories;
    localStorage.setItem(
      includeArchivedStorageKey,
      String(includeArchivedRepositories),
    );
  }

  function captureProjectDialog(node: HTMLDialogElement) {
    projectDialog = node;
    return () => {
      projectDialog = undefined;
    };
  }

  function captureCredentialDialog(node: HTMLDialogElement) {
    credentialDialog = node;
    return () => {
      credentialDialog = undefined;
    };
  }

  function captureProjectEditDialog(node: HTMLDialogElement) {
    projectEditDialog = node;
    return () => {
      projectEditDialog = undefined;
    };
  }

  function captureRepositoryInput(node: HTMLInputElement) {
    repositoryInput = node;
    return () => {
      repositoryInput = undefined;
    };
  }

  async function loadCoding(refresh = false) {
    const initialLoad = !hasLoaded;
    if (initialLoad) loading = true;
    else refreshing = true;
    initialLoadFailed = false;
    pageError = "";
    try {
      const [nextCoding, nextCategories] = await Promise.all([
        fetchCoding(refresh),
        fetchCodingCategories(),
      ]);
      coding = nextCoding;
      categoryState = nextCategories;
      hasLoaded = true;
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to load Coding";
      initialLoadFailed = initialLoad;
    } finally {
      loading = false;
      refreshing = false;
    }
  }

  async function openProjectDialog() {
    repository = "";
    repositoryError = "";
    projectDialog?.showModal();
    await tick();
    repositoryInput?.focus();
  }

  async function addProject(event: SubmitEvent) {
    event.preventDefault();
    if (savingRepository) return;
    savingRepository = true;
    repositoryError = "";
    try {
      await createCodingProject(repository.trim());
      projectDialog?.close();
      await loadCoding();
    } catch (reason: unknown) {
      repositoryError = reason instanceof Error ? reason.message : "Unable to add project";
    } finally {
      savingRepository = false;
    }
  }

  async function removeProject(project: CodingProject) {
    if (deleteId !== project.id) {
      deleteId = project.id;
      return;
    }
    pageError = "";
    try {
      await deleteCodingProject(project.id);
      coding = {
        ...coding,
        projects: coding.projects.filter((item) => item.id !== project.id),
        releases: coding.releases.filter((item) => item.project_id !== project.id),
        pipelines: coding.pipelines.filter((item) => item.project_id !== project.id),
      };
      categoryState = {
        ...categoryState,
        assignments: categoryState.assignments.filter(
          (assignment) => assignment.project_id !== project.id,
        ),
      };
      deleteId = "";
      projectMenuId = "";
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to remove project";
    }
  }

  function openCredential(
    provider: CodingProvider = "gitlab",
    host = providerHosts[provider],
  ) {
    credentialProvider = provider;
    credentialHost = host;
    credentialToken = "";
    credentialError = "";
    credentialDialog?.showModal();
  }

  function openProjectEditor(project: CodingProject) {
    projectMenuId = "";
    deleteId = "";
    editingProject = project;
    draftCategoryIds = [...(categoryIdsByProject[project.id] ?? [])];
    newCategoryName = "";
    categoryError = "";
    deleteCategoryId = "";
    projectEditDialog?.showModal();
  }

  function toggleProjectMenu(projectId: string) {
    projectMenuId = projectMenuId === projectId ? "" : projectId;
    deleteId = "";
  }

  function closeProjectMenuOnFocusOut(event: FocusEvent, projectId: string) {
    const anchor = event.currentTarget;
    const nextTarget = event.relatedTarget;
    if (
      anchor instanceof HTMLElement &&
      nextTarget instanceof Node &&
      anchor.contains(nextTarget)
    ) {
      return;
    }
    if (projectMenuId === projectId) {
      projectMenuId = "";
      deleteId = "";
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || !projectMenuId) return;
    const activeMenuId = projectMenuId;
    projectMenuId = "";
    deleteId = "";
    void tick().then(() => {
      document.getElementById(`coding-project-menu-trigger-${activeMenuId}`)?.focus();
    });
  }

  function handleWindowPointerdown(event: PointerEvent) {
    const target = event.target;
    if (!projectMenuId) return;
    if (
      target instanceof Element &&
      target.closest(`[data-coding-project-menu-root="${projectMenuId}"]`)
    ) {
      return;
    }
    projectMenuId = "";
    deleteId = "";
  }

  function toggleDraftCategory(categoryId: string) {
    draftCategoryIds = draftCategoryIds.includes(categoryId)
      ? draftCategoryIds.filter((id) => id !== categoryId)
      : [...draftCategoryIds, categoryId];
  }

  async function addCategory() {
    const name = newCategoryName.trim();
    if (!name || creatingCategory) return;
    creatingCategory = true;
    categoryError = "";
    try {
      const category = await createCodingCategory(name);
      categoryState = {
        ...categoryState,
        categories: [...categoryState.categories, category].sort((left, right) =>
          left.name.localeCompare(right.name),
        ),
      };
      draftCategoryIds = [...draftCategoryIds, category.id];
      newCategoryName = "";
    } catch (reason: unknown) {
      categoryError = reason instanceof Error ? reason.message : "Unable to create category";
    } finally {
      creatingCategory = false;
    }
  }

  async function removeCategory(category: CodingCategory) {
    if (deleteCategoryId !== category.id) {
      deleteCategoryId = category.id;
      return;
    }
    categoryError = "";
    try {
      await deleteCodingCategory(category.id);
      categoryState = {
        categories: categoryState.categories.filter((item) => item.id !== category.id),
        assignments: categoryState.assignments.filter(
          (assignment) => assignment.category_id !== category.id,
        ),
      };
      draftCategoryIds = draftCategoryIds.filter((id) => id !== category.id);
      if (categoryFilter === category.id) categoryFilter = "all";
      deleteCategoryId = "";
    } catch (reason: unknown) {
      categoryError = reason instanceof Error ? reason.message : "Unable to delete category";
    }
  }

  async function saveProjectEdits(event: SubmitEvent) {
    event.preventDefault();
    if (!editingProject || savingProjectEdits) return;
    savingProjectEdits = true;
    categoryError = "";
    try {
      const update = await updateCodingProjectCategories(
        editingProject.id,
        draftCategoryIds,
      );
      categoryState = {
        ...categoryState,
        assignments: [
          ...categoryState.assignments.filter(
            (assignment) => assignment.project_id !== update.project_id,
          ),
          ...update.category_ids.map((categoryId) => ({
            project_id: update.project_id,
            category_id: categoryId,
          })),
        ],
      };
      projectEditDialog?.close();
    } catch (reason: unknown) {
      categoryError = reason instanceof Error ? reason.message : "Unable to assign categories";
    } finally {
      savingProjectEdits = false;
    }
  }

  function changeCredentialProvider() {
    credentialHost = providerHosts[credentialProvider];
  }

  async function saveCredential(event: SubmitEvent) {
    event.preventDefault();
    if (savingCredential) return;
    savingCredential = true;
    credentialError = "";
    try {
      await updateCodingCredential({
        provider: credentialProvider,
        host: credentialHost.trim(),
        token: credentialToken,
      });
      credentialDialog?.close();
      await loadCoding();
    } catch (reason: unknown) {
      credentialError = reason instanceof Error ? reason.message : "Unable to save provider token";
    } finally {
      savingCredential = false;
    }
  }

  async function removeCredential(credential: CodingCredential) {
    pageError = "";
    try {
      await updateCodingCredential({
        provider: credential.provider,
        host: credential.host,
        clear: true,
      });
      await loadCoding();
    } catch (reason: unknown) {
      pageError = reason instanceof Error ? reason.message : "Unable to remove provider token";
    }
  }

  function releaseFor(projectId: string) {
    return coding.releases.find((release) => release.project_id === projectId);
  }

  function releaseTimestamp(projectId: string) {
    const publishedAt = releaseFor(projectId)?.published_at;
    if (!publishedAt) return Number.NEGATIVE_INFINITY;
    const timestamp = Date.parse(publishedAt);
    return Number.isNaN(timestamp) ? Number.NEGATIVE_INFINITY : timestamp;
  }

  function projectUrl(project: CodingProject) {
    return `https://${project.host}/${project.repository}`;
  }

  function projectName(project: CodingProject) {
    return project.repository.split("/").at(-1) || project.repository;
  }

  function categoriesForProject(projectId: string) {
    const categoryIds = new Set(categoryIdsByProject[projectId] ?? []);
    return categoryState.categories.filter((category) => categoryIds.has(category.id));
  }

  function pullRequestLabel(repository: CodingOwnedRepository) {
    if (repository.open_pull_requests === null) return "Count unavailable";
    return repository.open_pull_requests === 1 ? "open request" : "open requests";
  }

  function formatDate(value: string) {
    if (!value) return "Date unavailable";
    const date = new Date(value);
    return Number.isNaN(date.valueOf())
      ? "Date unavailable"
      : new Intl.DateTimeFormat("en", {
          month: "short",
          day: "numeric",
          year: "numeric",
        }).format(date);
  }

  function formatCacheTime(value: string | null) {
    if (!value) return "Background cache warming";
    const date = new Date(value);
    return Number.isNaN(date.valueOf())
      ? "Cached snapshot"
      : `Cached ${new Intl.DateTimeFormat("en", {
          month: "short",
          day: "numeric",
          hour: "numeric",
          minute: "2-digit",
        }).format(date)}`;
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} onpointerdown={handleWindowPointerdown} />

<section class="coding-page product-page" data-od-id="coding-page">
  <header class="coding-header page-header" data-od-id="coding-header">
    <div>
      <TypedHeading text="$ coding --watch" odId="coding-heading" />
      <p>Monitor releases, pending review work, and the newest pipelines for subscribed projects.</p>
    </div>
    <div class="header-actions">
      <button type="button" onclick={() => loadCoding(true)} disabled={loading || refreshing} data-od-id="refresh-coding">
        <RefreshCw class={refreshing ? "spinning" : undefined} size={15} strokeWidth={1.8} aria-hidden="true" />
        {loading ? "Loading…" : refreshing ? "Refreshing…" : initialLoadFailed ? "Retry" : "Refresh"}
      </button>
      <button class="ui-button ui-button--primary coding-primary" type="button" onclick={openProjectDialog} data-od-id="add-coding-project">
        <Plus size={16} strokeWidth={1.8} aria-hidden="true" /> Add project
      </button>
    </div>
  </header>

  {#if pageError}<p class="coding-error" role="alert">{pageError}</p>{/if}
  {#if !loading && !initialLoadFailed && (coding.projects.length || coding.credentials.length)}
    <p class="cache-state" data-od-id="coding-cache-state">
      {formatCacheTime(coding.cached_at)} · Hourly background refresh
      {#if !coding.cached_at}<span> · Use Refresh to fetch providers now.</span>{/if}
    </p>
  {/if}
  {#if loading}<p class="sr-only" role="status">Loading coding activity.</p>{/if}
  {#if coding.provider_errors.length}
    <details class="provider-errors">
      <summary>{coding.provider_errors.length} provider {coding.provider_errors.length === 1 ? "request" : "requests"} could not refresh</summary>
      <ul>{#each coding.provider_errors as error (error)}<li>{error}</li>{/each}</ul>
    </details>
  {/if}

  <div class="coding-stats" aria-label="Coding overview" aria-busy={loading}>
    <div><span>Projects</span><strong class:stat-placeholder={loading}>{#if !loading}<i class="stat-value" {@attach motionSurfaceEnter({ y: 4, duration: 0.18 })}>{initialLoadFailed ? "—" : coding.projects.length}</i>{/if}</strong></div>
    <div><span>Releases loaded</span><strong class:stat-placeholder={loading}>{#if !loading}<i class="stat-value" {@attach motionSurfaceEnter({ y: 4, duration: 0.18 })}>{initialLoadFailed ? "—" : coding.releases.length}</i>{/if}</strong></div>
    <div><span>Open pull requests</span><strong class:stat-placeholder={loading}>{#if !loading}<i class="stat-value" {@attach motionSurfaceEnter({ y: 4, duration: 0.18 })}>{initialLoadFailed ? "—" : openPullRequestCount}</i>{/if}</strong></div>
    <div><span>Recent pipelines</span><strong class:stat-placeholder={loading}>{#if !loading}<i class="stat-value" {@attach motionSurfaceEnter({ y: 4, duration: 0.18 })}>{initialLoadFailed ? "—" : coding.pipelines.length}</i>{/if}</strong></div>
  </div>

  <div class="coding-layout">
    <section class="projects-panel" aria-busy={loading} data-od-id="coding-projects">
      <header>
        <div><span>[ SUBSCRIPTIONS ]</span><h3>Projects</h3></div>
        <span class="project-panel-status">{coding.projects.length} subscribed</span>
      </header>

      <div class="project-filters" data-od-id="coding-project-filters">
        <label class="project-search">
          <Search size={16} strokeWidth={1.8} aria-hidden="true" />
          <span class="sr-only">Filter projects</span>
          <input type="search" bind:value={query} disabled={loading || initialLoadFailed} placeholder="Search names, hosts, or repositories…" data-od-id="coding-text-filter" />
        </label>
        <label class="category-filter">
          <span class="sr-only">Filter projects by category</span>
          <select bind:value={categoryFilter} disabled={loading || initialLoadFailed} data-od-id="coding-category-filter">
            <option value="all">All categories</option>
            <option value="uncategorized">Uncategorized</option>
            {#each categoryState.categories as category (category.id)}
              <option value={category.id}>{category.name}</option>
            {/each}
          </select>
        </label>
        <span class="project-filter-count" aria-live="polite">{filteredProjects.length} / {coding.projects.length} projects</span>
      </div>

      <div class="project-table" role="table" aria-label="Subscribed coding projects">
        <div class="project-table-header" role="row">
          <span role="columnheader">Name</span>
          <span role="columnheader">Repo location</span>
          <span role="columnheader">Release</span>
          <span role="columnheader">Release date</span>
          <span role="columnheader">Categories</span>
          <span class="sr-only" role="columnheader">Actions</span>
        </div>

        {#if loading}
          <div class="panel-loading project-loading" role="status" data-od-id="coding-projects-loading">
            <span class="panel-loading-label">Loading subscribed projects…</span>
            <div class="project-loading-rows" aria-hidden="true">
              {#each [0, 1, 2] as row (row)}
                <div class="project-loading-row">
                  {#each ["name", "location", "release", "date", "categories"] as cell (cell)}
                    <div class="loading-stack"><i class="loading-line loading-line--short"></i><i class="loading-line loading-line--medium"></i></div>
                  {/each}
                  <i class="loading-menu"></i>
                </div>
              {/each}
            </div>
          </div>
        {:else if initialLoadFailed}
          <div class="panel-unavailable" data-od-id="coding-projects-unavailable">
            <strong>Project activity unavailable</strong>
            <p>Use Retry in the page header to load subscriptions and release activity.</p>
          </div>
        {:else}
          <div class="project-list" {@attach motionSurfaceEnter({ y: 6, duration: 0.22 })}>
          {#each filteredProjects as project (project.id)}
            {@const release = releaseFor(project.id)}
            {@const projectCategories = categoriesForProject(project.id)}
            <div class={["project-row", projectMenuId === project.id && "has-open-menu"]} role="row" data-od-id={`coding-project-${project.id}`}>
              <div class="project-cell project-name-cell" role="cell">
                <span class="mobile-column-label">Name</span>
                <strong title={projectName(project)}>{projectName(project)}</strong>
                <small>{project.provider}</small>
              </div>
              <div class="project-cell repo-location-cell" role="cell">
                <span class="mobile-column-label">Repo location</span>
                <a href={projectUrl(project)} target="_blank" rel="noreferrer">
                  <span title={`${project.host}/${project.repository}`}>{project.host}/{project.repository}</span>
                  <ArrowUpRight size={14} strokeWidth={1.7} aria-hidden="true" />
                </a>
                <small>{project.has_credential ? "Authenticated" : "Public access"}</small>
              </div>
              <div class="project-cell release-cell" role="cell">
                <span class="mobile-column-label">Release</span>
                {#if release}
                  <a href={release.url} target="_blank" rel="noreferrer">{release.version}</a>
                {:else}
                  <span class="cell-unavailable">Unavailable</span>
                {/if}
              </div>
              <div class="project-cell release-date-cell" role="cell">
                <span class="mobile-column-label">Release date</span>
                {#if release}
                  <time datetime={release.published_at}>{formatDate(release.published_at)}</time>
                {:else}
                  <span class="cell-unavailable">—</span>
                {/if}
              </div>
              <div class="project-cell project-categories" role="cell">
                <span class="mobile-column-label">Categories</span>
                <div class="category-chip-list">
                  {#each projectCategories.slice(0, 2) as category (category.id)}
                    <span>{category.name}</span>
                  {:else}
                    <span class="category-empty">Uncategorized</span>
                  {/each}
                  {#if projectCategories.length > 2}<span class="category-overflow">+{projectCategories.length - 2}</span>{/if}
                </div>
              </div>
              <div
                class="project-row-menu"
                role="cell"
                data-coding-project-menu-root={project.id}
                onfocusout={(event) => closeProjectMenuOnFocusOut(event, project.id)}
              >
                <button
                  class="project-row-menu-trigger"
                  id={`coding-project-menu-trigger-${project.id}`}
                  type="button"
                  aria-label={`More actions for ${project.repository}`}
                  aria-haspopup="menu"
                  aria-expanded={projectMenuId === project.id}
                  aria-controls={`coding-project-menu-${project.id}`}
                  data-od-id={`coding-project-actions-${project.id}`}
                  onclick={() => toggleProjectMenu(project.id)}
                >
                  <Ellipsis size={18} strokeWidth={1.8} aria-hidden="true" />
                </button>
                <div
                  class="project-row-menu-popover"
                  id={`coding-project-menu-${project.id}`}
                  role="menu"
                  aria-label={`${project.repository} actions`}
                  aria-hidden={projectMenuId !== project.id}
                  inert={projectMenuId !== project.id}
                  data-od-id={`coding-project-menu-${project.id}`}
                  {@attach motionPopover(projectMenuId === project.id, { closedY: -6 })}
                >
                  <button type="button" role="menuitem" onclick={() => openProjectEditor(project)} data-od-id={`edit-coding-project-${project.id}`}>
                    <Pencil size={15} strokeWidth={1.8} aria-hidden="true" /> Edit project
                  </button>
                  <button
                    class={["project-delete-action", deleteId === project.id && "is-armed"]}
                    type="button"
                    role="menuitem"
                    aria-label={deleteId === project.id ? `Confirm removal of ${project.repository}` : `Delete ${project.repository}`}
                    onclick={() => removeProject(project)}
                    data-od-id={`delete-coding-project-${project.id}`}
                  >
                    <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
                    {deleteId === project.id ? "Confirm delete" : "Delete project"}
                  </button>
                </div>
              </div>
            </div>
          {:else}
            <div class="coding-empty" data-od-id={filteringProjects ? "coding-filter-empty-state" : "coding-projects-empty-state"}>
              <GitBranch size={28} strokeWidth={1.4} aria-hidden="true" />
              <h3>{filteringProjects ? "No matching projects" : "No projects subscribed"}</h3>
              <p>{filteringProjects ? "Try a broader search or category." : "Add a repository to begin watching its latest release."}</p>
              {#if !filteringProjects}<button type="button" onclick={openProjectDialog}>Add your first project</button>{/if}
            </div>
          {/each}
          </div>
        {/if}
      </div>
    </section>

    <aside class="coding-rail">
      <section class="merge-panel" aria-busy={loading} data-od-id="coding-merge-requests">
        <header><div><span>[ OWNED REPOSITORIES ]</span><h3>Pending pull requests</h3></div><GitMerge size={18} strokeWidth={1.6} aria-hidden="true" /></header>
        <div class="merge-settings" data-od-id="coding-archived-repository-setting">
          <button
            class="ui-toggle-button archived-repository-toggle"
            type="button"
            aria-pressed={includeArchivedRepositories}
            aria-label={`${includeArchivedRepositories ? "Exclude" : "Include"} archived repositories`}
            disabled={loading || initialLoadFailed}
            onclick={toggleArchivedRepositories}
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>
              <strong>Include archived</strong>
              <small>{archivedPendingRepositoryCount} with open requests</small>
            </span>
          </button>
        </div>
        {#if loading}
          <div class="panel-loading rail-loading" role="status" data-od-id="coding-merge-requests-loading">
            <span class="panel-loading-label">Loading pull requests…</span>
            <div class="loading-stack" aria-hidden="true"><i class="loading-line loading-line--short"></i><i class="loading-line loading-line--long"></i><i class="loading-line loading-line--medium"></i></div>
          </div>
        {:else if initialLoadFailed}
          <div class="panel-unavailable compact"><strong>Pull request data unavailable</strong></div>
        {:else if !profileActivityConnected}
          <div class="rail-empty" {@attach motionSurfaceEnter({ y: 6, duration: 0.22 })}>
            <p>Connect a provider token to discover repositories owned by that account and count their open pull requests.</p>
            <button type="button" onclick={() => openCredential()}>Connect provider</button>
          </div>
        {:else}
          <div class="merge-list" {@attach motionSurfaceEnter({ y: 6, duration: 0.22 })}>
            {#each pendingRepositories as repository (`${repository.provider}:${repository.host}:${repository.repository}`)}
              <a href={repository.url} target="_blank" rel="noreferrer" data-od-id={`owned-repository-${repository.provider}-${repository.repository.replaceAll("/", "-")}`}>
                <span>{repository.provider} · {repository.host}</span>
                <strong>{repository.repository}</strong>
                <small class:count-unavailable={repository.open_pull_requests === null}>
                  <b>{repository.open_pull_requests ?? "—"}</b> {pullRequestLabel(repository)}
                </small>
              </a>
            {:else}
              <div class="rail-empty">
                <p>{!includeArchivedRepositories && archivedPendingRepositoryCount > 0 ? "Open requests exist only in archived repositories." : "No owned repositories have open pull requests."}</p>
              </div>
            {/each}
          </div>
        {/if}
      </section>

      <section class="access-panel" aria-busy={loading} data-od-id="coding-provider-access">
        <header><div><span>[ CREDENTIALS ]</span><h3>Provider access</h3></div><KeyRound size={18} strokeWidth={1.6} aria-hidden="true" /></header>
        <p>Tokens stay encrypted on the server and are never returned to this page.</p>
        {#if loading}
          <div class="panel-loading rail-loading" role="status" data-od-id="coding-provider-access-loading">
            <span class="panel-loading-label">Checking provider access…</span>
            <div class="loading-stack" aria-hidden="true"><i class="loading-line loading-line--medium"></i><i class="loading-line loading-line--long"></i></div>
          </div>
        {:else if initialLoadFailed}
          <div class="panel-unavailable compact"><strong>Provider access unavailable</strong></div>
        {:else if !coding.secret_storage_enabled}
          <div class="credential-note" {@attach motionSurfaceEnter({ y: 6, duration: 0.22 })}>Configure server secret storage before saving provider tokens.</div>
        {:else}
          <div class="access-content" {@attach motionSurfaceEnter({ y: 6, duration: 0.22 })}>
            <div class="credential-list">
              {#each coding.credentials as credential (`${credential.provider}:${credential.host}`)}
                <div><span><strong>{credential.provider}</strong><small>{credential.host}</small></span><button class="ui-button ui-button--danger" type="button" onclick={() => removeCredential(credential)}>Disconnect</button></div>
              {:else}
                <span class="no-credentials">No provider tokens stored.</span>
              {/each}
            </div>
            <button class="ui-button ui-button--secondary access-action" type="button" onclick={() => openCredential()}><Plus size={14} strokeWidth={1.8} /> Add provider token</button>
          </div>
        {/if}
      </section>
    </aside>
  </div>

  <dialog class="coding-dialog" {@attach captureProjectDialog} onclick={(event) => event.target === projectDialog && projectDialog?.close()} data-od-id="coding-project-dialog">
    <header><div><span>[ PROJECT.ADD ]</span><h2>Subscribe to releases</h2></div><button class="ui-button ui-button--ghost ui-button--icon coding-dialog-close" type="button" aria-label="Close project dialog" onclick={() => projectDialog?.close()} data-od-id="close-coding-project-dialog"><X size={18} /></button></header>
    <form onsubmit={addProject}>
      <label for="coding-repository">Repository</label>
      <input id="coding-repository" bind:value={repository} {@attach captureRepositoryInput} maxlength="300" placeholder="owner/repository" required />
      <p class="field-help">Use <code>gitlab:owner/repository</code>, <code>codeberg:owner/repository</code>, or <code>forgejo@host:owner/repository</code>. GitHub is the default.</p>
      {#if repositoryError}<p class="form-error" role="alert">{repositoryError}</p>{/if}
      <footer><button class="ui-button ui-button--secondary" type="button" onclick={() => projectDialog?.close()}>Cancel</button><button class="ui-button ui-button--primary coding-primary" type="submit" disabled={savingRepository}>{savingRepository ? "Adding…" : "Add project"}</button></footer>
    </form>
  </dialog>

  <dialog class="coding-dialog coding-category-dialog" {@attach captureProjectEditDialog} onclick={(event) => event.target === projectEditDialog && projectEditDialog?.close()} aria-labelledby="coding-project-edit-dialog-title" data-od-id="coding-project-edit-dialog">
    <header>
      <div><span>[ PROJECT.EDIT ]</span><h2 id="coding-project-edit-dialog-title">{editingProject ? `Edit ${projectName(editingProject)}` : "Edit project"}</h2></div>
      <button class="ui-button ui-button--ghost ui-button--icon coding-dialog-close" type="button" aria-label="Close project editor" onclick={() => projectEditDialog?.close()} data-od-id="close-coding-project-edit-dialog"><X size={18} /></button>
    </header>
    <form onsubmit={saveProjectEdits}>
      <div class="coding-category-dialog-body">
        {#if editingProject}
          <div class="project-edit-summary" data-od-id="coding-project-edit-summary">
            <span><small>Provider</small><strong>{editingProject.provider}</strong></span>
            <span><small>Repository</small><strong>{editingProject.host}/{editingProject.repository}</strong></span>
          </div>
        {/if}
        <p class="field-help">Choose the categories assigned to this repository, then save your changes. Creating or deleting a category applies account-wide immediately.</p>
        <div class="category-create" data-od-id="coding-category-create">
          <label for="coding-category-name">New category</label>
          <div>
            <input
              id="coding-category-name"
              bind:value={newCategoryName}
              maxlength="48"
              placeholder="Backend"
              onkeydown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void addCategory();
                }
              }}
            />
            <button class="ui-button ui-button--secondary" type="button" disabled={!newCategoryName.trim() || creatingCategory} onclick={addCategory} data-od-id="create-coding-category">
              <Plus size={14} strokeWidth={1.8} aria-hidden="true" /> {creatingCategory ? "Adding…" : "Add"}
            </button>
          </div>
        </div>
        <div class="category-options" aria-label="Repository categories">
          {#each categoryState.categories as category (category.id)}
            <div class="category-option" data-od-id={`coding-category-${category.id}`}>
              <button
                class="ui-toggle-button category-toggle"
                type="button"
                aria-pressed={draftCategoryIds.includes(category.id)}
                onclick={() => toggleDraftCategory(category.id)}
              >
                <span class="ui-toggle-indicator" aria-hidden="true"></span>
                <span>{category.name}</span>
              </button>
              <button
                class:confirm={deleteCategoryId === category.id}
                class="ui-button ui-button--danger ui-button--icon delete-category"
                type="button"
                aria-label={deleteCategoryId === category.id ? `Confirm deletion of ${category.name}` : `Delete ${category.name}`}
                title={deleteCategoryId === category.id ? "Select again to confirm" : "Delete category"}
                onclick={() => removeCategory(category)}
              >
                <Trash2 size={15} strokeWidth={1.7} aria-hidden="true" />
              </button>
            </div>
          {:else}
            <p class="category-options-empty">No categories yet. Create one above to organize this repository.</p>
          {/each}
        </div>
        {#if categoryError}<p class="form-error" role="alert">{categoryError}</p>{/if}
      </div>
      <footer>
        <button class="ui-button ui-button--secondary" type="button" onclick={() => projectEditDialog?.close()}>Cancel</button>
        <button class="ui-button ui-button--primary coding-primary" type="submit" disabled={!editingProject || savingProjectEdits}>{savingProjectEdits ? "Saving…" : "Save changes"}</button>
      </footer>
    </form>
  </dialog>

  <dialog class="coding-dialog" {@attach captureCredentialDialog} onclick={(event) => event.target === credentialDialog && credentialDialog?.close()} data-od-id="coding-credential-dialog">
    <header><div><span>[ PROVIDER.ACCESS ]</span><h2>Store provider token</h2></div><button class="ui-button ui-button--ghost ui-button--icon coding-dialog-close" type="button" aria-label="Close provider dialog" onclick={() => credentialDialog?.close()} data-od-id="close-coding-credential-dialog"><X size={18} /></button></header>
    <form onsubmit={saveCredential}>
      <div class="form-grid">
        <div><label for="credential-provider">Provider</label><select id="credential-provider" bind:value={credentialProvider} onchange={changeCredentialProvider}><option value="gitlab">GitLab</option><option value="github">GitHub</option><option value="codeberg">Codeberg</option><option value="gitea">Gitea</option><option value="forgejo">Forgejo</option></select></div>
        <div><label for="credential-host">Host</label><input id="credential-host" bind:value={credentialHost} maxlength="253" placeholder="git.example.com" readonly={Boolean(providerHosts[credentialProvider])} required /></div>
      </div>
      <label for="credential-token">Personal access token</label>
      <input id="credential-token" type="password" bind:value={credentialToken} maxlength="4096" autocomplete="new-password" placeholder="Token is encrypted before storage" required />
      <p class="field-help">Provider tokens increase release API limits, unlock private repositories, and enable available profile or pipeline activity.</p>
      {#if credentialError}<p class="form-error" role="alert">{credentialError}</p>{/if}
      <footer><button class="ui-button ui-button--secondary" type="button" onclick={() => credentialDialog?.close()}>Cancel</button><button class="ui-button ui-button--primary coding-primary" type="submit" disabled={savingCredential}>{savingCredential ? "Saving…" : "Save token"}</button></footer>
    </form>
  </dialog>
</section>

<style>
  .coding-page { display: grid; gap: 18px; min-width: 0; padding: clamp(24px, 3vw, 42px); }
  .coding-header { display: flex; align-items: end; justify-content: space-between; gap: 24px; padding-bottom: 18px; border-bottom: 1px solid var(--border); }
  .projects-panel header span, .coding-rail header span, .coding-dialog header span { color: var(--muted); font-family: var(--font-mono); font-size: 10px; letter-spacing: .09em; text-transform: uppercase; }
  .coding-header p { max-width: 68ch; margin: 7px 0 0; color: var(--muted); font-family: var(--font-mono); font-size: 11px; line-height: 1.6; }
  button, input, select { min-height: 42px; border: 1px solid var(--border); background: transparent; color: inherit; font: inherit; }
  button { cursor: pointer; }
  button:disabled { cursor: not-allowed; opacity: .55; }
  .header-actions { display: flex; gap: 8px; }
  .header-actions button, .coding-primary { display: inline-flex; align-items: center; justify-content: center; gap: 8px; padding: 0 15px; font-family: var(--font-mono); font-size: 10px; letter-spacing: .04em; text-transform: uppercase; }
  .coding-primary { border-color: var(--fg); background: var(--fg); color: var(--bg); }
  .coding-primary:hover { background: transparent; color: var(--fg); }
  .header-actions > button:first-child:hover { border-color: var(--fg); }
  .cache-state { margin: -7px 0 0; color: var(--muted); font-family: var(--font-mono); font-size: 9px; letter-spacing: .02em; }
  .coding-error, .form-error { margin: 0; border: 1px solid oklch(60% .16 25 / .5); background: oklch(20% .04 25 / .75); padding: 10px 12px; color: oklch(82% .09 25); font-family: var(--font-mono); font-size: 11px; }
  .provider-errors { border: 1px solid var(--border); background: var(--page-surface, var(--surface)); padding: 10px 12px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
  .provider-errors summary { cursor: pointer; color: var(--fg); }
  .provider-errors ul { display: grid; gap: 6px; margin: 10px 0 0; padding-left: 20px; }
  .coding-stats { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); border: 1px solid var(--border); background: var(--page-surface, var(--surface)); }
  .coding-stats > div { display: flex; min-height: 70px; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 16px; border-right: 1px solid var(--border); }
  .coding-stats > div:last-child { border-right: 0; }
  .coding-stats span { color: var(--muted); font-family: var(--font-mono); font-size: 9px; letter-spacing: .07em; text-transform: uppercase; }
  .coding-stats strong { font-family: var(--font-mono); font-size: 23px; font-weight: 520; }
  .coding-stats strong.stat-placeholder { width: 30px; height: 9px; }
  .stat-value { display: block; font: inherit; }
  .coding-layout { display: grid; grid-template-columns: minmax(0, 1.55fr) minmax(320px, .8fr); gap: 18px; align-items: start; }
  .projects-panel, .coding-rail > section { min-width: 0; border: 1px solid var(--border); background: var(--page-surface, var(--surface)); }
  .projects-panel > header, .coding-rail > section > header { display: flex; min-height: 62px; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 14px; border-bottom: 1px solid var(--border); }
  .projects-panel h3, .coding-rail h3 { margin: 5px 0 0; font-family: var(--font-mono); font-size: 14px; font-weight: 550; }
  .project-panel-status { color: var(--muted); font-family: var(--font-mono); font-size: 9px; letter-spacing: .04em; text-transform: uppercase; }
  .project-filters { display: grid; grid-template-columns: minmax(220px, 1fr) minmax(220px, .7fr) auto; gap: 8px; margin: 8px; padding: 8px; border: 1px solid var(--border); border-radius: 9px; background: color-mix(in oklch, var(--page-surface, var(--surface)) 86%, transparent); }
  .projects-panel .project-filters label { min-width: 0; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); }
  .project-search { display: flex; align-items: center; gap: 9px; padding: 0 12px; color: var(--muted); }
  .category-filter { min-width: 220px; padding: 0; color: var(--fg); }
  .projects-panel .project-filters input, .category-filter select { width: 100%; min-height: 42px; border: 0; outline: 0; background: transparent; color: var(--fg); font-family: var(--font-mono); font-size: 12px; }
  .category-filter select { padding: 0 36px 0 14px; color-scheme: dark; cursor: pointer; }
  .category-filter select option { background: var(--bg); color: var(--fg); padding: 8px 14px; }
  .project-filter-count { display: flex; align-items: center; padding: 0 8px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; white-space: nowrap; }
  .project-table { --project-columns: minmax(100px, .85fr) minmax(145px, 1.3fr) minmax(78px, .65fr) minmax(98px, .78fr) minmax(112px, 1fr) 52px; border-top: 1px solid var(--border); }
  .project-table-header, .project-row, .project-loading-row { display: grid; grid-template-columns: var(--project-columns); align-items: center; gap: 12px; padding-left: 14px; }
  .project-table-header { min-height: 40px; border-bottom: 1px solid var(--border); color: var(--muted); font-family: var(--font-mono); font-size: 9px; letter-spacing: .08em; text-transform: uppercase; }
  .panel-loading { color: var(--muted); font-family: var(--font-mono); }
  .panel-loading-label { display: block; padding: 14px; border-bottom: 1px solid var(--border); font-size: 9px; letter-spacing: .04em; }
  .project-loading-rows { display: grid; }
  .project-loading-row { min-height: 64px; border-bottom: 1px solid var(--border); }
  .project-loading-row:last-child { border-bottom: 0; }
  .loading-stack { display: grid; justify-items: start; gap: 8px; }
  .loading-line, .coding-stats strong.stat-placeholder { background: linear-gradient(90deg, color-mix(in oklch, var(--fg) 8%, transparent) 20%, color-mix(in oklch, var(--fg) 18%, transparent) 45%, color-mix(in oklch, var(--fg) 8%, transparent) 70%); background-size: 240% 100%; animation: coding-skeleton-scan 1.35s cubic-bezier(.2, 0, 0, 1) infinite; }
  .loading-line { display: block; width: 62%; height: 7px; }
  .loading-menu { width: 32px; height: 32px; border: 1px solid var(--border); background: color-mix(in oklch, var(--fg) 6%, transparent); }
  .project-loading-row:nth-child(2) .loading-line { animation-delay: -.18s; }
  .project-loading-row:nth-child(3) .loading-line { animation-delay: -.36s; }
  .loading-line--short { width: 32%; }
  .loading-line--medium { width: 58%; }
  .loading-line--long { width: 82%; height: 9px; }
  .rail-loading { display: grid; }
  .rail-loading > .loading-stack { min-height: 72px; align-content: center; padding: 14px; }
  .panel-unavailable { display: grid; min-height: 340px; place-content: center; justify-items: center; gap: 7px; padding: 28px; color: var(--muted); text-align: center; }
  .panel-unavailable.compact { min-height: 96px; justify-items: start; text-align: left; }
  .panel-unavailable strong { color: var(--fg); font-family: var(--font-mono); font-size: 11px; font-weight: 550; }
  .panel-unavailable p { max-width: 48ch; margin: 0; font-size: 10px; line-height: 1.6; }
  .project-row { position: relative; z-index: 0; min-height: 64px; border-bottom: 1px solid var(--border); transition: background-color 120ms cubic-bezier(.2, 0, 0, 1); }
  .project-row:last-child { border-bottom: 0; }
  .project-row:hover { background: color-mix(in oklch, var(--fg) 4%, transparent); }
  .project-row.has-open-menu { z-index: 4; }
  .project-cell { min-width: 0; display: grid; align-content: center; gap: 3px; font-family: var(--font-mono); }
  .project-cell strong { overflow: hidden; font-size: 12px; font-weight: 550; text-overflow: ellipsis; white-space: nowrap; }
  .project-cell small, .project-cell time, .cell-unavailable { overflow: hidden; color: var(--muted); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .project-name-cell small { letter-spacing: .06em; text-transform: uppercase; }
  .repo-location-cell > a, .release-cell > a { display: inline-flex; min-width: 0; width: fit-content; max-width: 100%; align-items: center; gap: 5px; color: var(--fg); font-family: var(--font-mono); font-size: 10px; font-weight: 540; text-decoration: none; }
  .repo-location-cell > a span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .repo-location-cell > a :global(svg) { flex: 0 0 auto; }
  .repo-location-cell > a:hover span, .release-cell > a:hover { text-decoration: underline; text-underline-offset: 3px; }
  .category-chip-list { min-width: 0; display: flex; align-items: center; gap: 4px; overflow: hidden; }
  .category-chip-list > span { min-width: 0; max-width: 84px; overflow: hidden; border: 1px solid var(--border); padding: 2px 5px; color: var(--fg); font-family: var(--font-mono); font-size: 8px; letter-spacing: .02em; text-overflow: ellipsis; white-space: nowrap; }
  .category-chip-list > .category-empty { color: var(--muted); }
  .category-chip-list > .category-overflow { flex: 0 0 auto; color: var(--muted); }
  .mobile-column-label { display: none; color: var(--muted); font-family: var(--font-mono); font-size: 8px; letter-spacing: .07em; text-transform: uppercase; }
  .project-row-menu { position: relative; z-index: 2; display: grid; align-self: stretch; place-items: center; padding-right: 8px; }
  .project-row-menu-trigger { display: grid; width: 44px; height: 44px; min-height: 44px; place-items: center; border: 1px solid var(--border); background: transparent; color: var(--fg); }
  .project-row-menu-trigger:hover, .project-row-menu-trigger[aria-expanded="true"] { border-color: var(--fg); background: var(--fg-soft); }
  .project-row-menu-popover { position: absolute; z-index: 10; top: calc(50% + 24px); right: 8px; width: 184px; border: 1px solid var(--border); background: var(--bg); padding: 6px; }
  .project-row-menu-popover button { display: flex; width: 100%; min-height: 44px; align-items: center; gap: 9px; border: 1px solid transparent; background: transparent; padding: 0 10px; color: var(--fg); text-align: left; font-family: var(--font-mono); font-size: 10px; }
  .project-row-menu-popover button:hover { border-color: var(--border); background: var(--fg-soft); color: var(--fg); }
  .project-row-menu-popover .project-delete-action { color: var(--danger); }
  .project-row-menu-popover .project-delete-action:hover { border-color: color-mix(in oklch, var(--danger) 55%, var(--border)); background: color-mix(in oklch, var(--danger) 12%, transparent); color: var(--danger); }
  .project-row-menu-popover .project-delete-action.is-armed { border-color: var(--danger); background: var(--danger); color: var(--bg); }
  .coding-empty { display: grid; min-height: 340px; place-content: center; justify-items: center; padding: 30px; color: var(--muted); text-align: center; font-family: var(--font-mono); font-size: 11px; }
  .coding-empty h3 { margin: 14px 0 5px; color: var(--fg); font-size: 14px; }
  .coding-empty p { max-width: 48ch; margin: 0; line-height: 1.6; }
  .coding-empty button { margin-top: 16px; padding: 0 13px; font-family: var(--font-mono); font-size: 10px; }
  .coding-rail { display: grid; gap: 18px; }
  .merge-settings { padding: 10px 14px; border-bottom: 1px solid var(--border); }
  .archived-repository-toggle { width: 100%; min-height: 44px; justify-content: flex-start; border: 0; padding: 0; text-align: left; }
  .archived-repository-toggle > span:last-child { display: grid; gap: 2px; }
  .archived-repository-toggle strong { font-family: var(--font-mono); font-size: 10px; font-weight: 550; letter-spacing: .02em; }
  .archived-repository-toggle small { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .merge-list > a { display: grid; gap: 5px; padding: 13px 14px; border-bottom: 1px solid var(--border); color: var(--fg); text-decoration: none; }
  .merge-list > a:last-child { border-bottom: 0; }
  .merge-list > a:hover strong { text-decoration: underline; text-underline-offset: 3px; }
  .merge-list span, .merge-list small { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .merge-list strong { font-size: 12px; font-weight: 540; line-height: 1.45; }
  .merge-list small { display: flex; align-items: baseline; gap: 5px; }
  .merge-list small b { color: var(--fg); font-size: 15px; font-weight: 550; }
  .merge-list small.count-unavailable b { color: var(--muted); }
  .rail-empty { display: grid; gap: 12px; padding: 18px 14px; }
  .rail-empty p, .access-panel > p { margin: 0; color: var(--muted); font-size: 11px; line-height: 1.6; }
  .rail-empty button, .access-action { width: fit-content; min-height: 36px; padding: 0 12px; font-family: var(--font-mono); font-size: 9px; text-transform: uppercase; }
  .access-panel > p { padding: 14px 14px 0; }
  .credential-note { margin: 14px; border: 1px solid var(--border); padding: 12px; color: var(--muted); font-family: var(--font-mono); font-size: 9px; line-height: 1.6; }
  .credential-list { padding: 10px 14px 0; }
  .credential-list > div { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 0; border-bottom: 1px solid var(--border); }
  .credential-list > div > span { display: grid; gap: 3px; }
  .credential-list strong { font-family: var(--font-mono); font-size: 10px; text-transform: uppercase; }
  .credential-list small, .no-credentials { color: var(--muted); font-family: var(--font-mono); font-size: 9px; }
  .credential-list button { min-height: 30px; border: 0; padding: 0; color: var(--muted); font-family: var(--font-mono); font-size: 9px; text-decoration: underline; text-underline-offset: 3px; }
  .access-action { margin: 14px; display: inline-flex; align-items: center; gap: 7px; }
  .coding-dialog { position: fixed; inset: 0; width: min(620px, calc(100vw - 32px)); max-height: min(760px, calc(100dvh - 32px)); margin: auto; border: 1px solid var(--border); background: var(--page-surface, var(--surface)); color: var(--fg); padding: 0; overflow: auto; overscroll-behavior: contain; }
  .coding-dialog::backdrop { background: oklch(5% 0 0 / .72); backdrop-filter: blur(5px); }
  .coding-dialog > header { display: flex; align-items: center; justify-content: space-between; padding: 18px 20px; border-bottom: 1px solid var(--border); }
  .coding-dialog > header > div { min-width: 0; }
  .coding-dialog h2 { margin: 6px 0 0; font-family: var(--font-mono); font-size: 20px; font-weight: 550; }
  .coding-dialog-close { display: inline-grid; width: 44px; height: 44px; min-height: 44px; flex: 0 0 44px; place-items: center; align-self: center; padding: 0; line-height: 0; }
  .coding-dialog form { display: grid; gap: 9px; padding: 20px; }
  .coding-category-dialog { --category-dialog-top: max(16px, env(safe-area-inset-top, 0px)); --category-dialog-right: max(16px, env(safe-area-inset-right, 0px)); --category-dialog-bottom: max(16px, env(safe-area-inset-bottom, 0px)); --category-dialog-left: max(16px, env(safe-area-inset-left, 0px)); inset: var(--category-dialog-top) var(--category-dialog-right) var(--category-dialog-bottom) var(--category-dialog-left); width: min(560px, calc(100vw - var(--category-dialog-left) - var(--category-dialog-right))); max-height: min(720px, calc(100dvh - var(--category-dialog-top) - var(--category-dialog-bottom))); overflow: hidden; }
  .coding-category-dialog[open] { display: flex; flex-direction: column; }
  .coding-category-dialog > header { flex: 0 0 auto; }
  .coding-category-dialog h2 { overflow-wrap: anywhere; }
  .coding-category-dialog > form { min-height: 0; flex: 1 1 auto; display: flex; flex-direction: column; gap: 0; padding: 0; overflow: hidden; }
  .coding-category-dialog-body { min-height: 0; display: grid; flex: 1 1 auto; align-content: start; gap: 9px; overflow-y: auto; overscroll-behavior: contain; scrollbar-gutter: stable; padding: 20px; }
  .coding-category-dialog .category-options { max-height: none; overflow: visible; }
  .coding-dialog.coding-category-dialog footer { flex: 0 0 auto; margin: 0; padding: 14px 20px; border-top: 1px solid var(--border); background: var(--page-surface, var(--surface)); }
  .coding-dialog label { font-family: var(--font-mono); font-size: 10px; letter-spacing: .04em; }
  .coding-dialog input, .coding-dialog select { width: 100%; padding: 0 12px; background: var(--bg); }
  .field-help { margin: 3px 0 6px; color: var(--muted); font-size: 10px; line-height: 1.6; }
  .field-help code { color: var(--fg); font-family: var(--font-mono); }
  .project-edit-summary { display: grid; grid-template-columns: minmax(110px, .45fr) minmax(0, 1.55fr); border: 1px solid var(--border); background: var(--bg); }
  .project-edit-summary > span { min-width: 0; display: grid; gap: 4px; padding: 11px 12px; border-right: 1px solid var(--border); }
  .project-edit-summary > span:last-child { border-right: 0; }
  .project-edit-summary small { color: var(--muted); font-family: var(--font-mono); font-size: 8px; letter-spacing: .08em; text-transform: uppercase; }
  .project-edit-summary strong { overflow-wrap: anywhere; font-family: var(--font-mono); font-size: 10px; font-weight: 550; line-height: 1.5; }
  .category-create { display: grid; gap: 8px; padding-bottom: 14px; border-bottom: 1px solid var(--border); }
  .category-create > div { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px; }
  .category-create button { display: inline-flex; min-width: 92px; min-height: 44px; align-items: center; justify-content: center; gap: 7px; padding: 0 13px; font-family: var(--font-mono); font-size: 9px; text-transform: uppercase; }
  .category-options { display: grid; max-height: 330px; overflow: auto; scrollbar-gutter: stable; border: 1px solid var(--border); }
  .category-option { display: grid; grid-template-columns: minmax(0, 1fr) 44px; align-items: center; border-bottom: 1px solid var(--border); }
  .category-option:last-child { border-bottom: 0; }
  .category-toggle { min-width: 0; min-height: 48px; justify-content: flex-start; border: 0; padding: 0 12px; text-align: left; }
  .category-toggle > span:last-child { overflow: hidden; font-family: var(--font-mono); font-size: 10px; font-weight: 550; text-overflow: ellipsis; white-space: nowrap; }
  .delete-category { display: grid; width: 44px; min-height: 44px; place-items: center; border: 0; border-left: 1px solid var(--border); }
  .delete-category.confirm { color: oklch(72% .16 25); }
  .category-options-empty { margin: 0; padding: 18px 14px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; line-height: 1.6; }
  .form-grid { display: grid; grid-template-columns: 1fr 1.5fr; gap: 12px; }
  .form-grid > div { display: grid; gap: 8px; }
  .coding-dialog footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 10px; }
  .coding-dialog footer > button:not(.coding-primary) { padding: 0 15px; font-family: var(--font-mono); font-size: 10px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
  :focus-visible { outline: 2px solid var(--fg); outline-offset: 2px; }
  @keyframes coding-skeleton-scan { from { background-position: 100% 0; } to { background-position: -140% 0; } }
  @media (max-width: 1100px) { .coding-layout { grid-template-columns: 1fr; } .coding-rail { grid-template-columns: 1fr 1fr; } }
  @media (max-width: 780px) {
    .coding-header { align-items: stretch; flex-direction: column; }
    .header-actions { flex-wrap: wrap; }
    .coding-stats { grid-template-columns: repeat(2, 1fr); }
    .coding-stats > div:nth-child(2) { border-right: 0; }
    .coding-stats > div:nth-child(-n + 2) { border-bottom: 1px solid var(--border); }
    .project-filters { grid-template-columns: minmax(0, 1fr) minmax(190px, .65fr); }
    .project-filter-count { grid-column: 1 / -1; padding: 2px 3px; }
    .project-table-header { display: none; }
    .project-row { grid-template-columns: minmax(0, 1fr) 52px; align-items: start; gap: 10px; padding: 14px 0 14px 14px; }
    .project-cell { grid-column: 1; }
    .mobile-column-label { display: block; margin-top: 5px; }
    .project-row-menu { grid-column: 2; grid-row: 1; place-items: start center; padding-right: 8px; }
    .project-row-menu-popover { top: 48px; }
    .project-loading-row { grid-template-columns: minmax(0, 1fr) 52px; align-items: start; gap: 10px; padding: 14px 0 14px 14px; }
    .project-loading-row .loading-stack { grid-column: 1; }
    .project-loading-row .loading-menu { grid-column: 2; grid-row: 1; }
    .coding-rail { grid-template-columns: 1fr; }
  }
  @media (max-width: 560px) {
    .header-actions > button { flex: 1; }
    .coding-stats { grid-template-columns: 1fr; }
    .coding-stats > div { border-right: 0; border-bottom: 1px solid var(--border); }
    .coding-stats > div:last-child { border-bottom: 0; }
    .project-filters { grid-template-columns: 1fr; }
    .project-search, .category-filter { width: 100%; min-width: 0; }
    .project-filter-count { grid-column: auto; }
    .project-edit-summary, .category-create > div, .form-grid { grid-template-columns: 1fr; }
    .project-edit-summary > span { border-right: 0; border-bottom: 1px solid var(--border); }
    .project-edit-summary > span:last-child { border-bottom: 0; }
    .coding-dialog footer { align-items: stretch; flex-direction: column-reverse; }
  }
  @media (prefers-reduced-motion: reduce) { .loading-line, .coding-stats strong.stat-placeholder { animation: none; } }
</style>

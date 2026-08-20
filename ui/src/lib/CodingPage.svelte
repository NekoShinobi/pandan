<script lang="ts">
  import ArrowUpRight from "lucide-svelte/icons/arrow-up-right";
  import CircleDot from "lucide-svelte/icons/circle-dot";
  import GitBranch from "lucide-svelte/icons/git-branch";
  import GitMerge from "lucide-svelte/icons/git-merge";
  import KeyRound from "lucide-svelte/icons/key-round";
  import Plus from "lucide-svelte/icons/plus";
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import Search from "lucide-svelte/icons/search";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    createCodingProject,
    deleteCodingProject,
    fetchCoding,
    updateCodingCredential,
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
  };
  const providerHosts: Record<CodingProvider, string> = {
    github: "github.com",
    gitlab: "gitlab.com",
    codeberg: "codeberg.org",
    gitea: "",
    forgejo: "",
  };

  let coding = $state.raw<CodingResponse>(emptyCoding);
  let hasLoaded = false;
  let loading = $state(true);
  let refreshing = $state(false);
  let initialLoadFailed = $state(false);
  let pageError = $state("");
  let query = $state("");
  let repository = $state("");
  let repositoryError = $state("");
  let savingRepository = $state(false);
  let deleteId = $state("");
  let projectDialog = $state<HTMLDialogElement>();
  let credentialDialog = $state<HTMLDialogElement>();
  let repositoryInput = $state<HTMLInputElement>();
  let credentialProvider = $state<CodingProvider>("gitlab");
  let credentialHost = $state("gitlab.com");
  let credentialToken = $state("");
  let credentialError = $state("");
  let savingCredential = $state(false);

  let filteredProjects = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    const matchingProjects = needle
      ? coding.projects.filter((project) =>
          [project.provider, project.host, project.repository].some((value) =>
            value.toLowerCase().includes(needle),
          ),
        )
      : coding.projects;
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
  let openPullRequestCount = $derived(
    coding.owned_repositories.reduce(
      (total, repository) => total + (repository.open_pull_requests ?? 0),
      0,
    ),
  );
  let pendingRepositories = $derived(
    coding.owned_repositories.filter(
      (repository) => (repository.open_pull_requests ?? 0) > 0,
    ),
  );

  onMount(() => {
    void loadCoding();
  });

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

  function captureRepositoryInput(node: HTMLInputElement) {
    repositoryInput = node;
    return () => {
      repositoryInput = undefined;
    };
  }

  async function loadCoding() {
    const initialLoad = !hasLoaded;
    if (initialLoad) loading = true;
    else refreshing = true;
    initialLoadFailed = false;
    pageError = "";
    try {
      coding = await fetchCoding();
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
      deleteId = "";
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

  function pipelineFor(projectId: string) {
    return coding.pipelines.find((pipeline) => pipeline.project_id === projectId);
  }

  function projectUrl(project: CodingProject) {
    return `https://${project.host}/${project.repository}`;
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
</script>

<section class="coding-page product-page" data-od-id="coding-page">
  <header class="coding-header page-header" data-od-id="coding-header">
    <div>
      <TypedHeading text="$ coding --watch" odId="coding-heading" />
      <p>Monitor releases, pending review work, and the newest pipelines for subscribed projects.</p>
    </div>
    <div class="header-actions">
      <button type="button" onclick={loadCoding} disabled={loading || refreshing} data-od-id="refresh-coding">
        <RefreshCw class={refreshing ? "spinning" : undefined} size={15} strokeWidth={1.8} aria-hidden="true" />
        {loading ? "Loading…" : refreshing ? "Refreshing…" : initialLoadFailed ? "Retry" : "Refresh"}
      </button>
      <button class="ui-button ui-button--primary coding-primary" type="button" onclick={openProjectDialog} data-od-id="add-coding-project">
        <Plus size={16} strokeWidth={1.8} aria-hidden="true" /> Add project
      </button>
    </div>
  </header>

  {#if pageError}<p class="coding-error" role="alert">{pageError}</p>{/if}
  {#if loading}<p class="sr-only" role="status">Loading coding activity.</p>{/if}
  {#if coding.provider_errors.length}
    <details class="provider-errors">
      <summary>{coding.provider_errors.length} provider {coding.provider_errors.length === 1 ? "request" : "requests"} could not refresh</summary>
      <ul>{#each coding.provider_errors as error (error)}<li>{error}</li>{/each}</ul>
    </details>
  {/if}

  <div class="coding-stats" aria-label="Coding overview" aria-busy={loading}>
    <div><span>Projects</span><strong class:stat-placeholder={loading}>{loading ? "" : initialLoadFailed ? "—" : coding.projects.length}</strong></div>
    <div><span>Releases loaded</span><strong class:stat-placeholder={loading}>{loading ? "" : initialLoadFailed ? "—" : coding.releases.length}</strong></div>
    <div><span>Open pull requests</span><strong class:stat-placeholder={loading}>{loading ? "" : initialLoadFailed ? "—" : openPullRequestCount}</strong></div>
    <div><span>Recent pipelines</span><strong class:stat-placeholder={loading}>{loading ? "" : initialLoadFailed ? "—" : coding.pipelines.length}</strong></div>
  </div>

  <div class="coding-layout">
    <section class="projects-panel" aria-busy={loading} data-od-id="coding-projects">
      <header>
        <div><span>[ SUBSCRIPTIONS ]</span><h3>Projects</h3></div>
        <label>
          <Search size={14} strokeWidth={1.8} aria-hidden="true" />
          <span class="sr-only">Filter projects</span>
          <input type="search" bind:value={query} disabled={loading || initialLoadFailed} placeholder="Filter provider, host, or repository…" />
        </label>
      </header>

      {#if loading}
        <div class="panel-loading project-loading" role="status" data-od-id="coding-projects-loading">
          <span class="panel-loading-label">Loading subscribed projects…</span>
          <div class="project-loading-rows" aria-hidden="true">
            {#each [0, 1, 2] as row (row)}
              <div class="project-loading-row">
                <div class="loading-stack"><i class="loading-line loading-line--short"></i><i class="loading-line loading-line--long"></i><i class="loading-line loading-line--medium"></i></div>
                <div class="loading-stack"><i class="loading-line loading-line--short"></i><i class="loading-line loading-line--medium"></i></div>
                <div class="loading-stack"><i class="loading-line loading-line--short"></i><i class="loading-line loading-line--medium"></i></div>
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
        <div class="project-list">
          {#each filteredProjects as project (project.id)}
            {@const release = releaseFor(project.id)}
            {@const pipeline = pipelineFor(project.id)}
            <article data-od-id={`coding-project-${project.id}`}>
              <div class="project-main">
                <span class="provider-label">{project.provider}</span>
                <a href={projectUrl(project)} target="_blank" rel="noreferrer">
                  <strong>{project.repository}</strong>
                  <ArrowUpRight size={14} strokeWidth={1.7} aria-hidden="true" />
                </a>
                <small>{project.host}{project.has_credential ? " · authenticated" : " · public access"}</small>
              </div>
              <div class="release-cell">
                <span>Latest release</span>
                {#if release}
                  <a href={release.url} target="_blank" rel="noreferrer">{release.version}</a>
                  <small>{formatDate(release.published_at)}</small>
                {:else}
                  <strong>Unavailable</strong><small>No release returned</small>
                {/if}
              </div>
              <div class="pipeline-cell">
                <span>Pipeline</span>
                {#if project.provider === "gitlab" && pipeline}
                  <a href={pipeline.url} target="_blank" rel="noreferrer" class={`pipeline-${pipeline.status}`}>
                    <CircleDot size={13} strokeWidth={1.8} aria-hidden="true" /> {pipeline.status}
                  </a>
                  <small>{pipeline.reference || "detached"} · {pipeline.sha}</small>
                {:else if project.provider === "gitlab" && !project.has_credential}
                  <button class="ui-button ui-button--secondary inline-action" type="button" onclick={() => openCredential("gitlab", project.host)}>Add provider token</button>
                  <small>Token required</small>
                {:else}
                  <strong>—</strong><small>Not available for this provider</small>
                {/if}
              </div>
              <button
                class:confirm={deleteId === project.id}
                class="ui-button ui-button--danger ui-button--icon delete-project"
                type="button"
                aria-label={deleteId === project.id ? `Confirm removal of ${project.repository}` : `Remove ${project.repository}`}
                title={deleteId === project.id ? "Select again to confirm" : "Remove project"}
                onclick={() => removeProject(project)}
              >
                <Trash2 size={15} strokeWidth={1.7} aria-hidden="true" />
              </button>
            </article>
          {:else}
            <div class="coding-empty" data-od-id={query ? "coding-filter-empty-state" : "coding-projects-empty-state"}>
              <GitBranch size={28} strokeWidth={1.4} aria-hidden="true" />
              <h3>{query ? "No matching projects" : "No projects subscribed"}</h3>
              <p>{query ? "Try a broader filter." : "Add a repository to begin watching its latest release."}</p>
              {#if !query}<button type="button" onclick={openProjectDialog}>Add your first project</button>{/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <aside class="coding-rail">
      <section class="merge-panel" aria-busy={loading} data-od-id="coding-merge-requests">
        <header><div><span>[ OWNED REPOSITORIES ]</span><h3>Pending pull requests</h3></div><GitMerge size={18} strokeWidth={1.6} aria-hidden="true" /></header>
        {#if loading}
          <div class="panel-loading rail-loading" role="status" data-od-id="coding-merge-requests-loading">
            <span class="panel-loading-label">Loading pull requests…</span>
            <div class="loading-stack" aria-hidden="true"><i class="loading-line loading-line--short"></i><i class="loading-line loading-line--long"></i><i class="loading-line loading-line--medium"></i></div>
          </div>
        {:else if initialLoadFailed}
          <div class="panel-unavailable compact"><strong>Pull request data unavailable</strong></div>
        {:else if !profileActivityConnected}
          <div class="rail-empty">
            <p>Connect a provider token to discover repositories owned by that account and count their open pull requests.</p>
            <button type="button" onclick={() => openCredential()}>Connect provider</button>
          </div>
        {:else}
          <div class="merge-list">
            {#each pendingRepositories as repository (`${repository.provider}:${repository.host}:${repository.repository}`)}
              <a href={repository.url} target="_blank" rel="noreferrer" data-od-id={`owned-repository-${repository.provider}-${repository.repository.replaceAll("/", "-")}`}>
                <span>{repository.provider} · {repository.host}</span>
                <strong>{repository.repository}</strong>
                <small class:count-unavailable={repository.open_pull_requests === null}>
                  <b>{repository.open_pull_requests ?? "—"}</b> {pullRequestLabel(repository)}
                </small>
              </a>
            {:else}
              <div class="rail-empty"><p>No owned repositories have open pull requests.</p></div>
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
          <div class="credential-note">Configure server secret storage before saving provider tokens.</div>
        {:else}
          <div class="credential-list">
            {#each coding.credentials as credential (`${credential.provider}:${credential.host}`)}
              <div><span><strong>{credential.provider}</strong><small>{credential.host}</small></span><button class="ui-button ui-button--danger" type="button" onclick={() => removeCredential(credential)}>Disconnect</button></div>
            {:else}
              <span class="no-credentials">No provider tokens stored.</span>
            {/each}
          </div>
          <button class="ui-button ui-button--secondary access-action" type="button" onclick={() => openCredential()}><Plus size={14} strokeWidth={1.8} /> Add provider token</button>
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
  .coding-error, .form-error { margin: 0; border: 1px solid oklch(60% .16 25 / .5); background: oklch(20% .04 25 / .75); padding: 10px 12px; color: oklch(82% .09 25); font-family: var(--font-mono); font-size: 11px; }
  .provider-errors { border: 1px solid var(--border); background: var(--page-surface, var(--surface)); padding: 10px 12px; color: var(--muted); font-family: var(--font-mono); font-size: 10px; }
  .provider-errors summary { cursor: pointer; color: var(--fg); }
  .provider-errors ul { display: grid; gap: 6px; margin: 10px 0 0; padding-left: 20px; }
  .coding-stats { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); border: 1px solid var(--border); background: var(--page-surface, var(--surface)); }
  .coding-stats > div { display: flex; min-height: 70px; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 16px; border-right: 1px solid var(--border); }
  .coding-stats > div:last-child { border-right: 0; }
  .coding-stats span { color: var(--muted); font-family: var(--font-mono); font-size: 9px; letter-spacing: .07em; text-transform: uppercase; }
  .coding-stats strong { font-family: var(--font-mono); font-size: 23px; font-weight: 520; }
  .coding-stats strong.stat-placeholder { width: 30px; height: 9px; background: color-mix(in oklch, var(--fg) 13%, transparent); }
  .coding-layout { display: grid; grid-template-columns: minmax(0, 1.55fr) minmax(320px, .8fr); gap: 18px; align-items: start; }
  .projects-panel, .coding-rail > section { min-width: 0; border: 1px solid var(--border); background: var(--page-surface, var(--surface)); }
  .projects-panel > header, .coding-rail > section > header { display: flex; min-height: 62px; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 14px; border-bottom: 1px solid var(--border); }
  .projects-panel h3, .coding-rail h3 { margin: 5px 0 0; font-family: var(--font-mono); font-size: 14px; font-weight: 550; }
  .projects-panel label { display: flex; width: min(380px, 50%); min-height: 38px; align-items: center; gap: 8px; border: 1px solid var(--border); padding: 0 10px; }
  .projects-panel input { width: 100%; min-height: 36px; border: 0; outline: 0; font-family: var(--font-mono); font-size: 10px; }
  .panel-loading { color: var(--muted); font-family: var(--font-mono); }
  .panel-loading-label { display: block; padding: 14px; border-bottom: 1px solid var(--border); font-size: 9px; letter-spacing: .04em; }
  .project-loading-rows { display: grid; }
  .project-loading-row { display: grid; grid-template-columns: minmax(180px, 1.3fr) minmax(120px, .8fr) minmax(125px, .8fr); min-height: 94px; align-items: center; gap: 16px; padding: 14px; border-bottom: 1px solid var(--border); }
  .project-loading-row:last-child { border-bottom: 0; }
  .loading-stack { display: grid; justify-items: start; gap: 8px; }
  .loading-line { display: block; width: 62%; height: 7px; background: color-mix(in oklch, var(--fg) 11%, transparent); }
  .loading-line--short { width: 32%; }
  .loading-line--medium { width: 58%; }
  .loading-line--long { width: 82%; height: 9px; }
  .rail-loading { display: grid; }
  .rail-loading > .loading-stack { min-height: 72px; align-content: center; padding: 14px; }
  .panel-unavailable { display: grid; min-height: 340px; place-content: center; justify-items: center; gap: 7px; padding: 28px; color: var(--muted); text-align: center; }
  .panel-unavailable.compact { min-height: 96px; justify-items: start; text-align: left; }
  .panel-unavailable strong { color: var(--fg); font-family: var(--font-mono); font-size: 11px; font-weight: 550; }
  .panel-unavailable p { max-width: 48ch; margin: 0; font-size: 10px; line-height: 1.6; }
  .project-list > article { display: grid; grid-template-columns: minmax(180px, 1.3fr) minmax(120px, .8fr) minmax(125px, .8fr) 38px; align-items: center; gap: 16px; min-height: 94px; padding: 14px; border-bottom: 1px solid var(--border); }
  .project-list > article:last-child { border-bottom: 0; }
  .project-main, .release-cell, .pipeline-cell { min-width: 0; display: grid; gap: 5px; }
  .provider-label { width: fit-content; border: 1px solid var(--border); padding: 2px 6px; color: var(--muted); font-family: var(--font-mono); font-size: 8px; letter-spacing: .08em; text-transform: uppercase; }
  .project-main > a, .release-cell > a, .pipeline-cell > a { display: inline-flex; min-width: 0; width: fit-content; align-items: center; gap: 5px; color: var(--fg); text-decoration: none; }
  .project-main > a:hover strong, .release-cell > a:hover, .pipeline-cell > a:hover { text-decoration: underline; text-underline-offset: 3px; }
  .project-main strong { overflow: hidden; text-overflow: ellipsis; font-size: 13px; font-weight: 550; }
  .project-main small, .release-cell small, .pipeline-cell small { overflow: hidden; color: var(--muted); font-family: var(--font-mono); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .release-cell > span, .pipeline-cell > span { color: var(--muted); font-family: var(--font-mono); font-size: 8px; letter-spacing: .07em; text-transform: uppercase; }
  .release-cell > a, .release-cell > strong, .pipeline-cell > a, .pipeline-cell > strong { font-family: var(--font-mono); font-size: 11px; font-weight: 540; }
  .pipeline-success { color: var(--fg); }
  .pipeline-failed, .pipeline-canceled { color: oklch(72% .15 25); }
  .pipeline-running, .pipeline-pending { color: var(--accent); }
  .inline-action { width: fit-content; min-height: 28px; border: 0; padding: 0; font-family: var(--font-mono); font-size: 10px; text-decoration: underline; text-underline-offset: 3px; }
  .delete-project { display: grid; width: 36px; min-height: 36px; place-items: center; }
  .delete-project:hover { border-color: var(--fg); }
  .delete-project.confirm { border-color: oklch(62% .19 25); color: oklch(72% .16 25); }
  .coding-empty { display: grid; min-height: 340px; place-content: center; justify-items: center; padding: 30px; color: var(--muted); text-align: center; font-family: var(--font-mono); font-size: 11px; }
  .coding-empty h3 { margin: 14px 0 5px; color: var(--fg); font-size: 14px; }
  .coding-empty p { max-width: 48ch; margin: 0; line-height: 1.6; }
  .coding-empty button { margin-top: 16px; padding: 0 13px; font-family: var(--font-mono); font-size: 10px; }
  .coding-rail { display: grid; gap: 18px; }
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
  .coding-dialog label { font-family: var(--font-mono); font-size: 10px; letter-spacing: .04em; }
  .coding-dialog input, .coding-dialog select { width: 100%; padding: 0 12px; background: var(--bg); }
  .field-help { margin: 3px 0 6px; color: var(--muted); font-size: 10px; line-height: 1.6; }
  .field-help code { color: var(--fg); font-family: var(--font-mono); }
  .form-grid { display: grid; grid-template-columns: 1fr 1.5fr; gap: 12px; }
  .form-grid > div { display: grid; gap: 8px; }
  .coding-dialog footer { display: flex; justify-content: flex-end; gap: 8px; margin-top: 10px; }
  .coding-dialog footer > button:not(.coding-primary) { padding: 0 15px; font-family: var(--font-mono); font-size: 10px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
  :focus-visible { outline: 2px solid var(--fg); outline-offset: 2px; }
  @media (max-width: 1100px) { .coding-layout { grid-template-columns: 1fr; } .coding-rail { grid-template-columns: 1fr 1fr; } }
  @media (max-width: 780px) { .coding-header { align-items: stretch; flex-direction: column; } .header-actions { flex-wrap: wrap; } .coding-stats { grid-template-columns: repeat(2, 1fr); } .coding-stats > div:nth-child(2) { border-right: 0; } .coding-stats > div:nth-child(-n + 2) { border-bottom: 1px solid var(--border); } .projects-panel > header { align-items: stretch; flex-direction: column; } .projects-panel label { width: 100%; } .project-list > article { grid-template-columns: minmax(0, 1fr) 36px; } .release-cell, .pipeline-cell { grid-column: 1; } .delete-project { grid-column: 2; grid-row: 1; } .coding-rail { grid-template-columns: 1fr; } }
  @media (max-width: 560px) { .header-actions > button { flex: 1; } .coding-stats { grid-template-columns: 1fr; } .coding-stats > div { border-right: 0; border-bottom: 1px solid var(--border); } .coding-stats > div:last-child { border-bottom: 0; } .form-grid { grid-template-columns: 1fr; } .coding-dialog footer { align-items: stretch; flex-direction: column-reverse; } }
</style>

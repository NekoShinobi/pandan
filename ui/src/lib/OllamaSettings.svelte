<script lang="ts">
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import { onMount } from "svelte";
  import {
    fetchOllamaModels,
    fetchOllamaSettings,
    updateOllamaSettings,
    type OllamaModel,
    type OllamaSettings as OllamaSettingsRecord,
  } from "$lib/api";

  let settings = $state<OllamaSettingsRecord | null>(null);
  let enabled = $state(false);
  let baseUrl = $state("http://localhost:11434");
  let model = $state("gemma3:4b");
  let prompt = $state("");
  let tagCount = $state(5);
  let models = $state.raw<OllamaModel[]>([]);
  let loading = $state(true);
  let loadingModels = $state(false);
  let saving = $state(false);
  let notice = $state("");
  let error = $state("");

  const modelOptions = $derived.by(() => {
    const available = [...models];
    if (model && !available.some((candidate) => candidate.name === model)) {
      available.unshift({ name: model, size: 0, parameter_size: "" });
    }
    return available;
  });

  onMount(() => {
    void loadSettings();
  });

  async function loadSettings() {
    loading = true;
    error = "";
    try {
      applySettings(await fetchOllamaSettings());
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to load Ollama settings";
    } finally {
      loading = false;
    }
  }

  function applySettings(next: OllamaSettingsRecord) {
    settings = next;
    enabled = next.enabled;
    baseUrl = next.base_url;
    model = next.model;
    prompt = next.prompt;
    tagCount = next.tag_count;
  }

  async function loadModels() {
    if (loadingModels || !baseUrl.trim()) return;
    loadingModels = true;
    notice = "";
    error = "";
    try {
      models = await fetchOllamaModels(baseUrl.trim());
      notice = models.length
        ? `Found ${models.length} installed ${models.length === 1 ? "model" : "models"}.`
        : "Ollama is reachable, but it has no installed models.";
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to load installed Ollama models";
    } finally {
      loadingModels = false;
    }
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true;
    notice = "";
    error = "";
    try {
      const updated = await updateOllamaSettings({
        enabled,
        base_url: baseUrl.trim(),
        model: model.trim(),
        prompt: prompt.trim(),
        tag_count: tagCount,
      });
      applySettings(updated);
      notice = updated.enabled
        ? "Ollama is enabled and the selected vision model was verified."
        : "Ollama settings saved. The integration remains disabled.";
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to save Ollama settings";
    } finally {
      saving = false;
    }
  }

  function formatModel(option: OllamaModel) {
    const details = [
      option.parameter_size,
      option.size > 0 ? formatBytes(option.size) : "",
    ].filter(Boolean);
    return details.length ? `${option.name} · ${details.join(" · ")}` : option.name;
  }

  function formatBytes(bytes: number) {
    if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
    if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(0)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }
</script>

<section
  class="authentication-policy ollama-settings"
  aria-labelledby="ollama-settings-title"
  data-od-id="ollama-settings"
>
  <div class="authentication-policy-heading">
    <div>
      <p class="widget-kicker">[ LOCAL AI ]</p>
      <h3 id="ollama-settings-title">Ollama</h3>
    </div>
    <span>{enabled ? "Enabled" : "Disabled"}</span>
  </div>

  {#if loading}
    <p class="network-access-empty" role="status">Loading Ollama settings…</p>
  {:else}
    <p class="network-access-intro">
      Connect one Ollama instance for administrator-run features. Wall tagging
      sends only the selected wall's bounded thumbnail and returns suggestions
      for review before they are saved.
    </p>

    <form class="ollama-form" onsubmit={save}>
      <div class="authentication-policy-row">
        <span>
          <strong id="ollama-enabled-label">Enable Ollama features</strong>
          <small id="ollama-enabled-description">
            Enabling verifies that the selected installed model supports images.
          </small>
        </span>
        <button
          class="ui-toggle-button authentication-policy-toggle"
          type="button"
          aria-pressed={enabled}
          aria-labelledby="ollama-enabled-label"
          aria-describedby="ollama-enabled-description"
          disabled={saving}
          onclick={() => (enabled = !enabled)}
          data-od-id="ollama-enabled"
        >
          <span class="ui-toggle-indicator" aria-hidden="true"></span>
        </button>
      </div>

      <div class="ollama-fields">
        <label class="ollama-origin">
          <span>Ollama base URL</span>
          <input
            class="text-input"
            type="url"
            bind:value={baseUrl}
            maxlength="2000"
            required
            autocomplete="url"
            spellcheck="false"
            data-od-id="ollama-base-url"
          />
          <small>
            A local or HTTP origin needs an exact AI / Ollama allow rule below.
          </small>
        </label>
        <button
          class="ui-button ui-button--secondary ollama-load-models"
          type="button"
          disabled={loadingModels || saving || !baseUrl.trim()}
          onclick={() => void loadModels()}
          data-od-id="load-ollama-models"
        >
          <RefreshCw
            class={loadingModels ? "spinning" : undefined}
            size={15}
            strokeWidth={1.8}
            aria-hidden="true"
          />
          {loadingModels ? "Checking…" : "Load installed models"}
        </button>

        <label>
          <span>Vision model</span>
          <select
            class="select-input"
            bind:value={model}
            disabled={saving}
            required
            data-od-id="ollama-model"
          >
            {#each modelOptions as option (option.name)}
              <option value={option.name}>{formatModel(option)}</option>
            {/each}
          </select>
          <small>The default is the compact image-capable gemma3:4b model.</small>
        </label>

        <label>
          <span>Number of tags</span>
          <input
            class="text-input"
            type="number"
            min="1"
            max="8"
            bind:value={tagCount}
            disabled={saving}
            required
            data-od-id="ollama-tag-count"
          />
          <small>Walls support one to eight distinct tags.</small>
        </label>

        <label class="ollama-prompt">
          <span>Wall tagging prompt</span>
          <textarea
            class="text-input"
            rows="4"
            maxlength="2000"
            bind:value={prompt}
            disabled={saving}
            required
            data-od-id="ollama-wall-prompt"
          ></textarea>
          <small>
            Pandan adds the exact tag count and structured response rules.
          </small>
        </label>
      </div>

      <div class="ollama-actions">
        {#if settings?.last_verified_at}
          <small>
            Model last verified {new Date(settings.last_verified_at).toLocaleString()}.
          </small>
        {:else}
          <small>The model has not been verified yet.</small>
        {/if}
        <button
          class="ui-button ui-button--primary"
          type="submit"
          disabled={saving || !baseUrl.trim() || !model.trim() || !prompt.trim()}
          data-od-id="save-ollama-settings"
        >
          {saving ? (enabled ? "Verifying…" : "Saving…") : "Save Ollama settings"}
        </button>
      </div>
    </form>
  {/if}

  {#if notice}
    <p class="settings-page-notice" role="status">{notice}</p>
  {/if}
  {#if error}
    <p class="form-error" role="alert">{error}</p>
  {/if}
</section>

<style>
  .ollama-settings,
  .ollama-form {
    display: grid;
    gap: 18px;
  }

  .ollama-fields {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(160px, 0.34fr);
    gap: 16px;
  }

  .ollama-fields label {
    display: grid;
    align-content: start;
    gap: 7px;
    min-width: 0;
  }

  .ollama-fields label > span {
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 590;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .ollama-fields label > small,
  .ollama-actions small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.5;
  }

  .ollama-prompt {
    grid-column: 1 / -1;
  }

  .ollama-load-models {
    align-self: center;
    justify-self: start;
  }

  .ollama-prompt textarea {
    min-height: 112px;
    resize: vertical;
  }

  .ollama-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }

  @media (max-width: 720px) {
    .ollama-fields {
      grid-template-columns: 1fr;
    }

    .ollama-prompt {
      grid-column: auto;
    }

    .ollama-actions {
      align-items: stretch;
      flex-direction: column;
    }
  }
</style>

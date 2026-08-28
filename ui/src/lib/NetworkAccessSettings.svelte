<script lang="ts">
  import { onMount } from "svelte";
  import {
    createNetworkAccessRule,
    deleteNetworkAccessRule,
    fetchNetworkAccessRules,
    type NetworkAccessAction,
    type NetworkAccessIntegration,
    type NetworkAccessRule,
  } from "$lib/api";

  const integrationOptions: Array<{
    value: NetworkAccessIntegration;
    label: string;
  }> = [
    { value: "all", label: "All managed fetches" },
    { value: "rss", label: "RSS and Atom" },
    { value: "calendar", label: "Calendars" },
    { value: "contacts", label: "Contacts and CardDAV" },
    { value: "podcasts", label: "Podcasts" },
    { value: "notifications", label: "Notifications and ntfy actions" },
    { value: "coding", label: "Coding providers" },
    { value: "images", label: "Remote profile images" },
    { value: "youtube", label: "YouTube and Invidious" },
    { value: "widgets", label: "Other remote widgets" },
    { value: "jellyfin", label: "Jellyfin music" },
  ];

  let rules = $state.raw<NetworkAccessRule[]>([]);
  let action = $state<NetworkAccessAction>("allow");
  let integration = $state<NetworkAccessIntegration>("all");
  let origin = $state("");
  let loading = $state(true);
  let saving = $state(false);
  let deletingId = $state("");
  let pendingDeleteId = $state("");
  let error = $state("");

  onMount(() => {
    void loadRules();
  });

  async function loadRules() {
    loading = true;
    error = "";
    try {
      rules = await fetchNetworkAccessRules();
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to load network access rules";
    } finally {
      loading = false;
    }
  }

  async function addRule(event: SubmitEvent) {
    event.preventDefault();
    if (saving || !origin.trim()) return;
    saving = true;
    error = "";
    try {
      const rule = await createNetworkAccessRule({
        action,
        origin: origin.trim(),
        integration,
      });
      rules = [...rules, rule].sort(compareRules);
      origin = "";
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to create network access rule";
    } finally {
      saving = false;
    }
  }

  async function removeRule(rule: NetworkAccessRule) {
    if (deletingId) return;
    if (pendingDeleteId !== rule.id) {
      pendingDeleteId = rule.id;
      return;
    }
    deletingId = rule.id;
    error = "";
    try {
      await deleteNetworkAccessRule(rule.id);
      rules = rules.filter((candidate) => candidate.id !== rule.id);
      pendingDeleteId = "";
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to delete network access rule";
    } finally {
      deletingId = "";
    }
  }

  function compareRules(left: NetworkAccessRule, right: NetworkAccessRule) {
    return (
      Number(right.action === "deny") - Number(left.action === "deny") ||
      left.integration.localeCompare(right.integration) ||
      formatOrigin(left).localeCompare(formatOrigin(right))
    );
  }

  function formatOrigin(rule: NetworkAccessRule) {
    const host = rule.host.includes(":") ? `[${rule.host}]` : rule.host;
    const defaultPort = rule.scheme === "https" ? 443 : 80;
    const port = rule.port === defaultPort ? "" : `:${rule.port}`;
    return `${rule.scheme}://${host}${port}`;
  }

  function integrationLabel(value: NetworkAccessIntegration) {
    return (
      integrationOptions.find((option) => option.value === value)?.label ??
      value
    );
  }
</script>

<section
  class="authentication-policy network-access-policy"
  aria-labelledby="network-access-policy-title"
  data-od-id="network-access-policy"
>
  <div class="authentication-policy-heading">
    <div>
      <p class="widget-kicker">[ NETWORK ACCESS ]</p>
      <h3 id="network-access-policy-title">Server destinations</h3>
    </div>
    <span>{rules.length} / 128 rules</span>
  </div>

  <p class="network-access-intro">
    Public HTTPS remains the default. An allow rule can authorize one exact
    private or HTTP origin; a matching deny rule always wins. DNS is checked and
    pinned for each connection and every permitted podcast redirect.
  </p>

  <form class="network-access-form" onsubmit={addRule}>
    <label>
      <span>Decision</span>
      <select class="select-input" bind:value={action} data-od-id="network-rule-action">
        <option value="allow">Allow</option>
        <option value="deny">Deny</option>
      </select>
    </label>
    <label>
      <span>Applies to</span>
      <select
        class="select-input"
        bind:value={integration}
        data-od-id="network-rule-integration"
      >
        {#each integrationOptions as option (option.value)}
          <option value={option.value}>{option.label}</option>
        {/each}
      </select>
    </label>
    <label class="network-access-origin">
      <span>Exact origin</span>
      <input
        class="text-input"
        type="url"
        bind:value={origin}
        placeholder="http://192.168.1.20:3000"
        maxlength="2000"
        required
        autocomplete="off"
        spellcheck="false"
        data-od-id="network-rule-origin"
      />
    </label>
    <button
      class="ui-button ui-button--primary"
      type="submit"
      disabled={saving || !origin.trim() || rules.length >= 128}
      data-od-id="add-network-access-rule"
    >
      {saving ? "Adding…" : "Add rule"}
    </button>
  </form>

  <p class="network-access-note">
    These rules cover requests made by Pandan. Embedded custom pages and ordinary
    external links are loaded by the browser and are not evaluated here.
  </p>

  {#if error}
    <p class="form-error network-access-error" role="alert">{error}</p>
  {/if}

  {#if loading}
    <p class="network-access-empty" role="status">Loading network rules…</p>
  {:else if rules.length === 0}
    <p class="network-access-empty">No exceptions or explicit denials configured.</p>
  {:else}
    <div class="network-access-rules" data-od-id="network-access-rules">
      {#each rules as rule (rule.id)}
        <article data-od-id={`network-access-rule-${rule.id}`}>
          <span class={["network-rule-action", rule.action]}>{rule.action}</span>
          <div>
            <strong>{formatOrigin(rule)}</strong>
            <small>{integrationLabel(rule.integration)}</small>
          </div>
          <button
            class="ui-button ui-button--danger"
            type="button"
            disabled={deletingId !== ""}
            onclick={() => void removeRule(rule)}
            data-od-id={`remove-network-access-rule-${rule.id}`}
          >
            {deletingId === rule.id
              ? "Removing…"
              : pendingDeleteId === rule.id
                ? "Confirm"
                : "Remove"}
          </button>
        </article>
      {/each}
    </div>
  {/if}
</section>

<script lang="ts">
  import Check from "lucide-svelte/icons/check";
  import Download from "lucide-svelte/icons/download";
  import MonitorSmartphone from "lucide-svelte/icons/monitor-smartphone";
  import { pwa } from "$lib/pwa.svelte";
</script>

<section class="pwa-install-settings" data-od-id="pwa-install-settings">
  <div class="pwa-install-icon" aria-hidden="true">
    {#if pwa.installed}
      <Check size={19} strokeWidth={1.8} />
    {:else}
      <MonitorSmartphone size={19} strokeWidth={1.8} />
    {/if}
  </div>
  <div class="pwa-install-copy">
    <strong>{pwa.installed ? "Pandan is installed" : "Install Pandan"}</strong>
    <span>
      {pwa.installed
        ? "This device launches Pandan in its own application window."
        : pwa.installHint}
    </span>
  </div>
  {#if pwa.installAvailable && !pwa.installed}
    <button
      class="ui-button ui-button--secondary"
      type="button"
      disabled={pwa.installing}
      onclick={() => void pwa.install()}
      data-od-id="install-pandan"
    >
      <Download size={16} strokeWidth={1.8} aria-hidden="true" />
      {pwa.installing ? "Opening…" : "Install"}
    </button>
  {/if}
</section>

<style>
  .pwa-install-settings {
    min-width: 0;
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    margin-bottom: 22px;
    padding: 14px 0 22px;
    border-bottom: 1px solid var(--border);
  }

  .pwa-install-icon {
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    color: var(--accent);
  }

  .pwa-install-copy {
    min-width: 0;
    display: grid;
    gap: 4px;
  }

  .pwa-install-copy strong {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 620;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .pwa-install-copy span {
    max-width: 58ch;
    color: var(--muted);
    font-size: 10px;
    line-height: 1.5;
  }

  .pwa-install-settings button {
    gap: 7px;
    padding-inline: 12px;
  }

  @media (max-width: 560px) {
    .pwa-install-settings {
      grid-template-columns: 44px minmax(0, 1fr);
    }

    .pwa-install-settings button {
      grid-column: 1 / -1;
      width: 100%;
      justify-content: center;
    }
  }
</style>

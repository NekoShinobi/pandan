<script lang="ts">
  import RefreshCw from "lucide-svelte/icons/refresh-cw";
  import WifiOff from "lucide-svelte/icons/wifi-off";
  import { onMount } from "svelte";
  import { pwa } from "$lib/pwa.svelte";

  onMount(() => pwa.initialize());
</script>

{#if !pwa.online || pwa.updateAvailable}
  <aside
    class={["pwa-status", !pwa.online && "is-offline"]}
    role="status"
    aria-live="polite"
    data-od-id="pwa-status"
  >
    {#if !pwa.online}
      <span class="pwa-status-icon" aria-hidden="true">
        <WifiOff size={18} strokeWidth={1.8} />
      </span>
      <div>
        <strong>Server unavailable</strong>
        <span>Changes need a connection to your Pandan instance.</span>
      </div>
    {:else}
      <span class="pwa-status-icon" aria-hidden="true">
        <RefreshCw size={18} strokeWidth={1.8} />
      </span>
      <div>
        <strong>Update ready</strong>
        <span>Reload once to use the latest Pandan build.</span>
      </div>
      <button type="button" onclick={() => pwa.activateUpdate()}>Reload</button>
    {/if}
  </aside>
{/if}

<style>
  .pwa-status {
    --pwa-bg: oklch(11% 0.012 165);
    --pwa-surface: oklch(15% 0.014 165);
    --pwa-fg: oklch(91% 0.016 150);
    --pwa-muted: oklch(65% 0.02 155);
    --pwa-border: oklch(38% 0.025 155);
    --pwa-accent: oklch(79% 0.16 145);
    position: fixed;
    top: calc(max(12px, env(safe-area-inset-top)) + 76px);
    right: max(12px, env(safe-area-inset-right));
    z-index: 180;
    width: min(390px, calc(100vw - 24px));
    min-height: 64px;
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--pwa-border);
    background: color-mix(in oklch, var(--pwa-surface) 96%, var(--pwa-bg));
    color: var(--pwa-fg);
    box-shadow: 10px 10px 0 color-mix(in oklch, var(--pwa-bg) 72%, transparent);
    font-family: "JetBrains Mono", "SFMono-Regular", Consolas, monospace;
  }

  .pwa-status.is-offline {
    grid-template-columns: 24px minmax(0, 1fr);
  }

  .pwa-status-icon {
    display: grid;
    place-items: center;
    color: var(--pwa-accent);
  }

  .pwa-status > div {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .pwa-status strong {
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .pwa-status > div span {
    color: var(--pwa-muted);
    font-size: 10px;
    line-height: 1.45;
  }

  .pwa-status button {
    min-height: 44px;
    padding: 0 12px;
    border: 1px solid var(--pwa-accent);
    background: transparent;
    color: var(--pwa-accent);
    font: inherit;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .pwa-status button:hover {
    background: var(--pwa-accent);
    color: var(--pwa-bg);
  }

  .pwa-status button:focus-visible {
    outline: 3px solid var(--pwa-fg);
    outline-offset: 3px;
  }

  @media (max-width: 720px) {
    .pwa-status {
      top: calc(max(8px, env(safe-area-inset-top)) + 68px);
      right: max(10px, env(safe-area-inset-right));
      width: min(390px, calc(100vw - 20px));
    }
  }
</style>

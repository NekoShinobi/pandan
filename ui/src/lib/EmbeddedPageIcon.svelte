<script lang="ts">
  import PanelTop from "lucide-svelte/icons/panel-top";

  type Props = {
    iconUrl: string | null;
    size?: number;
  };

  let { iconUrl, size = 18 }: Props = $props();
  let failedUrl = $state("");
  let resolvedIconUrl = $derived(
    iconUrl && iconUrl !== failedUrl ? iconUrl : null,
  );

  function useFallback() {
    failedUrl = iconUrl ?? "";
  }
</script>

{#if resolvedIconUrl}
  <img
    class="embedded-page-icon-image"
    src={resolvedIconUrl}
    alt=""
    decoding="async"
    referrerpolicy="no-referrer"
    onerror={useFallback}
    aria-hidden="true"
  />
{:else}
  <PanelTop {size} strokeWidth={1.7} aria-hidden="true" />
{/if}

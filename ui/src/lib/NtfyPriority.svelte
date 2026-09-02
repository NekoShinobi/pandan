<script lang="ts">
  import ChevronDown from "lucide-svelte/icons/chevron-down";
  import ChevronUp from "lucide-svelte/icons/chevron-up";
  import ChevronsDown from "lucide-svelte/icons/chevrons-down";
  import ChevronsUp from "lucide-svelte/icons/chevrons-up";
  import Minus from "lucide-svelte/icons/minus";

  let { priority, ariaLabel }: { priority: number; ariaLabel?: string } =
    $props();

  const level = $derived(Math.min(5, Math.max(1, priority || 3)));
  const label = $derived(
    (
      {
        1: "Min",
        2: "Low",
        3: "Default",
        4: "High",
        5: "Max",
      } as Record<number, string>
    )[level],
  );
</script>

<span
  class="ntfy-priority"
  data-level={level}
  role="img"
  aria-label={ariaLabel ?? `${label} priority`}
>
  {#if level === 1}
    <ChevronsDown size={14} strokeWidth={1.8} aria-hidden="true" />
  {:else if level === 2}
    <ChevronDown size={14} strokeWidth={1.8} aria-hidden="true" />
  {:else if level === 4}
    <ChevronUp size={14} strokeWidth={1.8} aria-hidden="true" />
  {:else if level === 5}
    <ChevronsUp size={14} strokeWidth={1.8} aria-hidden="true" />
  {:else}
    <Minus size={14} strokeWidth={1.8} aria-hidden="true" />
  {/if}
</span>

<style>
  .ntfy-priority {
    display: inline-flex;
    width: 26px;
    height: 26px;
    flex: 0 0 26px;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    background: var(--fg-soft);
    color: var(--muted);
  }
  .ntfy-priority[data-level="1"] {
    border-color: color-mix(in oklch, var(--muted) 35%, var(--border));
    background: color-mix(in oklch, var(--muted) 5%, transparent);
  }
  .ntfy-priority[data-level="2"] {
    border-color: color-mix(in oklch, var(--muted) 55%, var(--border));
    background: color-mix(in oklch, var(--muted) 10%, transparent);
    color: color-mix(in oklch, var(--muted) 78%, var(--accent));
  }
  .ntfy-priority[data-level="3"] {
    border-color: color-mix(in oklch, var(--fg) 28%, var(--border));
    background: var(--fg-soft);
    color: var(--fg);
  }
  .ntfy-priority[data-level="4"] {
    border-color: color-mix(in oklch, var(--accent) 52%, var(--border));
    background: var(--accent-soft);
    color: var(--accent);
  }
  .ntfy-priority[data-level="5"] {
    border-color: color-mix(in oklch, var(--danger) 58%, var(--border));
    background: color-mix(in oklch, var(--danger) 14%, transparent);
    color: var(--danger);
  }
</style>

<script lang="ts">
  import { createSortable } from "@dnd-kit/svelte/sortable";
  import GripVertical from "lucide-svelte/icons/grip-vertical";
  import type { YoutubeGroup } from "$lib/api";

  let {
    active,
    disabled,
    group,
    index,
    onselect,
    reducedMotion,
  }: {
    active: boolean;
    disabled: boolean;
    group: YoutubeGroup;
    index: number;
    onselect: (groupId: string) => void;
    reducedMotion: boolean;
  } = $props();

  const sortable = createSortable({
    get id() {
      return group.id;
    },
    get index() {
      return index;
    },
    get disabled() {
      return disabled;
    },
    get transition() {
      return reducedMotion
        ? null
        : { duration: 150, easing: "cubic-bezier(0.2, 0, 0, 1)" };
    },
    group: "youtube-groups",
    type: "youtube-group",
    accept: "youtube-group",
  });
</script>

<div
  class={[
    "youtube-group-sortable",
    active && "active",
    sortable.isDragging && "is-dragging",
    sortable.isDropTarget && "is-drop-target",
  ]}
  data-od-id={`youtube-group-${group.id}`}
  {@attach sortable.attach}
>
  <button
    class="youtube-group-drag-handle"
    type="button"
    disabled={disabled}
    aria-label={`Drag ${group.name} category to reorder`}
    title={`Drag ${group.name} to reorder`}
    data-od-id={`youtube-reorder-group-${group.id}`}
    {@attach sortable.attachHandle}
  >
    <GripVertical size={11} strokeWidth={1.35} aria-hidden="true" />
  </button>
  <button
    class="youtube-group-select"
    type="button"
    aria-pressed={active}
    onclick={() => onselect(group.id)}
  >{group.name}</button>
</div>

<style>
  .youtube-group-sortable {
    flex: 0 0 auto;
    display: grid;
    grid-template-columns: 44px auto;
    min-height: 44px;
    border: 1px solid var(--border);
    border-radius: 0;
    background: var(--surface);
    color: var(--fg);
    transition:
      border-color 120ms var(--ease-out),
      background-color 120ms var(--ease-out),
      color 120ms var(--ease-out);
  }
  .youtube-group-sortable:hover,
  .youtube-group-sortable.active,
  .youtube-group-sortable.is-drop-target {
    border-color: var(--fg);
  }
  .youtube-group-sortable.active {
    background: var(--fg);
    color: var(--surface);
  }
  .youtube-group-sortable.is-dragging {
    opacity: 0.58;
  }
  button {
    min-height: 44px;
    border: 0;
    background: transparent;
    color: inherit;
    font-family: var(--font-mono);
    font-size: 10px;
  }
  button:focus-visible {
    position: relative;
    z-index: 1;
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }
  .active button:focus-visible {
    outline-color: var(--surface);
  }
  .youtube-group-drag-handle {
    width: 44px;
    display: grid;
    place-items: center;
    color: var(--muted);
    opacity: 0.42;
    cursor: grab;
    transition: opacity 100ms var(--ease-out);
  }
  .active .youtube-group-drag-handle {
    color: var(--surface);
  }
  .youtube-group-drag-handle:hover,
  .youtube-group-drag-handle:focus-visible {
    opacity: 0.82;
  }
  .youtube-group-drag-handle:active {
    opacity: 1;
    cursor: grabbing;
  }
  .youtube-group-drag-handle:disabled {
    opacity: 0.24;
    cursor: default;
  }
  .youtube-group-select {
    padding: 0 12px 0 10px;
  }
  @media (prefers-reduced-motion: reduce) {
    .youtube-group-sortable,
    .youtube-group-drag-handle {
      transition: none;
    }
  }
</style>

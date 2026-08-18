<script lang="ts">
  import { createDroppable } from "@dnd-kit/svelte";
  import type { Snippet } from "svelte";

  let {
    children,
    disabled,
    footer,
    header,
    id,
    label,
    odId,
  }: {
    children: Snippet;
    disabled: boolean;
    footer: Snippet;
    header: Snippet;
    id: string;
    label: string;
    odId: string;
  } = $props();

  const droppable = createDroppable({
    get id() {
      return id;
    },
    get disabled() {
      return disabled;
    },
    accept: "kanban-card",
    collisionPriority: -1,
  });
</script>

<section
  class={["kanban-column", droppable.isDropTarget && "is-drop-target"]}
  aria-label={label}
  data-od-id={odId}
  {@attach droppable.attach}
>
  {@render header()}
  <div class="kanban-card-list">
    {@render children()}
  </div>
  {@render footer()}
</section>

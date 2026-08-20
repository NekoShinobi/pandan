<script lang="ts">
  import { createDroppable } from "@dnd-kit/svelte";
  import { createSortable } from "@dnd-kit/svelte/sortable";
  import type { Snippet } from "svelte";
  import type { Attachment } from "svelte/attachments";

  const CARD_GROUP_PREFIX = "kanban-cards:";

  let {
    boardId,
    cardDropDisabled,
    children,
    columnDragDisabled,
    entering = false,
    footer,
    header,
    id,
    index,
    label,
    odId,
    reducedMotion,
  }: {
    boardId: string;
    cardDropDisabled: boolean;
    children: Snippet;
    columnDragDisabled: boolean;
    /** True for the one render after this column was created, to play its entrance. */
    entering?: boolean;
    footer: Snippet;
    header: Snippet<[Attachment<HTMLElement>]>;
    id: string;
    index: number;
    label: string;
    odId: string;
    reducedMotion: boolean;
  } = $props();

  const droppable = createDroppable({
    get id() {
      return `${CARD_GROUP_PREFIX}${id}`;
    },
    get disabled() {
      return cardDropDisabled;
    },
    accept: "kanban-card",
    collisionPriority: -1,
  });

  const sortable = createSortable({
    get id() {
      return id;
    },
    get index() {
      return index;
    },
    get group() {
      return boardId;
    },
    get disabled() {
      return columnDragDisabled;
    },
    get transition() {
      return reducedMotion
        ? null
        : { duration: 150, easing: "cubic-bezier(0.2, 0, 0, 1)" };
    },
    type: "kanban-column",
    accept: "kanban-column",
  });
</script>

<section
  class={[
    "kanban-column",
    droppable.isDropTarget && "is-drop-target",
    !columnDragDisabled && "is-sortable",
    sortable.isDragging && "is-column-dragging",
    sortable.isDropTarget && "is-column-drop-target",
    entering && "is-entering",
  ]}
  aria-label={label}
  data-od-id={odId}
  {@attach droppable.attach}
  {@attach sortable.attach}
>
  {@render header(sortable.attachHandle)}
  <div class="kanban-card-list">
    {@render children()}
  </div>
  {@render footer()}
</section>

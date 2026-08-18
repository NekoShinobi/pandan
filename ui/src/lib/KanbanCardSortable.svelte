<script lang="ts">
  import { createSortable } from "@dnd-kit/svelte/sortable";
  import CalendarClock from "lucide-svelte/icons/calendar-clock";
  import GripVertical from "lucide-svelte/icons/grip-vertical";
  import MessageSquare from "lucide-svelte/icons/message-square";
  import Paperclip from "lucide-svelte/icons/paperclip";
  import type { KanbanCard } from "$lib/api";

  let {
    card,
    columnId,
    disabled,
    index,
    reducedMotion,
    onopen,
  }: {
    card: KanbanCard;
    columnId: string;
    disabled: boolean;
    index: number;
    reducedMotion: boolean;
    onopen: (card: KanbanCard) => void;
  } = $props();

  const sortable = createSortable({
    get id() {
      return card.id;
    },
    get index() {
      return index;
    },
    get group() {
      return columnId;
    },
    get disabled() {
      return disabled;
    },
    get transition() {
      return reducedMotion
        ? null
        : { duration: 150, easing: "cubic-bezier(0.2, 0, 0, 1)" };
    },
    type: "kanban-card",
    accept: "kanban-card",
  });
</script>

<button
  class={[
    "kanban-card",
    !disabled && "is-draggable",
    sortable.isDragging && "is-dragging",
    sortable.isDropTarget && "is-drop-target",
  ]}
  type="button"
  aria-label={card.title}
  onclick={() => onopen(card)}
  data-od-id={`kanban-card-${card.id}`}
  {@attach sortable.attach}
>
  <span class="kanban-card-grip" aria-hidden="true"><GripVertical size={14} /></span>
  {#if card.labels.length}
    <span class="kanban-card-labels">
      {#each card.labels as label (label.id)}
        <i class={`is-${label.color}`} title={label.name}></i>
      {/each}
    </span>
  {/if}
  <strong>{card.title}</strong>
  {#if card.description}<p>{card.description}</p>{/if}
  <footer>
    {#if card.due_date}
      <span class:overdue={new Date(`${card.due_date}T23:59:59`) < new Date()}
        ><CalendarClock size={13} />{card.due_date}</span
      >
    {/if}
    {#if card.comments.length}
      <span><MessageSquare size={13} />{card.comments.length}</span>
    {/if}
    {#if card.attachments.length}
      <span><Paperclip size={13} />{card.attachments.length}</span>
    {/if}
    {#if card.assignees.length}
      <span class="kanban-avatar-stack">
        {#each card.assignees.slice(0, 3) as assignee (assignee.user_id)}
          <i title={assignee.display_name}
            >{assignee.display_name.slice(0, 1).toUpperCase()}</i
          >
        {/each}
      </span>
    {/if}
  </footer>
</button>

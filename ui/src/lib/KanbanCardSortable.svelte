<script lang="ts">
  import { createSortable } from "@dnd-kit/svelte/sortable";
  import CalendarClock from "lucide-svelte/icons/calendar-clock";
  import GripVertical from "lucide-svelte/icons/grip-vertical";
  import MessageSquare from "lucide-svelte/icons/message-square";
  import Paperclip from "lucide-svelte/icons/paperclip";
  import Pilcrow from "lucide-svelte/icons/pilcrow";
  import type { KanbanCard } from "$lib/api";

  const CARD_GROUP_PREFIX = "kanban-cards:";

  let {
    card,
    columnId,
    disabled,
    entering = false,
    index,
    reducedMotion,
    avatarUrl,
    onopen,
    oncontextmenu,
  }: {
    card: KanbanCard;
    columnId: string;
    disabled: boolean;
    /** True for the one render after this card was created, to play its entrance. */
    entering?: boolean;
    index: number;
    reducedMotion: boolean;
    /** Resolves a workspace member's avatar endpoint. */
    avatarUrl: (userId: string) => string;
    onopen: (card: KanbanCard) => void;
    oncontextmenu: (card: KanbanCard, event: MouseEvent) => void;
  } = $props();

  /** Members without an uploaded avatar answer 404, which uncovers the initial beneath. */
  function hideBrokenAvatar(event: Event) {
    if (event.currentTarget instanceof HTMLImageElement) {
      event.currentTarget.remove();
    }
  }

  const sortable = createSortable({
    get id() {
      return card.id;
    },
    get index() {
      return index;
    },
    get group() {
      return `${CARD_GROUP_PREFIX}${columnId}`;
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
    entering && "is-entering",
  ]}
  type="button"
  aria-label={card.description.trim()
    ? `${card.title}, has description`
    : card.title}
  onclick={() => onopen(card)}
  oncontextmenu={(event) => oncontextmenu(card, event)}
  onkeydown={(event) => {
    if (event.key !== "ContextMenu" && !(event.shiftKey && event.key === "F10")) {
      return;
    }
    event.preventDefault();
    const rect = event.currentTarget.getBoundingClientRect();
    oncontextmenu(
      card,
      new MouseEvent("contextmenu", {
        clientX: rect.left + Math.min(rect.width - 12, 36),
        clientY: rect.top + Math.min(rect.height - 12, 36),
      }),
    );
  }}
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
  <footer>
    {#if card.description.trim()}
      <span title="Has description" aria-hidden="true"><Pilcrow size={13} /></span>
    {/if}
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
          <i class="kanban-member-avatar" title={assignee.display_name}>
            <span aria-hidden="true"
              >{assignee.display_name.slice(0, 1).toUpperCase()}</span
            >
            <img
              src={avatarUrl(assignee.user_id)}
              alt=""
              onerror={hideBrokenAvatar}
            />
          </i>
        {/each}
      </span>
    {/if}
  </footer>
</button>

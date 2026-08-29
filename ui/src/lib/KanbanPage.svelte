<script lang="ts">
  import DOMPurify from "dompurify";
  import { move } from "@dnd-kit/helpers";
  import {
    DragDropProvider,
    KeyboardSensor,
    PointerSensor,
    type DragDropEventHandlers,
  } from "@dnd-kit/svelte";
  import { marked } from "marked";
  import Archive from "lucide-svelte/icons/archive";
  import Check from "lucide-svelte/icons/check";
  import ChevronLeft from "lucide-svelte/icons/chevron-left";
  import Copy from "lucide-svelte/icons/copy";
  import EllipsisVertical from "lucide-svelte/icons/ellipsis-vertical";
  import GripVertical from "lucide-svelte/icons/grip-vertical";
  import Paperclip from "lucide-svelte/icons/paperclip";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import Search from "lucide-svelte/icons/search";
  import Star from "lucide-svelte/icons/star";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import Users from "lucide-svelte/icons/users";
  import X from "lucide-svelte/icons/x";
  import { onDestroy, onMount, untrack } from "svelte";
  import { MediaQuery } from "svelte/reactivity";
  import PandanDatePicker from "$lib/components/PandanDatePicker.svelte";
  import { createViewSwap } from "$lib/viewSwap.svelte";
  import KanbanCardSortable from "$lib/KanbanCardSortable.svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import KanbanColumnDropzone from "$lib/KanbanColumnDropzone.svelte";
  import {
    archiveKanbanCard,
    createKanbanBoard,
    createKanbanCard,
    createKanbanChecklist,
    createKanbanChecklistItem,
    createKanbanComment,
    createKanbanColumn,
    createKanbanLabel,
    createKanbanWorkspace,
    deleteKanbanAttachment,
    deleteKanbanChecklist,
    deleteKanbanColumn,
    deleteKanbanComment,
    deleteKanbanWorkspace,
    fetchKanbanBoard,
    fetchKanbanBoards,
    fetchKanbanCard,
    fetchKanbanOverview,
    fetchKanbanWorkspaceSettings,
    inviteKanbanMember,
    kanbanAttachmentUrl,
    moveKanbanCard,
    removeKanbanMember,
    respondKanbanInvitation,
    searchKanbanDirectory,
    setKanbanBoardFavorite,
    setKanbanRolePermission,
    updateKanbanBoard,
    updateKanbanCard,
    updateKanbanColumn,
    updateKanbanChecklistItem,
    updateKanbanMemberRole,
    uploadKanbanAttachment,
    type KanbanBoard,
    type KanbanBoardSummary,
    type KanbanCard,
    type KanbanColumn,
    type KanbanDirectoryUser,
    type KanbanLabelColor,
    type KanbanOverview,
    type KanbanRole,
    type KanbanSection,
    type KanbanWorkspaceSettings,
  } from "$lib/api";

  let { section, viewerId }: { section: KanbanSection; viewerId: string } =
    $props();

  const permissionGroups = [
    { name: "Workspace", permissions: ["workspace:view", "workspace:edit"] },
    {
      name: "Boards",
      permissions: ["board:view", "board:create", "board:edit", "board:delete"],
    },
    {
      name: "Columns",
      permissions: ["list:view", "list:create", "list:edit", "list:delete"],
    },
    {
      name: "Cards",
      permissions: ["card:view", "card:create", "card:edit", "card:delete"],
    },
    {
      name: "Comments",
      permissions: [
        "comment:view",
        "comment:create",
        "comment:edit",
        "comment:delete",
      ],
    },
    {
      name: "Members",
      permissions: [
        "member:view",
        "member:invite",
        "member:edit",
        "member:remove",
      ],
    },
  ];
  const labelColors: KanbanLabelColor[] = [
    "accent",
    "blue",
    "amber",
    "red",
    "violet",
    "gray",
  ];
  const configurableRoles: Array<"member" | "guest"> = ["member", "guest"];
  const kanbanSensors = [
    PointerSensor,
    KeyboardSensor.configure({
      keyboardCodes: {
        start: ["Space"],
        cancel: ["Escape"],
        end: ["Space", "Tab"],
        up: ["ArrowUp"],
        down: ["ArrowDown"],
        left: ["ArrowLeft"],
        right: ["ArrowRight"],
      },
    }),
  ];
  type KanbanDragHandlers = DragDropEventHandlers;
  type KanbanDragStartEvent = Parameters<
    NonNullable<KanbanDragHandlers["onDragStart"]>
  >[0];
  type KanbanDragOverEvent = Parameters<
    NonNullable<KanbanDragHandlers["onDragOver"]>
  >[0];
  type KanbanDragEndEvent = Parameters<
    NonNullable<KanbanDragHandlers["onDragEnd"]>
  >[0];
  type CardDraft = {
    title: string;
    description: string;
    due_date: string | null;
    assignee_ids: string[];
    label_ids: string[];
  };

  const kanbanSections: KanbanSection[] = [
    "boards",
    "workspaces",
    "invitations",
  ];
  const CARD_GROUP_PREFIX = "kanban-cards:";
  const viewSwap = createViewSwap();
  // Rendering lags the `section` prop by one leave animation so the outgoing
  // view can fade before the sidebar's choice takes over.
  let displayedSection = $state<KanbanSection>(untrack(() => section));

  let overview = $state.raw<KanbanOverview>({
    workspaces: [],
    invitations: [],
  });
  let selectedWorkspaceId = $state("");
  let boards = $state.raw<KanbanBoardSummary[]>([]);
  let board = $state.raw<KanbanBoard | null>(null);
  let settings = $state.raw<KanbanWorkspaceSettings | null>(null);
  let directoryResults = $state.raw<KanbanDirectoryUser[]>([]);
  let selectedCard = $state.raw<KanbanCard | null>(null);
  let loading = $state(true);
  let busy = $state(false);
  let cardSaving = $state(false);
  let error = $state("");
  let boardSearch = $state("");
  let archivedBoards = $state(false);
  let cardSearch = $state("");
  let assigneeFilter = $state("");
  let labelFilter = $state("");
  let dueFilter = $state<"all" | "overdue" | "week" | "none">("all");
  let addCardColumnId = $state("");
  let addCardTitle = $state("");
  let openColumnMenuId = $state("");
  let newColumnName = $state("");
  /** Deleting a column is confirmed by pressing its control a second time. */
  let pendingColumnDelete = $state("");
  /**
   * The records the reader just created. They carry a one-shot entrance so a new card or
   * column is visibly the thing that arrived, rather than appearing between a refetch.
   */
  let enteringCardId = $state("");
  let enteringColumnId = $state("");
  let enteringTimer: ReturnType<typeof setTimeout> | undefined;

  let workspaceDialog = $state<HTMLDialogElement>();
  let boardDialog = $state<HTMLDialogElement>();
  let archiveBoardDialog = $state<HTMLDialogElement>();
  let columnDialog = $state<HTMLDialogElement>();
  let columnNameInput = $state<HTMLInputElement>();
  let addCardDialog = $state<HTMLDialogElement>();
  let addCardTitleInput = $state<HTMLInputElement>();
  let cardContextDialog = $state<HTMLDialogElement>();
  let contextCard = $state.raw<KanbanCard | null>(null);
  let contextDialogX = $state(0);
  let contextDialogY = $state(0);
  let pendingCardDelete = $state(false);
  let cardDialog = $state<HTMLDialogElement>();
  let workspaceName = $state("");
  let workspaceDescription = $state("");
  let boardName = $state("");
  let boardDescription = $state("");
  let boardVisibility = $state<"private" | "public">("private");
  let boardDialogMode = $state<"create" | "edit">("create");
  let cardTitle = $state("");
  let cardDescription = $state("");
  let cardDescriptionMode = $state<"edit" | "preview">("preview");
  let cardDueDate = $state("");
  let cardAssigneeIds = $state.raw<string[]>([]);
  let cardLabelIds = $state.raw<string[]>([]);
  let commentDraft = $state("");
  let checklistDraft = $state("");
  let checklistItemDraft = $state<Record<string, string>>({});
  let labelName = $state("");
  let labelColor = $state<KanbanLabelColor>("accent");
  let memberQuery = $state("");
  let inviteRole = $state<KanbanRole>("member");
  let cardDragSnapshot: Record<string, KanbanCard[]> | null = null;
  let columnDragSnapshot: KanbanColumn[] | null = null;
  let lastSavedCardSignature = "";
  let cardSavePromise: Promise<void> | null = null;

  let activeWorkspace = $derived(
    overview.workspaces.find(
      (workspace) => workspace.id === selectedWorkspaceId,
    ) ?? null,
  );
  let canCreateBoard = $derived(
    activeWorkspace?.permissions.includes("board:create") ?? false,
  );
  let canEditBoard = $derived(
    board?.permissions.includes("board:edit") ?? false,
  );
  let canCreateColumn = $derived(
    board?.permissions.includes("list:create") ?? false,
  );
  let canEditColumn = $derived(
    board?.permissions.includes("list:edit") ?? false,
  );
  let canDeleteColumn = $derived(
    board?.permissions.includes("list:delete") ?? false,
  );
  let canCreateCard = $derived(
    board?.permissions.includes("card:create") ?? false,
  );
  let canEditCard = $derived(board?.permissions.includes("card:edit") ?? false);
  let canDeleteCard = $derived(
    board?.permissions.includes("card:delete") ?? false,
  );
  let addCardColumn = $derived(
    board?.columns.find((column) => column.id === addCardColumnId) ?? null,
  );
  let cardFiltersActive = $derived(
    Boolean(
      cardSearch.trim() || assigneeFilter || labelFilter || dueFilter !== "all",
    ),
  );
  const reducedMotion = new MediaQuery("(prefers-reduced-motion: reduce)");
  let filteredBoards = $derived(
    boards.filter((item) => {
      const query = boardSearch.trim().toLowerCase();
      return (
        !query ||
        `${item.name} ${item.description}`.toLowerCase().includes(query)
      );
    }),
  );
  function sanitizedMarkdown(markdown: string) {
    const parsed = marked.parse(markdown, { async: false, breaks: true });
    return DOMPurify.sanitize(String(parsed), { USE_PROFILES: { html: true } });
  }

  let renderedCardDescription = $derived(sanitizedMarkdown(cardDescription));
  let renderedBoardDescription = $derived(
    sanitizedMarkdown(board?.description ?? ""),
  );

  function renderSanitizedMarkdown(html: string) {
    return (node: HTMLElement) => {
      node.innerHTML = html;
      return () => node.replaceChildren();
    };
  }

  function kanbanMemberAvatarUrl(userId: string) {
    return `/api/kanban/workspaces/${encodeURIComponent(selectedWorkspaceId)}/members/${encodeURIComponent(userId)}/avatar`;
  }

  function hideBrokenAvatar(event: Event) {
    if (event.currentTarget instanceof HTMLImageElement) {
      event.currentTarget.remove();
    }
  }

  function focusDescriptionEditor(node: HTMLTextAreaElement) {
    node.focus();
  }

  onMount(() => {
    void loadOverview();
  });

  onDestroy(() => {
    viewSwap.cancel();
  });

  $effect(() => {
    if (!selectedWorkspaceId) return;
    if (section === "boards") void loadBoards();
    if (section === "workspaces") void loadSettings();
  });

  $effect(() => {
    const next = section;
    untrack(() => {
      if (next === displayedSection) return;
      void viewSwap.run({
        forward:
          kanbanSections.indexOf(next) >
          kanbanSections.indexOf(displayedSection),
        commit: () => {
          displayedSection = next;
        },
      });
    });
  });

  async function run(action: () => Promise<void>) {
    busy = true;
    error = "";
    try {
      await action();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Kanban request failed";
    } finally {
      busy = false;
    }
  }

  async function loadOverview() {
    loading = true;
    await run(async () => {
      overview = await fetchKanbanOverview();
      if (
        !overview.workspaces.some(
          (workspace) => workspace.id === selectedWorkspaceId,
        )
      ) {
        selectedWorkspaceId = overview.workspaces[0]?.id ?? "";
      }
    });
    loading = false;
  }

  async function loadBoards() {
    if (!selectedWorkspaceId) return;
    boards = await fetchKanbanBoards(selectedWorkspaceId, archivedBoards);
  }

  async function toggleArchivedBoards() {
    archivedBoards = !archivedBoards;
    await run(loadBoards);
  }

  async function loadSettings() {
    if (!selectedWorkspaceId) return;
    settings = await fetchKanbanWorkspaceSettings(selectedWorkspaceId);
  }

  async function selectWorkspace(id: string) {
    // Scoping the page to another workspace is a filter, not a navigation, and
    // the switcher itself sits inside the swapping region — leave it unanimated
    // rather than fading the control out from under the pointer.
    selectedWorkspaceId = id;
    board = null;
    settings = null;
    await run(section === "workspaces" ? loadSettings : loadBoards);
  }

  async function submitWorkspace(event: SubmitEvent) {
    event.preventDefault();
    await run(async () => {
      const workspace = await createKanbanWorkspace({
        name: workspaceName,
        description: workspaceDescription,
      });
      workspaceDialog?.close();
      workspaceName = "";
      workspaceDescription = "";
      await loadOverview();
      await selectWorkspace(workspace.id);
    });
  }

  async function submitBoard(event: SubmitEvent) {
    event.preventDefault();
    if (boardDialogMode === "edit") {
      if (!board || !canEditBoard) return;
      const boardId = board.id;
      const archived = board.archived;
      await run(async () => {
        await updateKanbanBoard(boardId, {
          name: boardName,
          description: boardDescription,
          visibility: boardVisibility,
          archived,
        });
        boardDialog?.close();
        board = await fetchKanbanBoard(boardId);
        await loadBoards();
      });
      return;
    }

    if (!selectedWorkspaceId) return;
    await run(async () => {
      const created = await createKanbanBoard(selectedWorkspaceId, {
        name: boardName,
        description: boardDescription,
        visibility: boardVisibility,
      });
      boardDialog?.close();
      boardName = "";
      boardDescription = "";
      board = created;
      await loadBoards();
    });
  }

  function openCreateBoardDialog() {
    boardDialogMode = "create";
    boardName = "";
    boardDescription = "";
    boardVisibility = "private";
    boardDialog?.showModal();
  }

  function openEditBoardDialog() {
    if (!board || !canEditBoard) return;
    boardDialogMode = "edit";
    boardName = board.name;
    boardDescription = board.description;
    boardVisibility = board.visibility;
    boardDialog?.showModal();
  }

  async function openBoard(id: string) {
    closeAddColumn();
    pendingColumnDelete = "";
    let opened: KanbanBoard | null = null;
    const pending = run(async () => {
      opened = await fetchKanbanBoard(id);
    });
    await viewSwap.run({
      forward: true,
      pending,
      commit: () => {
        if (opened) board = opened;
      },
    });
    // A board slower than the leave animation still opens, just without one.
    await pending;
    if (opened && board !== opened) board = opened;
  }

  async function closeBoard() {
    await viewSwap.run({
      forward: false,
      commit: () => {
        board = null;
      },
    });
    await run(loadBoards);
  }

  async function refreshBoard() {
    if (board) board = await fetchKanbanBoard(board.id);
  }

  onDestroy(() => {
    clearTimeout(enteringTimer);
  });

  function openAddColumn() {
    newColumnName = "";
    columnDialog?.showModal();
    columnNameInput?.focus();
  }

  function closeAddColumn() {
    columnDialog?.close();
  }

  function resetAddColumn() {
    newColumnName = "";
  }

  /**
   * Marks a freshly created record so its entrance plays once. Clearing the flag keeps
   * the animation off the same row when the board is refetched later.
   */
  function markEntering(card: string, column: string) {
    clearTimeout(enteringTimer);
    enteringCardId = card;
    enteringColumnId = column;
    enteringTimer = setTimeout(() => {
      enteringCardId = "";
      enteringColumnId = "";
    }, 600);
  }

  async function submitColumn(event: SubmitEvent) {
    event.preventDefault();
    if (!board || !newColumnName.trim()) return;
    await run(async () => {
      const created = await createKanbanColumn(board!.id, newColumnName);
      closeAddColumn();
      await refreshBoard();
      markEntering("", created.id);
    });
  }

  /**
   * Removes a column. The server refuses while it still holds active cards and says so,
   * which is the message the board surfaces rather than a guess made here.
   */
  async function removeColumn(column: KanbanColumn) {
    if (pendingColumnDelete !== column.id) {
      pendingColumnDelete = column.id;
      return;
    }
    pendingColumnDelete = "";
    openColumnMenuId = "";
    await run(async () => {
      await deleteKanbanColumn(column.id);
      await refreshBoard();
    });
  }

  async function toggleFavorite(item: KanbanBoardSummary) {
    await run(async () => {
      await setKanbanBoardFavorite(item.id, !item.favorite);
      await loadBoards();
    });
  }

  async function setBoardArchived(
    archived: boolean,
    dialog?: HTMLDialogElement,
  ) {
    if (!board) return;
    await run(async () => {
      await updateKanbanBoard(board!.id, {
        name: board!.name,
        description: board!.description,
        visibility: board!.visibility,
        archived,
      });
      dialog?.close();
      board = null;
      await loadBoards();
    });
  }

  function requestBoardArchive() {
    if (!board) return;
    if (board.archived) {
      void setBoardArchived(false);
      return;
    }
    archiveBoardDialog?.showModal();
  }

  async function confirmBoardArchive(event: SubmitEvent) {
    event.preventDefault();
    await setBoardArchived(true, archiveBoardDialog);
  }

  async function submitCard(event: SubmitEvent) {
    event.preventDefault();
    if (!addCardColumnId || !addCardTitle.trim()) return;
    const columnId = addCardColumnId;
    await run(async () => {
      const created = await createKanbanCard(columnId, {
        title: addCardTitle,
      });
      closeAddCard();
      await refreshBoard();
      markEntering(created.id, "");
    });
  }

  function openAddCard(columnId: string) {
    addCardColumnId = columnId;
    addCardTitle = "";
    openColumnMenuId = "";
    pendingColumnDelete = "";
    addCardDialog?.showModal();
    queueMicrotask(() => addCardTitleInput?.focus());
  }

  function closeAddCard() {
    addCardDialog?.close();
  }

  function resetAddCard() {
    addCardColumnId = "";
    addCardTitle = "";
  }

  function toggleColumnMenu(columnId: string) {
    const opening = openColumnMenuId !== columnId;
    openColumnMenuId = opening ? columnId : "";
    pendingColumnDelete = "";
  }

  function closeColumnMenu() {
    openColumnMenuId = "";
    pendingColumnDelete = "";
  }

  function handleColumnMenuDocumentClick(event: MouseEvent) {
    const target = event.target;
    if (!(target instanceof Element)) return;
    if (target.closest(".kanban-column-menu")) return;
    closeColumnMenu();
  }

  function handleColumnMenuDocumentKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") closeColumnMenu();
  }

  function openCardContext(card: KanbanCard, event: MouseEvent) {
    event.preventDefault();
    if (cardContextDialog?.open) {
      closeCardContext();
      return;
    }
    if (!canEditCard && !canCreateCard && !canDeleteCard) return;
    openColumnMenuId = "";
    pendingCardDelete = false;
    contextCard = card;
    const menuWidth = 260;
    const menuHeight = 156;
    const inset = 12;
    contextDialogX = Math.max(
      inset,
      Math.min(event.clientX, window.innerWidth - menuWidth - inset),
    );
    contextDialogY = Math.max(
      inset,
      Math.min(event.clientY, window.innerHeight - menuHeight - inset),
    );
    cardContextDialog?.showModal();
  }

  function closeCardContext() {
    cardContextDialog?.close();
  }

  function resetCardContext() {
    contextCard = null;
    pendingCardDelete = false;
  }

  async function editContextCard() {
    const card = contextCard;
    if (!card) return;
    closeCardContext();
    await openCard(card);
  }

  async function duplicateContextCard() {
    const card = contextCard;
    if (!card || !canCreateCard) return;
    closeCardContext();
    await run(async () => {
      const source = await fetchKanbanCard(card.id);
      const created = await createKanbanCard(source.column_id, {
        title: `${source.title} (copy)`,
        description: source.description,
        due_date: source.due_date,
        assignee_ids: source.assignees.map((member) => member.user_id),
        label_ids: source.labels.map((label) => label.id),
      });
      await refreshBoard();
      markEntering(created.id, "");
    });
  }

  async function deleteContextCard() {
    const card = contextCard;
    if (!card || !canDeleteCard) return;
    if (!pendingCardDelete) {
      pendingCardDelete = true;
      return;
    }
    closeCardContext();
    await run(async () => {
      await archiveKanbanCard(card.id);
      await refreshBoard();
    });
  }

  async function openCard(card: KanbanCard) {
    await run(async () => {
      selectedCard = await fetchKanbanCard(card.id);
      cardTitle = selectedCard.title;
      cardDescription = selectedCard.description;
      cardDescriptionMode = "preview";
      cardDueDate = selectedCard.due_date ?? "";
      cardAssigneeIds = selectedCard.assignees.map((member) => member.user_id);
      cardLabelIds = selectedCard.labels.map((label) => label.id);
      lastSavedCardSignature = cardDraftSignature(currentCardDraft());
      cardDialog?.showModal();
    });
  }

  function currentCardDraft(): CardDraft {
    return {
      title: cardTitle,
      description: cardDescription,
      due_date: cardDueDate || null,
      assignee_ids: [...cardAssigneeIds].sort(),
      label_ids: [...cardLabelIds].sort(),
    };
  }

  function cardDraftSignature(draft: CardDraft) {
    return JSON.stringify(draft);
  }

  function saveCard(): Promise<void> {
    if (!selectedCard || !canEditCard) return Promise.resolve();
    if (cardSavePromise) return cardSavePromise;
    if (cardDraftSignature(currentCardDraft()) === lastSavedCardSignature) {
      return Promise.resolve();
    }

    const cardId = selectedCard.id;
    cardSavePromise = (async () => {
      cardSaving = true;
      error = "";
      try {
        while (selectedCard?.id === cardId) {
          const draft = currentCardDraft();
          const signature = cardDraftSignature(draft);
          if (signature === lastSavedCardSignature) break;
          selectedCard = await updateKanbanCard(cardId, draft);
          lastSavedCardSignature = signature;
          await refreshBoard();
        }
      } catch (cause) {
        error =
          cause instanceof Error ? cause.message : "Kanban request failed";
      } finally {
        cardSaving = false;
        cardSavePromise = null;
      }
    })();
    return cardSavePromise;
  }

  function startDescriptionEditing() {
    if (canEditCard) cardDescriptionMode = "edit";
  }

  function handleDescriptionSurfaceKeydown(event: KeyboardEvent) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    startDescriptionEditing();
  }

  function finishDescriptionEditing(
    event: FocusEvent & { currentTarget: HTMLDivElement },
  ) {
    const nextTarget = event.relatedTarget;
    if (nextTarget instanceof Node && event.currentTarget.contains(nextTarget))
      return;
    cardDescriptionMode = "preview";
    void saveCard();
  }

  function toggleCardAssignee(userId: string) {
    cardAssigneeIds = toggleValue(cardAssigneeIds, userId);
    void saveCard();
  }

  function toggleCardLabel(labelId: string) {
    cardLabelIds = toggleValue(cardLabelIds, labelId);
    void saveCard();
  }

  async function closeCardDialog() {
    cardDescriptionMode = "preview";
    await saveCard();
    cardDialog?.close();
  }

  async function archiveCard() {
    if (!selectedCard) return;
    await saveCard();
    await run(async () => {
      await archiveKanbanCard(selectedCard!.id);
      cardDialog?.close();
      selectedCard = null;
      await refreshBoard();
    });
  }

  function cardGroups() {
    if (!board) return {};
    return Object.fromEntries(
      board.columns.map((column) => [
        `${CARD_GROUP_PREFIX}${column.id}`,
        column.cards.slice(),
      ]),
    );
  }

  function applyCardGroups(groups: Record<string, KanbanCard[]>) {
    if (!board) return;
    board = {
      ...board,
      columns: board.columns.map((column) =>
        groups[`${CARD_GROUP_PREFIX}${column.id}`]
          ? {
              ...column,
              cards: groups[`${CARD_GROUP_PREFIX}${column.id}`],
            }
          : column,
      ),
    };
  }

  function cardLocation(groups: Record<string, KanbanCard[]>, cardId: string) {
    for (const [groupId, cards] of Object.entries(groups)) {
      const position = cards.findIndex((card) => card.id === cardId);
      if (position >= 0) {
        return {
          columnId: groupId.slice(CARD_GROUP_PREFIX.length),
          position,
        };
      }
    }
    return null;
  }

  function applyColumns(columns: KanbanColumn[]) {
    if (!board) return;
    board = {
      ...board,
      columns: columns.map((column, position) => ({ ...column, position })),
    };
  }

  function startKanbanDrag(event: KanbanDragStartEvent) {
    const sourceType = event.operation.source?.type;
    if (!board) return;
    if (sourceType === "kanban-card") {
      cardDragSnapshot = cardGroups();
    } else if (sourceType === "kanban-column" && canEditColumn) {
      columnDragSnapshot = board.columns.slice();
    }
  }

  function previewKanbanMove(event: KanbanDragOverEvent) {
    if (!board) return;
    const sourceType = event.operation.source?.type;
    if (sourceType === "kanban-card") {
      if (cardFiltersActive || !canEditCard) return;
      applyCardGroups(move(cardGroups(), event));
    } else if (sourceType === "kanban-column" && canEditColumn) {
      applyColumns(move(board.columns, event));
    }
  }

  async function finishKanbanDrag(event: KanbanDragEndEvent) {
    const source = event.operation.source;
    if (!board || !source) return;

    if (source.type === "kanban-column") {
      if (event.canceled || !event.operation.target) {
        if (columnDragSnapshot) applyColumns(columnDragSnapshot);
        columnDragSnapshot = null;
        return;
      }

      const nextColumns = move(board.columns, event);
      applyColumns(nextColumns);
      const columnId = String(source.id);
      const nextPosition = nextColumns.findIndex(
        (column) => column.id === columnId,
      );
      const previousPosition =
        columnDragSnapshot?.findIndex((column) => column.id === columnId) ?? -1;
      columnDragSnapshot = null;
      if (nextPosition < 0 || nextPosition === previousPosition) return;

      await run(async () => {
        try {
          await updateKanbanColumn(columnId, { position: nextPosition });
        } finally {
          await refreshBoard();
        }
      });
      return;
    }

    if (source.type !== "kanban-card") return;

    if (event.canceled || !event.operation.target) {
      if (cardDragSnapshot) applyCardGroups(cardDragSnapshot);
      cardDragSnapshot = null;
      return;
    }

    const nextGroups = move(cardGroups(), event);
    applyCardGroups(nextGroups);
    const cardId = String(source.id);
    const nextLocation = cardLocation(nextGroups, cardId);
    const previousLocation = cardDragSnapshot
      ? cardLocation(cardDragSnapshot, cardId)
      : null;
    cardDragSnapshot = null;

    if (
      !nextLocation ||
      (previousLocation?.columnId === nextLocation.columnId &&
        previousLocation.position === nextLocation.position)
    )
      return;

    await run(async () => {
      try {
        await moveKanbanCard(
          cardId,
          nextLocation.columnId,
          nextLocation.position,
        );
      } finally {
        await refreshBoard();
      }
    });
  }

  function visibleCards(cards: KanbanCard[]) {
    const now = new Date();
    const week = new Date(now);
    week.setDate(now.getDate() + 7);
    return cards.filter((card) => {
      const query = cardSearch.trim().toLowerCase();
      if (
        query &&
        !`${card.title} ${card.description}`.toLowerCase().includes(query)
      )
        return false;
      if (
        assigneeFilter &&
        !card.assignees.some((member) => member.user_id === assigneeFilter)
      )
        return false;
      if (labelFilter && !card.labels.some((label) => label.id === labelFilter))
        return false;
      if (dueFilter === "none") return !card.due_date;
      if (dueFilter === "overdue")
        return !!card.due_date && new Date(`${card.due_date}T23:59:59`) < now;
      if (dueFilter === "week")
        return !!card.due_date && new Date(`${card.due_date}T23:59:59`) <= week;
      return true;
    });
  }

  function toggleValue(values: string[], value: string) {
    return values.includes(value)
      ? values.filter((item) => item !== value)
      : [...values, value];
  }

  async function addLabel() {
    if (!board || !labelName.trim()) return;
    await run(async () => {
      const label = await createKanbanLabel(board!.id, labelName, labelColor);
      labelName = "";
      cardLabelIds = [...cardLabelIds, label.id];
      await refreshBoard();
    });
    await saveCard();
  }

  async function addComment() {
    if (!selectedCard || !commentDraft.trim()) return;
    await run(async () => {
      await createKanbanComment(selectedCard!.id, commentDraft);
      commentDraft = "";
      selectedCard = await fetchKanbanCard(selectedCard!.id);
    });
  }

  async function removeComment(id: string) {
    if (!selectedCard) return;
    await run(async () => {
      await deleteKanbanComment(id);
      selectedCard = await fetchKanbanCard(selectedCard!.id);
    });
  }

  async function addChecklist() {
    if (!selectedCard || !checklistDraft.trim()) return;
    await run(async () => {
      await createKanbanChecklist(selectedCard!.id, checklistDraft);
      checklistDraft = "";
      selectedCard = await fetchKanbanCard(selectedCard!.id);
    });
  }

  async function addChecklistItem(checklistId: string) {
    const title = checklistItemDraft[checklistId]?.trim();
    if (!selectedCard || !title) return;
    await run(async () => {
      await createKanbanChecklistItem(checklistId, title);
      checklistItemDraft = { ...checklistItemDraft, [checklistId]: "" };
      selectedCard = await fetchKanbanCard(selectedCard!.id);
    });
  }

  async function toggleChecklistItem(
    checklistId: string,
    itemId: string,
    title: string,
    completed: boolean,
  ) {
    if (!selectedCard) return;
    await run(async () => {
      await updateKanbanChecklistItem(checklistId, itemId, title, completed);
      selectedCard = await fetchKanbanCard(selectedCard!.id);
    });
  }

  async function removeChecklist(id: string) {
    if (!selectedCard) return;
    await run(async () => {
      await deleteKanbanChecklist(id);
      selectedCard = await fetchKanbanCard(selectedCard!.id);
    });
  }

  async function uploadAttachment(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file || !selectedCard) return;
    await run(async () => {
      await uploadKanbanAttachment(selectedCard!.id, file);
      selectedCard = await fetchKanbanCard(selectedCard!.id);
      input.value = "";
    });
  }

  async function removeAttachment(id: string) {
    if (!selectedCard) return;
    await run(async () => {
      await deleteKanbanAttachment(id);
      selectedCard = await fetchKanbanCard(selectedCard!.id);
    });
  }

  async function findMembers() {
    if (!selectedWorkspaceId || memberQuery.trim().length < 2) {
      directoryResults = [];
      return;
    }
    await run(async () => {
      directoryResults = await searchKanbanDirectory(
        selectedWorkspaceId,
        memberQuery,
      );
    });
  }

  async function invite(userId: string) {
    await run(async () => {
      await inviteKanbanMember(selectedWorkspaceId, userId, inviteRole);
      directoryResults = [];
      memberQuery = "";
      await loadSettings();
    });
  }

  async function changeRole(userId: string, role: KanbanRole) {
    await run(async () => {
      await updateKanbanMemberRole(selectedWorkspaceId, userId, role);
      await loadSettings();
      await loadOverview();
    });
  }

  function changeRoleFromSelect(userId: string, event: Event) {
    const role = (event.currentTarget as HTMLSelectElement).value;
    if (role === "admin" || role === "member" || role === "guest") {
      void changeRole(userId, role);
    }
  }

  async function removeMember(userId: string) {
    await run(async () => {
      await removeKanbanMember(selectedWorkspaceId, userId);
      await loadSettings();
      await loadOverview();
    });
  }

  function roleGrant(role: "member" | "guest", permission: string) {
    return (
      settings?.role_permissions.find(
        (grant) => grant.role === role && grant.permission === permission,
      )?.granted ?? false
    );
  }

  async function toggleRoleGrant(
    role: "member" | "guest",
    permission: string,
    granted: boolean,
  ) {
    await run(async () => {
      await setKanbanRolePermission(
        selectedWorkspaceId,
        role,
        permission,
        granted,
      );
      await loadSettings();
    });
  }

  async function answerInvitation(workspaceId: string, accept: boolean) {
    await run(async () => {
      await respondKanbanInvitation(workspaceId, accept);
      await loadOverview();
    });
  }
</script>

<svelte:document
  onclick={handleColumnMenuDocumentClick}
  onkeydown={handleColumnMenuDocumentKeydown}
/>

<section class="kanban-page product-page" data-od-id="kanban-page">
  <header class="kanban-page-header page-header" data-od-id="kanban-header">
    <div>
      <TypedHeading
        text={`$ kanban --${displayedSection}`}
        odId="kanban-heading"
      />
      {#key displayedSection}
        <p class="view-swap-copy">
          {displayedSection === "boards"
            ? "Move work from intent to finished."
            : displayedSection === "workspaces"
              ? "Members, roles, and workspace rules."
              : "Join shared workspaces from people already on Pandan."}
        </p>
      {/key}
    </div>
    {#if displayedSection === "workspaces"}
      <button
        class="ui-button ui-button--primary"
        type="button"
        onclick={() => workspaceDialog?.showModal()}
        data-od-id="create-workspace"><Plus size={16} />New Workspace</button
      >
    {:else if displayedSection === "boards" && canCreateBoard && !board}
      <button
        class="ui-button ui-button--primary"
        type="button"
        onclick={openCreateBoardDialog}
        data-od-id="create-board"><Plus size={16} />New Board</button
      >
    {/if}
  </header>

  {#if error}<div class="kanban-error" role="alert">{error}</div>{/if}
  <div
    class="kanban-page-body view-swap"
    data-view-phase={viewSwap.phase}
    data-view-direction={viewSwap.direction}
    {@attach viewSwap.attach}
  >
    {#if loading}
      <div class="kanban-empty" aria-live="polite">Loading Kanban…</div>
    {:else if displayedSection === "invitations"}
      <div class="kanban-invitations" data-od-id="kanban-invitations">
        {#each overview.invitations as invitation (invitation.workspace_id)}
          <article class="kanban-invitation-card">
            <div>
              <span>{invitation.role}</span>
              <h3>{invitation.workspace_name}</h3>
              <p>{invitation.invited_by_name} invited you.</p>
            </div>
            <div class="kanban-row-actions">
              <button
                class="ui-button ui-button--primary"
                type="button"
                disabled={busy}
                onclick={() => answerInvitation(invitation.workspace_id, true)}
                >Accept</button
              ><button
                class="ui-button ui-button--secondary"
                type="button"
                disabled={busy}
                onclick={() => answerInvitation(invitation.workspace_id, false)}
                >Decline</button
              >
            </div>
          </article>
        {:else}<div class="kanban-empty">No pending invitations.</div>{/each}
      </div>
    {:else if overview.workspaces.length === 0}
      <div class="kanban-empty">
        <h3>No Kanban workspace yet</h3>
        <p>
          Create one to start with a board and the Todo / In Progress / Finished
          workflow.
        </p>
        <button
          class="ui-button ui-button--primary"
          type="button"
          onclick={() => workspaceDialog?.showModal()}>Create Workspace</button
        >
      </div>
    {:else}
      <div class="kanban-workspace-switcher">
        <label for="kanban-workspace">Workspace</label>
        <select
          id="kanban-workspace"
          value={selectedWorkspaceId}
          onchange={(event) => selectWorkspace(event.currentTarget.value)}
          >{#each overview.workspaces as workspace (workspace.id)}<option
              value={workspace.id}>{workspace.name}</option
            >{/each}</select
        >
        {#if activeWorkspace}<span
            >{activeWorkspace.role} · {activeWorkspace.member_count} members</span
          >{/if}
      </div>

      {#if displayedSection === "boards"}
        {#if board}
          <div class="kanban-board-shell" data-od-id="active-kanban-board">
            <div class="kanban-board-toolbar">
              <button
                class="ui-button ui-button--ghost"
                type="button"
                onclick={() => void closeBoard()}
                ><ChevronLeft size={16} />All Boards</button
              >
              <div class="kanban-board-actions">
                {#if canEditBoard}
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    onclick={openEditBoardDialog}
                    data-od-id="edit-kanban-board"
                    ><Pencil size={15} />Edit Board</button
                  >
                {/if}
                {#if canCreateColumn}
                  <button
                    class="ui-button ui-button--secondary"
                    type="button"
                    aria-haspopup="dialog"
                    aria-controls="kanban-add-column-dialog"
                    onclick={openAddColumn}
                    data-od-id="show-add-kanban-column"
                    ><Plus size={15} />Add Column</button
                  >
                {/if}
                <button
                  class="ui-button ui-button--secondary"
                  type="button"
                  onclick={requestBoardArchive}
                  ><Archive size={15} />{board.archived
                    ? "Restore"
                    : "Archive"}</button
                >
              </div>
            </div>
            <div
              class="kanban-board-summary"
              data-od-id="active-kanban-board-summary"
            >
              <h3>{board.name}</h3>
              {#if board.description}
                <div
                  class="kanban-markdown-preview kanban-board-description"
                  {@attach renderSanitizedMarkdown(renderedBoardDescription)}
                ></div>
              {:else}
                <p class="kanban-board-description-empty">
                  No board description
                </p>
              {/if}
            </div>
            <div class="kanban-filters">
              <label
                ><Search size={15} /><input
                  aria-label="Search cards"
                  placeholder="Search cards"
                  bind:value={cardSearch}
                /></label
              >
              <select
                aria-label="Filter by assignee"
                bind:value={assigneeFilter}
                ><option value="">All assignees</option
                >{#each board.members as member (member.user_id)}<option
                    value={member.user_id}>{member.display_name}</option
                  >{/each}</select
              >
              <select aria-label="Filter by label" bind:value={labelFilter}
                ><option value="">All labels</option
                >{#each board.labels as label (label.id)}<option
                    value={label.id}>{label.name}</option
                  >{/each}</select
              >
              <select aria-label="Filter by due date" bind:value={dueFilter}
                ><option value="all">Any due date</option><option
                  value="overdue">Overdue</option
                ><option value="week">Due this week</option><option value="none"
                  >No due date</option
                ></select
              >
              {#if canEditCard && cardFiltersActive}<span
                  class="kanban-dnd-status"
                  >Clear filters to reorder cards.</span
                >{/if}
            </div>
            <DragDropProvider
              sensors={kanbanSensors}
              onDragStart={startKanbanDrag}
              onDragOver={previewKanbanMove}
              onDragEnd={(event) => void finishKanbanDrag(event)}
            >
              <div class="kanban-canvas" data-od-id="kanban-board-columns">
                {#each board.columns as column, columnIndex (column.id)}
                  <KanbanColumnDropzone
                    boardId={board.id}
                    id={column.id}
                    index={columnIndex}
                    label={`${column.name} column`}
                    odId={`kanban-column-${column.id}`}
                    entering={enteringColumnId === column.id}
                    cardDropDisabled={!canEditCard || cardFiltersActive || busy}
                    columnDragDisabled={!canEditColumn || busy}
                    reducedMotion={reducedMotion.current}
                  >
                    {#snippet header(columnHandle)}
                      <header>
                        <button
                          class="kanban-column-drag-handle"
                          type="button"
                          disabled={!canEditColumn || busy}
                          aria-label={`Drag ${column.name} Column to Reorder`}
                          {@attach columnHandle}
                          ><GripVertical size={15} aria-hidden="true" /><span
                            class="kanban-column-title">{column.name}</span
                          ><span class="kanban-column-count"
                            >{visibleCards(column.cards).length}</span
                          ></button
                        >
                        {#if canCreateCard || canDeleteColumn}
                          <div class="kanban-column-actions kanban-column-menu">
                            <button
                              class="kanban-icon-button kanban-column-menu-trigger"
                              type="button"
                              aria-label={`Actions for ${column.name}`}
                              aria-expanded={openColumnMenuId === column.id}
                              aria-controls={`kanban-column-menu-${column.id}`}
                              onclick={() => toggleColumnMenu(column.id)}
                              ><EllipsisVertical size={17} /></button
                            >
                            {#if openColumnMenuId === column.id}
                              <div
                                class="kanban-column-menu-popover"
                                id={`kanban-column-menu-${column.id}`}
                                role="group"
                                aria-label={`${column.name} column actions`}
                              >
                                {#if canCreateCard}
                                  <button
                                    type="button"
                                    onclick={() => openAddCard(column.id)}
                                    ><Plus size={15} />Add Card</button
                                  >
                                {/if}
                                {#if canDeleteColumn}
                                  <button
                                    class="kanban-column-menu-delete"
                                    class:confirm={pendingColumnDelete === column.id}
                                    type="button"
                                    disabled={busy}
                                    onclick={() => void removeColumn(column)}
                                    ><Trash2 size={15} />{pendingColumnDelete ===
                                    column.id
                                      ? "Confirm Delete"
                                      : "Delete Column"}</button
                                  >
                                {/if}
                              </div>
                            {/if}
                          </div>
                        {/if}
                      </header>
                    {/snippet}
                    {#snippet children()}
                      {#each visibleCards(column.cards) as card, index (card.id)}
                        <KanbanCardSortable
                          {card}
                          columnId={column.id}
                          disabled={!canEditCard || cardFiltersActive || busy}
                          {index}
                          reducedMotion={reducedMotion.current}
                          entering={enteringCardId === card.id}
                          avatarUrl={kanbanMemberAvatarUrl}
                          onopen={openCard}
                          oncontextmenu={openCardContext}
                        />
                      {/each}
                    {/snippet}
                    {#snippet footer()}
                      {#if canCreateCard}
                        <button
                          class="kanban-add-card"
                          type="button"
                          onclick={() => openAddCard(column.id)}
                          ><Plus size={15} />Add Card</button
                        >
                      {/if}
                    {/snippet}
                  </KanbanColumnDropzone>
                {/each}
              </div>
            </DragDropProvider>
          </div>
        {:else}
          <div class="kanban-list-toolbar">
            <label
              ><Search size={15} /><input
                aria-label="Search boards"
                placeholder="Search boards"
                bind:value={boardSearch}
              /></label
            ><button
              class="ui-toggle-button kanban-toggle-filter"
              type="button"
              aria-pressed={archivedBoards}
              disabled={busy}
              onclick={() => void toggleArchivedBoards()}
              ><span class="ui-toggle-indicator" aria-hidden="true"
                >{#if archivedBoards}<Check size={13} />{/if}</span
              ><span>Archived</span></button
            >
          </div>
          <div class="kanban-board-grid" data-od-id="kanban-board-list">
            {#each filteredBoards as item (item.id)}
              <article
                class="kanban-board-card"
                data-od-id={`kanban-board-${item.id}`}
              >
                <button
                  class="kanban-board-open"
                  type="button"
                  onclick={() => openBoard(item.id)}
                  ><span>{item.visibility}</span>
                  <h3>{item.name}</h3>
                  <dl>
                    <div>
                      <dt>Columns</dt>
                      <dd>{item.column_count}</dd>
                    </div>
                    <div>
                      <dt>Cards</dt>
                      <dd>{item.card_count}</dd>
                    </div>
                  </dl></button
                >
                <button
                  class:active={item.favorite}
                  class="kanban-favorite"
                  type="button"
                  aria-label={item.favorite
                    ? "Remove Favorite"
                    : "Add Favorite"}
                  onclick={() => toggleFavorite(item)}
                  ><Star
                    size={16}
                    fill={item.favorite ? "currentColor" : "none"}
                  /></button
                >
              </article>
            {:else}<div class="kanban-empty">
                No {archivedBoards ? "archived" : "active"} boards in this workspace.
              </div>{/each}
          </div>
        {/if}
      {:else if settings}
        <div class="kanban-settings" data-od-id="kanban-workspace-settings">
          <section class="kanban-settings-panel">
            <header>
              <div>
                <h3>Members</h3>
                <p>
                  Invitations are limited to accounts already on this Pandan
                  instance.
                </p>
              </div>
              <Users size={19} />
            </header>
            {#if settings.workspace.permissions.includes("member:invite")}<div
                class="kanban-member-search"
              >
                <label
                  ><Search size={15} /><input
                    placeholder="Name or email"
                    bind:value={memberQuery}
                    oninput={findMembers}
                  /></label
                ><select aria-label="Invitation role" bind:value={inviteRole}
                  ><option value="member">Member</option><option value="guest"
                    >Guest</option
                  >{#if settings.workspace.role === "admin"}<option
                      value="admin">Admin</option
                    >{/if}</select
                >{#if directoryResults.length}<div
                    class="kanban-directory-results"
                  >
                    {#each directoryResults as result (result.user_id)}<button
                        type="button"
                        onclick={() => invite(result.user_id)}
                        ><span
                          ><strong>{result.display_name}</strong><small
                            >{result.email}</small
                          ></span
                        ><Plus size={15} /></button
                      >{/each}
                  </div>{/if}
              </div>{/if}
            <div class="kanban-member-list">
              {#each settings.members as member (member.user_id)}
                <div>
                  <span class="kanban-member-avatar">
                    <span aria-hidden="true"
                      >{member.display_name.slice(0, 1).toUpperCase()}</span
                    >
                    <img
                      src={kanbanMemberAvatarUrl(member.user_id)}
                      alt=""
                      onerror={hideBrokenAvatar}
                    />
                  </span>
                  <span
                    ><strong
                      >{member.display_name}{member.user_id === viewerId
                        ? " (you)"
                        : ""}</strong
                    ><small>{member.email} · {member.status}</small></span
                  >
                  {#if settings.workspace.permissions.includes("member:edit")}<select
                      aria-label={`Role for ${member.display_name}`}
                      value={member.role}
                      onchange={(event) =>
                        changeRoleFromSelect(member.user_id, event)}
                      ><option value="admin">Admin</option><option
                        value="member">Member</option
                      ><option value="guest">Guest</option></select
                    >{:else}<span class="kanban-role-badge">{member.role}</span
                    >{/if}
                  {#if settings.workspace.permissions.includes("member:remove")}<button
                      class="kanban-icon-button"
                      type="button"
                      aria-label={`Remove ${member.display_name}`}
                      onclick={() => removeMember(member.user_id)}
                      ><Trash2 size={15} /></button
                    >{/if}
                </div>
              {/each}
            </div>
          </section>
          {#if settings.workspace.permissions.includes("workspace:manage")}<section
              class="kanban-settings-panel"
            >
              <header>
                <div>
                  <h3>Role permissions</h3>
                  <p>
                    Admin is immutable. Member and Guest follow kan.bn's
                    24-permission split.
                  </p>
                </div>
              </header>
              <div class="kanban-permission-table">
                <div class="kanban-permission-head">
                  <span>Capability</span><span>Member</span><span>Guest</span>
                </div>
                {#each permissionGroups as group (group.name)}<h4>
                    {group.name}
                  </h4>
                  {#each group.permissions as permission (permission)}<div
                      class="kanban-permission-row"
                    >
                      <code>{permission}</code
                      >{#each configurableRoles as role (role)}{@const granted =
                          roleGrant(role, permission)}<button
                          class="ui-toggle-button kanban-permission-toggle"
                          type="button"
                          aria-pressed={granted}
                          aria-label={`${role} ${permission}: ${granted ? "allowed" : "denied"}`}
                          disabled={busy}
                          onclick={() =>
                            void toggleRoleGrant(role, permission, !granted)}
                          ><span class="ui-toggle-indicator" aria-hidden="true"
                            >{#if granted}<Check size={13} />{/if}</span
                          ><span>{granted ? "Allow" : "Deny"}</span></button
                        >{/each}
                    </div>{/each}{/each}
              </div>
            </section>{/if}
          {#if settings.workspace.permissions.includes("workspace:delete")}<section
              class="kanban-settings-panel kanban-danger-zone"
            >
              <header>
                <div>
                  <h3>Delete workspace</h3>
                  <p>
                    Permanently removes every board, card, comment, and
                    attachment.
                  </p>
                </div>
              </header>
              <button
                class="ui-button ui-button--danger"
                type="button"
                onclick={() =>
                  run(async () => {
                    await deleteKanbanWorkspace(selectedWorkspaceId);
                    await loadOverview();
                  })}>Delete {settings.workspace.name}</button
              >
            </section>{/if}
        </div>
      {/if}
    {/if}
  </div>
</section>

<dialog
  class="ui-dialog kanban-dialog kanban-board-workflow-dialog"
  id="kanban-add-card-dialog"
  bind:this={addCardDialog}
  aria-labelledby="kanban-add-card-title"
  aria-describedby="kanban-add-card-description"
  onclose={resetAddCard}
  onclick={(event) => {
    if (event.target === event.currentTarget) closeAddCard();
  }}
  data-od-id="add-kanban-card"
>
  <form method="dialog" class="dialog-close-row">
    <button class="kanban-icon-button" aria-label="Close Add Card"
      ><X size={18} /></button
    >
  </form>
  <form class="kanban-dialog-form" onsubmit={submitCard}>
    <span class="kanban-kicker">{addCardColumn?.name ?? "COLUMN"}</span>
    <h3 id="kanban-add-card-title">Create a new card</h3>
    <p id="kanban-add-card-description">
      Add a focused work item to this column.
    </p>
    <label for="kanban-card-title">
      Card title
      <input
        id="kanban-card-title"
        required
        maxlength="200"
        placeholder="What needs to happen?"
        bind:this={addCardTitleInput}
        bind:value={addCardTitle}
      />
    </label>
    <div class="kanban-add-column-actions">
      <button
        class="ui-button ui-button--ghost"
        type="button"
        onclick={closeAddCard}
        data-od-id="cancel-add-kanban-card">Cancel</button
      >
      <button
        class="ui-button ui-button--primary"
        type="submit"
        disabled={busy || !addCardTitle.trim()}
        data-od-id="create-kanban-card">Add Card</button
      >
    </div>
  </form>
</dialog>

<dialog
  class="ui-dialog kanban-dialog kanban-board-workflow-dialog"
  id="kanban-add-column-dialog"
  bind:this={columnDialog}
  aria-labelledby="kanban-add-column-title"
  aria-describedby="kanban-add-column-description"
  onclose={resetAddColumn}
  onclick={(event) => {
    if (event.target === event.currentTarget) closeAddColumn();
  }}
  data-od-id="add-kanban-column"
>
  <form method="dialog" class="dialog-close-row">
    <button class="kanban-icon-button" aria-label="Close Add Column"
      ><X size={18} /></button
    >
  </form>
  <form class="kanban-dialog-form" onsubmit={submitColumn}>
    <span class="kanban-kicker">BOARD STRUCTURE</span>
    <h3 id="kanban-add-column-title">Create a new column</h3>
    <p id="kanban-add-column-description">
      Add the next stage to the end of this board.
    </p>
    <label for="kanban-column-name">
      Column name
      <input
        id="kanban-column-name"
        required
        maxlength="80"
        placeholder="e.g. Review"
        bind:this={columnNameInput}
        bind:value={newColumnName}
      />
    </label>
    <div class="kanban-add-column-actions">
      <button
        class="ui-button ui-button--ghost"
        type="button"
        onclick={closeAddColumn}
        data-od-id="cancel-add-kanban-column">Cancel</button
      >
      <button
        class="ui-button ui-button--primary"
        type="submit"
        disabled={busy || !newColumnName.trim()}
        data-od-id="create-kanban-column">Create Column</button
      >
    </div>
  </form>
</dialog>

<dialog
  class="ui-dialog kanban-dialog"
  bind:this={workspaceDialog}
  onclose={() => (error = "")}
  onclick={(event) => {
    if (event.target === event.currentTarget) workspaceDialog?.close();
  }}
  data-od-id="workspace-dialog"
>
  <form method="dialog" class="dialog-close-row">
    <button class="kanban-icon-button" aria-label="Close"
      ><X size={18} /></button
    >
  </form>
  <form class="kanban-dialog-form" onsubmit={submitWorkspace}>
    <span class="kanban-kicker">NEW WORKSPACE</span>
    <h3>Create a shared workspace</h3>
    <label
      >Name<input required maxlength="80" bind:value={workspaceName} /></label
    ><label
      >Description<textarea
        rows="3"
        maxlength="1000"
        bind:value={workspaceDescription}></textarea></label
    ><button class="ui-button ui-button--primary" type="submit" disabled={busy}
      >Create Workspace</button
    >
  </form>
</dialog>

<dialog
  class="ui-dialog kanban-dialog"
  bind:this={boardDialog}
  onclose={() => (error = "")}
  onclick={(event) => {
    if (event.target === event.currentTarget) boardDialog?.close();
  }}
  data-od-id="board-dialog"
>
  <form method="dialog" class="dialog-close-row">
    <button class="kanban-icon-button" aria-label="Close"
      ><X size={18} /></button
    >
  </form>
  <form class="kanban-dialog-form" onsubmit={submitBoard}>
    <span class="kanban-kicker"
      >{boardDialogMode === "edit" ? "BOARD SETTINGS" : "NEW BOARD"}</span
    >
    <h3>
      {boardDialogMode === "edit"
        ? "Edit board details"
        : "Start with a proven flow"}
    </h3>
    <p>
      {boardDialogMode === "edit"
        ? "Update the board name, description, and workspace visibility."
        : "Todo, In Progress, and Finished are created automatically."}
    </p>
    <label>Name<input required maxlength="120" bind:value={boardName} /></label
    ><label
      >Description · Markdown<textarea
        rows="4"
        maxlength="2000"
        bind:value={boardDescription}></textarea></label
    ><label
      >Visibility<select bind:value={boardVisibility}
        ><option value="private">Private workspace board</option><option
          value="public">Public to workspace members</option
        ></select
      ></label
    ><button class="ui-button ui-button--primary" type="submit" disabled={busy}
      >{boardDialogMode === "edit" ? "Save Board" : "Create Board"}</button
    >
  </form>
</dialog>

<dialog
  class="ui-dialog kanban-dialog kanban-confirmation-dialog"
  bind:this={archiveBoardDialog}
  aria-labelledby="archive-board-title"
  aria-describedby="archive-board-description"
  onclick={(event) => {
    if (event.target === event.currentTarget) archiveBoardDialog?.close();
  }}
  data-od-id="archive-board-dialog"
>
  <form method="dialog" class="dialog-close-row">
    <button class="kanban-icon-button" aria-label="Close Archive Confirmation"
      ><X size={18} /></button
    >
  </form>
  <form class="kanban-dialog-form" onsubmit={confirmBoardArchive}>
    <span class="kanban-kicker">ARCHIVE BOARD</span>
    <h3 id="archive-board-title">Archive {board?.name}?</h3>
    <p id="archive-board-description" class="kanban-confirmation-copy">
      The board will leave the active boards list. Its columns, cards, and
      activity will remain intact and can be restored later.
    </p>
    <div class="kanban-confirmation-actions">
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={() => archiveBoardDialog?.close()}>Cancel</button
      >
      <button class="ui-button ui-button--danger" type="submit" disabled={busy}
        >Archive Board</button
      >
    </div>
  </form>
</dialog>

<dialog
  class="ui-dialog kanban-context-dialog"
  bind:this={cardContextDialog}
  style:--context-x={`${contextDialogX}px`}
  style:--context-y={`${contextDialogY}px`}
  aria-label="Card actions"
  onclose={resetCardContext}
  onclick={(event) => {
    if (event.target === event.currentTarget) closeCardContext();
  }}
  oncontextmenu={(event) => {
    event.preventDefault();
    closeCardContext();
  }}
  data-od-id="kanban-card-context-dialog"
>
  {#if contextCard}
    <div class="kanban-context-actions" role="group">
      {#if canEditCard}
        <button type="button" onclick={() => void editContextCard()}
          ><Pencil size={16} />Edit</button
        >
      {/if}
      {#if canCreateCard}
        <button
          type="button"
          disabled={busy}
          onclick={() => void duplicateContextCard()}
          ><Copy size={16} />Duplicate</button
        >
      {/if}
      {#if canDeleteCard}
        <button
          class="kanban-context-delete"
          class:confirm={pendingCardDelete}
          type="button"
          disabled={busy}
          onclick={() => void deleteContextCard()}
          ><Trash2 size={16} />{pendingCardDelete
            ? "Confirm Delete"
            : "Delete"}</button
        >
      {/if}
    </div>
  {/if}
</dialog>

<dialog
  class="ui-dialog kanban-card-dialog"
  bind:this={cardDialog}
  oncancel={(event) => {
    event.preventDefault();
    void closeCardDialog();
  }}
  onclick={(event) => {
    if (event.target === event.currentTarget) void closeCardDialog();
  }}
  onclose={() => {
    selectedCard = null;
    cardDescriptionMode = "preview";
  }}
  data-od-id="card-dialog"
>
  {#if selectedCard && board}<div class="kanban-card-editor">
      <header>
        <span class="kanban-kicker">CARD / {selectedCard.id.slice(0, 8)}</span
        ><button
          class="kanban-icon-button"
          type="button"
          aria-label="Close Card"
          onclick={() => void closeCardDialog()}><X size={19} /></button
        >
      </header>
      <div class="kanban-card-editor-grid">
        <main>
          <label class="kanban-title-field"
            >Title<input
              bind:value={cardTitle}
              disabled={!canEditCard}
              onblur={() => void saveCard()}
            /></label
          >
          <div class="kanban-description-tabs">
            <span>Description · Markdown</span>
            {#if canEditCard}
              <span class="kanban-save-status" aria-live="polite"
                >{cardSaving ? "Saving…" : "Saved automatically"}</span
              >
            {/if}
          </div>
          {#if cardDescriptionMode === "edit" && canEditCard}
            <div
              class="kanban-description-surface is-editing"
              onfocusout={finishDescriptionEditing}
            >
              <textarea
                {@attach focusDescriptionEditor}
                class="kanban-markdown-editor"
                rows="9"
                bind:value={cardDescription}
                placeholder="Add details with Markdown…"></textarea>
            </div>
          {:else if canEditCard}
            <div
              class="kanban-description-surface is-clickable"
              role="button"
              tabindex="0"
              aria-label="Edit Card Description"
              onclick={startDescriptionEditing}
              onkeydown={handleDescriptionSurfaceKeydown}
            >
              {#if cardDescription}
                <div
                  class="kanban-markdown-preview"
                  {@attach renderSanitizedMarkdown(renderedCardDescription)}
                ></div>
              {:else}
                <p class="kanban-empty-description">
                  <Plus size={15} />Add a description
                </p>
              {/if}
            </div>
          {:else}
            <div class="kanban-description-surface">
              {#if cardDescription}<div
                  class="kanban-markdown-preview"
                  {@attach renderSanitizedMarkdown(renderedCardDescription)}
                ></div>{:else}<p class="kanban-empty-description">
                  No description.
                </p>{/if}
            </div>
          {/if}
          <section class="kanban-detail-section">
            <header><h4>Checklists</h4></header>
            {#each selectedCard.checklists as checklist (checklist.id)}<div
                class="kanban-checklist"
              >
                <div>
                  <strong>{checklist.name}</strong><button
                    class="kanban-icon-button"
                    type="button"
                    aria-label="Delete Checklist"
                    onclick={() => removeChecklist(checklist.id)}
                    ><Trash2 size={14} /></button
                  >
                </div>
                {#each checklist.items as item (item.id)}<button
                    class="ui-toggle-button kanban-checklist-toggle"
                    type="button"
                    aria-pressed={item.completed}
                    disabled={busy}
                    onclick={() =>
                      void toggleChecklistItem(
                        checklist.id,
                        item.id,
                        item.title,
                        !item.completed,
                      )}
                    ><span class="ui-toggle-indicator" aria-hidden="true"
                      >{#if item.completed}<Check size={13} />{/if}</span
                    ><span class:completed={item.completed}>{item.title}</span
                    ></button
                  >{/each}
                <form
                  onsubmit={(event) => {
                    event.preventDefault();
                    void addChecklistItem(checklist.id);
                  }}
                >
                  <input
                    placeholder="Add checklist item"
                    value={checklistItemDraft[checklist.id] ?? ""}
                    oninput={(event) =>
                      (checklistItemDraft = {
                        ...checklistItemDraft,
                        [checklist.id]: event.currentTarget.value,
                      })}
                  /><button
                    class="kanban-icon-button"
                    aria-label="Add Checklist Item"><Plus size={15} /></button
                  >
                </form>
              </div>{/each}
            <form
              class="kanban-inline-form"
              onsubmit={(event) => {
                event.preventDefault();
                void addChecklist();
              }}
            >
              <input
                placeholder="New checklist"
                bind:value={checklistDraft}
              /><button class="ui-button ui-button--secondary" type="submit"
                >Add Checklist</button
              >
            </form>
          </section>
          <section class="kanban-detail-section">
            <header>
              <h4>Comments</h4>
              <span>{selectedCard.comments.length}</span>
            </header>
            <form
              class="kanban-comment-form"
              onsubmit={(event) => {
                event.preventDefault();
                void addComment();
              }}
            >
              <textarea
                rows="3"
                placeholder="Write a comment…"
                bind:value={commentDraft}></textarea><button
                class="ui-button ui-button--primary"
                type="submit"
                disabled={!commentDraft.trim()}>Comment</button
              >
            </form>
            <div class="kanban-comments">
              {#each selectedCard.comments as comment (comment.id)}<article>
                  <span class="kanban-member-avatar"
                    ><span aria-hidden="true"
                      >{comment.author_name.slice(0, 1).toUpperCase()}</span
                    >{#if comment.user_id}<img
                        src={kanbanMemberAvatarUrl(comment.user_id)}
                        alt=""
                        onerror={hideBrokenAvatar}
                      />{/if}</span
                  >
                  <div>
                    <header>
                      <strong>{comment.author_name}</strong><time
                        >{new Date(comment.created_at).toLocaleString()}</time
                      >{#if comment.user_id === viewerId || board.permissions.includes("comment:delete")}<button
                          class="kanban-icon-button"
                          type="button"
                          aria-label="Delete Comment"
                          onclick={() => removeComment(comment.id)}
                          ><Trash2 size={13} /></button
                        >{/if}
                    </header>
                    <p>{comment.content}</p>
                  </div>
                </article>{/each}
            </div>
          </section>
        </main>
        <aside>
          <section>
            <h4>Due date</h4>
            <PandanDatePicker
              id="kanban-card-due-date"
              ariaLabel="Card due date"
              bind:value={cardDueDate}
              disabled={!canEditCard}
              compact
              odId="kanban-card-due-date"
              onchange={() => void saveCard()}
            />
          </section>
          <section>
            <h4>Assignees</h4>
            {#each board.members as member (member.user_id)}{@const selected =
                cardAssigneeIds.includes(member.user_id)}<button
                class="ui-toggle-button kanban-option"
                type="button"
                aria-pressed={selected}
                disabled={!canEditCard}
                onclick={() => toggleCardAssignee(member.user_id)}
                ><span class="ui-toggle-indicator" aria-hidden="true"
                  >{#if selected}<Check size={13} />{/if}</span
                ><span>{member.display_name}</span></button
              >{/each}
          </section>
          <section>
            <h4>Labels</h4>
            {#each board.labels as label (label.id)}{@const selected =
                cardLabelIds.includes(label.id)}<button
                class="ui-toggle-button kanban-option"
                type="button"
                aria-pressed={selected}
                disabled={!canEditCard}
                onclick={() => toggleCardLabel(label.id)}
                ><span class="ui-toggle-indicator" aria-hidden="true"
                  >{#if selected}<Check size={13} />{/if}</span
                ><i class={`kanban-label-dot is-${label.color}`}></i><span
                  >{label.name}</span
                ></button
              >{/each}{#if canEditCard}<div class="kanban-new-label">
                <input
                  placeholder="New label"
                  maxlength="40"
                  bind:value={labelName}
                /><select aria-label="Label color" bind:value={labelColor}
                  >{#each labelColors as color (color)}<option value={color}
                      >{color}</option
                    >{/each}</select
                ><button
                  class="kanban-icon-button"
                  type="button"
                  aria-label="Create Label"
                  onclick={addLabel}><Plus size={15} /></button
                >
              </div>{/if}
          </section>
          <section>
            <h4>Attachments</h4>
            <label class="ui-button ui-button--secondary kanban-upload"
              ><Paperclip size={14} />Attach File<input
                type="file"
                onchange={uploadAttachment}
              /></label
            >{#each selectedCard.attachments as attachment (attachment.id)}<div
                class="kanban-attachment"
              >
                <a
                  href={kanbanAttachmentUrl(attachment.id)}
                  target="_blank"
                  rel="noreferrer">{attachment.file_name}</a
                ><button
                  class="kanban-icon-button"
                  type="button"
                  aria-label="Delete Attachment"
                  onclick={() => removeAttachment(attachment.id)}
                  ><Trash2 size={13} /></button
                >
              </div>{/each}
          </section>
          {#if canEditCard}<button
              class="ui-button ui-button--danger"
              type="button"
              onclick={archiveCard}><Archive size={14} />Archive Card</button
            >{/if}
        </aside>
      </div>
    </div>{/if}
</dialog>

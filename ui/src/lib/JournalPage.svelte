<script lang="ts">
  import DOMPurify from "dompurify";
  import { marked } from "marked";
  import ArrowDown from "lucide-svelte/icons/arrow-down";
  import ArrowUp from "lucide-svelte/icons/arrow-up";
  import ChevronDown from "lucide-svelte/icons/chevron-down";
  import ChevronRight from "lucide-svelte/icons/chevron-right";
  import FileText from "lucide-svelte/icons/file-text";
  import FolderInput from "lucide-svelte/icons/folder-input";
  import MoreHorizontal from "lucide-svelte/icons/ellipsis";
  import PanelLeftClose from "lucide-svelte/icons/panel-left-close";
  import PanelLeftOpen from "lucide-svelte/icons/panel-left-open";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import Save from "lucide-svelte/icons/save";
  import Search from "lucide-svelte/icons/search";
  import SmilePlus from "lucide-svelte/icons/smile-plus";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import {
    emojiCatalog,
    emojiCatalogByCategory,
    emojiCategories,
    type EmojiCatalogEntry,
    type EmojiCategoryId,
  } from "$lib/emojiCatalog";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    createJournalNode,
    deleteJournalNode,
    fetchJournal,
    updateJournalNode,
    type JournalNode,
  } from "$lib/api";

  type TreeRow = { node: JournalNode; depth: number };
  type ViewMode = "raw" | "rendered";
  type ManageMode = "move" | "logo" | "delete";
  type DropPlacement = "before" | "inside" | "after";
  type DragTarget = {
    nodeId: string | "root";
    placement: DropPlacement;
  };
  type MenuPosition = { x: number; y: number };
  type EmojiGroupId = "journal" | EmojiCategoryId;
  type EmojiOption = Pick<EmojiCatalogEntry, "emoji" | "label" | "searchText">;

  const journalEmojiRecommendations = [
    "📓",
    "📝",
    "📔",
    "📚",
    "📖",
    "🔖",
    "✍️",
    "💭",
    "💡",
    "📌",
    "📅",
    "🧠",
  ] as const;
  const emojiGroups: ReadonlyArray<{
    id: EmojiGroupId;
    label: string;
    icon: string;
  }> = [
    { id: "journal", label: "Journal", icon: "📓" },
    ...emojiCategories.map((category) => ({
      ...category,
      icon: {
        people: "🙂",
        nature: "🐼",
        food: "🍎",
        activities: "⚽",
        travel: "✈️",
        objects: "💡",
        symbols: "🔣",
        flags: "🏳️",
      }[category.id],
    })),
  ];
  const emojiSequencePattern =
    /^(?:\p{Emoji}|\p{Emoji_Component}|\u200d|\ufe0f|\u20e3)+$/u;
  const emojiPresentationPattern = /(?:\p{Emoji_Presentation}|\ufe0f|\u20e3)/u;

  let nodes = $state.raw<JournalNode[]>([]);
  let loading = $state(true);
  let error = $state("");
  let searchQuery = $state("");
  let selectedId = $state<string | null>(null);
  let expandedIds = $state.raw<string[]>([]);
  let explorerOpen = $state(true);
  let viewMode = $state<ViewMode>("raw");
  let draftContent = $state("");
  let savedContent = $state("");
  let saving = $state(false);
  let inlineNameInput = $state<HTMLInputElement>();
  let manageDialog = $state<HTMLDialogElement>();
  let emojiSearchInput: HTMLInputElement | undefined;
  let itemMenu: HTMLDivElement | undefined;
  let menuTrigger: HTMLElement | undefined;
  let managingNodeId = $state<string | null>(null);
  let manageMode = $state<ManageMode>("move");
  let inlineCreateParentId = $state<string | null | undefined>(undefined);
  let inlineCreateName = $state("");
  let inlineError = $state("");
  let creating = $state(false);
  let manageParentId = $state("");
  let emojiSearch = $state("");
  let activeEmojiGroup = $state<EmojiGroupId>("journal");
  let manageError = $state("");
  let managing = $state(false);
  let menuNodeId = $state<string | null>(null);
  let menuPosition = $state<MenuPosition | null>(null);
  let renamingNodeId = $state<string | null>(null);
  let renameName = $state("");
  let renaming = $state(false);
  let draggingNodeId = $state<string | null>(null);
  let dragTarget = $state<DragTarget | null>(null);
  let movingNodeId = $state<string | null>(null);

  let selectedNode = $derived(
    nodes.find((node) => node.id === selectedId) ?? null,
  );
  let managedNode = $derived(
    nodes.find((node) => node.id === managingNodeId) ?? null,
  );
  let documents = $derived([...nodes].sort(sortNodes));
  let visibleRows = $derived.by(buildVisibleRows);
  let dirty = $derived(selectedNode !== null && draftContent !== savedContent);
  let renderedHtml = $derived.by(() => {
    if (typeof window === "undefined") return "";
    const parsed = marked.parse(draftContent, {
      async: false,
      breaks: true,
      gfm: true,
    });
    return DOMPurify.sanitize(String(parsed), {
      USE_PROFILES: { html: true },
    });
  });
  let wordCount = $derived(
    draftContent.trim() ? draftContent.trim().split(/\s+/).length : 0,
  );
  let characterCount = $derived(draftContent.length);
  let currentPath = $derived.by(() =>
    selectedNode ? journalPath(selectedNode) : "Journal",
  );
  let visibleEmojiOptions = $derived.by(() => {
    const query = normalizeEmojiSearch(emojiSearch);
    if (!query) {
      return activeEmojiGroup === "journal"
        ? journalEmojiRecommendations.map(emojiOption)
        : emojiCatalogByCategory[activeEmojiGroup];
    }
    const matches: EmojiOption[] = emojiCatalog.filter(
      (entry) =>
        normalizeEmojiSearch(entry.searchText).includes(query) ||
        entry.emoji === emojiSearch.trim(),
    );
    const pasted = emojiSearch.trim();
    if (
      looksLikeEmojiSequence(pasted) &&
      !matches.some((entry) => entry.emoji === pasted)
    ) {
      matches.unshift({
        emoji: pasted,
        label: "pasted emoji",
        searchText: pasted,
      });
    }
    return matches;
  });
  let showDefaultLogoOption = $derived(
    !emojiSearch.trim() && activeEmojiGroup === "journal",
  );
  let visibleEmojiOptionCount = $derived(
    visibleEmojiOptions.length + (showDefaultLogoOption ? 1 : 0),
  );

  function emojiOption(emoji: string): EmojiOption {
    return (
      emojiCatalog.find((entry) => entry.emoji === emoji) ?? {
        emoji,
        label: emoji,
        searchText: emoji,
      }
    );
  }

  function normalizeEmojiSearch(value: string) {
    return value
      .trim()
      .toLowerCase()
      .replaceAll("_", " ")
      .replaceAll("-", " ")
      .replace(/\s+/g, " ");
  }

  function selectEmojiGroup(groupId: EmojiGroupId) {
    activeEmojiGroup = groupId;
    emojiSearch = "";
  }

  function looksLikeEmojiSequence(value: string) {
    return (
      value.length > 0 &&
      value.length <= 32 &&
      emojiSequencePattern.test(value) &&
      emojiPresentationPattern.test(value)
    );
  }

  onMount(() => {
    void loadJournal();
  });

  function buildVisibleRows(): TreeRow[] {
    const query = searchQuery.trim().toLowerCase();
    if (query) {
      return nodes
        .filter((node) =>
          [node.name, journalPath(node)]
            .join(" ")
            .toLowerCase()
            .includes(query),
        )
        .sort(sortNodes)
        .map((node) => ({ node, depth: pathDepth(node) }));
    }
    const rows: TreeRow[] = [];
    const visit = (parentId: string | null, depth: number) => {
      const children = nodes
        .filter((node) => node.parent_id === parentId)
        .sort(sortNodes);
      for (const node of children) {
        rows.push({ node, depth });
        if (expandedIds.includes(node.id)) {
          visit(node.id, depth + 1);
        }
      }
    };
    visit(null, 0);
    return rows;
  }

  function sortNodes(left: JournalNode, right: JournalNode) {
    if (left.position !== right.position) return left.position - right.position;
    return left.name.localeCompare(right.name);
  }

  function hasChildren(nodeId: string) {
    return nodes.some((node) => node.parent_id === nodeId);
  }

  function pathDepth(node: JournalNode) {
    let depth = 0;
    let parentId = node.parent_id;
    const seen = new SvelteSet<string>();
    while (parentId && !seen.has(parentId)) {
      seen.add(parentId);
      depth += 1;
      parentId =
        nodes.find((candidate) => candidate.id === parentId)?.parent_id ?? null;
    }
    return depth;
  }

  function journalPath(node: JournalNode) {
    const parts = [node.name];
    let parentId = node.parent_id;
    const seen = new SvelteSet<string>();
    while (parentId && !seen.has(parentId)) {
      seen.add(parentId);
      const parent = nodes.find((candidate) => candidate.id === parentId);
      if (!parent) break;
      parts.unshift(parent.name);
      parentId = parent.parent_id;
    }
    return `Journal / ${parts.join(" / ")}`;
  }

  function expandAncestors(node: JournalNode) {
    const ancestors = new SvelteSet(expandedIds);
    let parentId = node.parent_id;
    while (parentId) {
      ancestors.add(parentId);
      parentId =
        nodes.find((candidate) => candidate.id === parentId)?.parent_id ?? null;
    }
    expandedIds = [...ancestors];
  }

  async function loadJournal() {
    loading = true;
    error = "";
    try {
      const response = await fetchJournal();
      nodes = response.nodes;
      if (selectedId && !nodes.some((node) => node.id === selectedId)) {
        selectedId = null;
      }
      const initial =
        nodes.find((node) => node.id === selectedId) ?? nodes[0] ?? null;
      if (initial) setSelectedDocument(initial);
    } catch (reason: unknown) {
      error =
        reason instanceof Error ? reason.message : "Unable to load journal";
    } finally {
      loading = false;
    }
  }

  function setSelectedDocument(node: JournalNode) {
    selectedId = node.id;
    draftContent = node.content;
    savedContent = node.content;
    expandAncestors(node);
  }

  async function selectNode(node: JournalNode) {
    if (node.id === selectedId) return;
    if (!(await saveCurrent())) return;
    setSelectedDocument(node);
  }

  function toggleDocument(id: string) {
    expandedIds = expandedIds.includes(id)
      ? expandedIds.filter((candidate) => candidate !== id)
      : [...expandedIds, id];
  }

  function closeTransientControls() {
    closeItemMenu();
    renamingNodeId = null;
    inlineCreateParentId = undefined;
    inlineError = "";
  }

  async function saveCurrent() {
    if (!selectedNode || !dirty) return true;
    if (saving) return false;
    saving = true;
    error = "";
    try {
      const updated = await updateJournalNode(selectedNode.id, {
        content: draftContent,
      });
      nodes = nodes.map((node) => (node.id === updated.id ? updated : node));
      savedContent = updated.content;
      return true;
    } catch (reason: unknown) {
      error =
        reason instanceof Error ? reason.message : "Unable to save document";
      return false;
    } finally {
      saving = false;
    }
  }

  function handleShortcut(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void saveCurrent();
      return;
    }
    if (event.key === "Escape") {
      if (menuNodeId) {
        event.preventDefault();
        closeItemMenu(true);
        return;
      }
      closeTransientControls();
    }
  }

  function handleWindowClick(event: MouseEvent) {
    const target = event.target;
    if (target instanceof Element && target.closest(".journal-row-actions")) {
      return;
    }
    closeItemMenu();
  }

  function captureInlineNameInput(node: HTMLInputElement) {
    inlineNameInput = node;
    return () => {
      inlineNameInput = undefined;
    };
  }

  function captureManageDialog(node: HTMLDialogElement) {
    manageDialog = node;
    return () => {
      manageDialog = undefined;
    };
  }

  function captureEmojiSearchInput(node: HTMLInputElement) {
    emojiSearchInput = node;
    return () => {
      emojiSearchInput = undefined;
    };
  }

  function captureItemMenu(node: HTMLDivElement) {
    itemMenu = node;
    return () => {
      itemMenu = undefined;
    };
  }

  function attachRenderedPreview(node: HTMLElement) {
    $effect(() => {
      node.innerHTML = renderedHtml;
    });
  }

  async function beginCreate(parentId: string | null) {
    closeItemMenu();
    renamingNodeId = null;
    searchQuery = "";
    inlineCreateParentId = parentId;
    inlineCreateName = "";
    inlineError = "";
    if (parentId) {
      const parent = nodes.find((node) => node.id === parentId);
      if (parent) {
        expandAncestors(parent);
        if (!expandedIds.includes(parentId)) {
          expandedIds = [...expandedIds, parentId];
        }
      }
    }
    await tick();
    inlineNameInput?.focus();
  }

  function cancelCreate() {
    inlineCreateParentId = undefined;
    inlineCreateName = "";
    inlineError = "";
  }

  async function submitCreate(event: SubmitEvent) {
    event.preventDefault();
    if (creating || inlineCreateParentId === undefined) return;
    creating = true;
    inlineError = "";
    try {
      const created = await createJournalNode({
        parent_id: inlineCreateParentId,
        name: inlineCreateName.trim() || "Untitled",
      });
      nodes = [...nodes, created];
      expandAncestors(created);
      cancelCreate();
      setSelectedDocument(created);
    } catch (reason: unknown) {
      inlineError =
        reason instanceof Error
          ? reason.message
          : "Unable to create journal item";
    } finally {
      creating = false;
    }
  }

  async function beginRename(node: JournalNode) {
    closeItemMenu();
    inlineCreateParentId = undefined;
    renamingNodeId = node.id;
    renameName = node.name;
    inlineError = "";
    await tick();
    inlineNameInput?.select();
  }

  function cancelRename() {
    renamingNodeId = null;
    renameName = "";
    inlineError = "";
  }

  async function submitRename(event: SubmitEvent, node: JournalNode) {
    event.preventDefault();
    if (renaming || renamingNodeId !== node.id) return;
    const name = renameName.trim();
    if (!name) {
      inlineError = "A file name is required.";
      return;
    }
    renaming = true;
    inlineError = "";
    try {
      const updated = await updateJournalNode(node.id, { name });
      nodes = nodes.map((candidate) =>
        candidate.id === updated.id ? updated : candidate,
      );
      cancelRename();
    } catch (reason: unknown) {
      inlineError =
        reason instanceof Error ? reason.message : "Unable to rename file";
    } finally {
      renaming = false;
    }
  }

  function closeItemMenu(returnFocus = false) {
    menuNodeId = null;
    menuPosition = null;
    if (returnFocus) menuTrigger?.focus();
    menuTrigger = undefined;
  }

  function focusFirstMenuItem() {
    itemMenu
      ?.querySelector<HTMLButtonElement>(
        'button[role="menuitem"]:not(:disabled)',
      )
      ?.focus();
  }

  async function showItemMenu(
    nodeId: string,
    position: MenuPosition | null,
    trigger?: HTMLElement,
  ) {
    menuNodeId = nodeId;
    menuPosition = position;
    menuTrigger = trigger;
    renamingNodeId = null;
    inlineCreateParentId = undefined;
    await tick();

    if (position && itemMenu) {
      const bounds = itemMenu.getBoundingClientRect();
      const gutter = 8;
      menuPosition = {
        x: Math.max(
          gutter,
          Math.min(position.x, window.innerWidth - bounds.width - gutter),
        ),
        y: Math.max(
          gutter,
          Math.min(position.y, window.innerHeight - bounds.height - gutter),
        ),
      };
      await tick();
    }

    focusFirstMenuItem();
  }

  function toggleMenu(event: MouseEvent, nodeId: string) {
    if (menuNodeId === nodeId && menuPosition === null) {
      closeItemMenu();
      return;
    }
    const trigger =
      event.currentTarget instanceof HTMLElement
        ? event.currentTarget
        : undefined;
    void showItemMenu(nodeId, null, trigger);
  }

  function openContextMenu(event: MouseEvent, node: JournalNode) {
    event.preventDefault();
    event.stopPropagation();
    const row =
      event.currentTarget instanceof HTMLElement
        ? event.currentTarget
        : undefined;
    const trigger = row?.querySelector<HTMLElement>(".journal-tree-more");
    void showItemMenu(
      node.id,
      { x: event.clientX, y: event.clientY },
      trigger ?? row,
    );
  }

  function openKeyboardContextMenu(event: KeyboardEvent, node: JournalNode) {
    if (
      event.key !== "ContextMenu" &&
      !(event.shiftKey && event.key === "F10")
    ) {
      return;
    }
    event.preventDefault();
    const trigger =
      event.currentTarget instanceof HTMLElement
        ? event.currentTarget
        : undefined;
    const bounds = trigger?.getBoundingClientRect();
    void showItemMenu(
      node.id,
      {
        x: bounds ? Math.min(bounds.right, bounds.left + 180) : 12,
        y: bounds ? bounds.bottom : 12,
      },
      trigger,
    );
  }

  function handleItemMenuKeydown(event: KeyboardEvent) {
    if (!itemMenu) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeItemMenu(true);
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      return;
    }
    const items = [
      ...itemMenu.querySelectorAll<HTMLButtonElement>(
        'button[role="menuitem"]:not(:disabled)',
      ),
    ];
    if (!items.length) return;
    event.preventDefault();
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? items.length - 1
          : event.key === "ArrowUp"
            ? index <= 0
              ? items.length - 1
              : index - 1
            : (index + 1) % items.length;
    items[next]?.focus();
  }

  async function openManage(node: JournalNode, mode: ManageMode) {
    closeItemMenu();
    managingNodeId = node.id;
    manageMode = mode;
    manageParentId = node.parent_id ?? "";
    emojiSearch = "";
    activeEmojiGroup = "journal";
    manageError = "";
    manageDialog?.showModal();
    if (mode === "logo") {
      await tick();
      emojiSearchInput?.focus();
    }
  }

  function descendantIds(nodeId: string) {
    const descendants = new SvelteSet<string>();
    const visit = (id: string) => {
      for (const child of nodes.filter((node) => node.parent_id === id)) {
        descendants.add(child.id);
        visit(child.id);
      }
    };
    visit(nodeId);
    return descendants;
  }

  function availableParents(node: JournalNode) {
    const unavailable = descendantIds(node.id);
    unavailable.add(node.id);
    return documents.filter((document) => !unavailable.has(document.id));
  }

  function applyUserOrder(
    updated: JournalNode,
    sourceParentId: string | null,
    requestedPosition?: number,
  ) {
    const remaining = nodes.filter((node) => node.id !== updated.id);
    const replacements: Record<string, JournalNode> = {};
    if (sourceParentId !== updated.parent_id) {
      remaining
        .filter((node) => node.parent_id === sourceParentId)
        .sort(sortNodes)
        .forEach((node, position) => {
          replacements[node.id] = { ...node, position };
        });
    }
    const destination = remaining
      .filter((node) => node.parent_id === updated.parent_id)
      .sort(sortNodes);
    const position = Math.min(
      Math.max(requestedPosition ?? updated.position, 0),
      destination.length,
    );
    destination.splice(position, 0, updated);
    destination.forEach((node, siblingPosition) => {
      replacements[node.id] = { ...node, position: siblingPosition };
    });
    nodes = [
      ...remaining.map((node) => replacements[node.id] ?? node),
      replacements[updated.id] ?? updated,
    ];
  }

  async function moveNode(
    node: JournalNode,
    parentId: string | null,
    position?: number,
  ) {
    if (
      movingNodeId ||
      (node.parent_id === parentId && position === undefined)
    ) {
      return;
    }
    movingNodeId = node.id;
    try {
      const updated = await updateJournalNode(node.id, {
        parent_id: parentId,
        ...(position === undefined ? {} : { position }),
      });
      applyUserOrder(updated, node.parent_id, position);
      if (parentId && !expandedIds.includes(parentId)) {
        expandedIds = [...expandedIds, parentId];
      }
      expandAncestors(updated);
    } finally {
      movingNodeId = null;
    }
  }

  function orderedSiblings(node: JournalNode) {
    return nodes
      .filter((candidate) => candidate.parent_id === node.parent_id)
      .sort(sortNodes);
  }

  function canShiftNode(node: JournalNode, direction: -1 | 1) {
    const siblings = orderedSiblings(node);
    const position = siblings.findIndex(
      (candidate) => candidate.id === node.id,
    );
    return (
      position >= 0 &&
      position + direction >= 0 &&
      position + direction < siblings.length
    );
  }

  async function shiftNode(node: JournalNode, direction: -1 | 1) {
    const siblings = orderedSiblings(node);
    const position = siblings.findIndex(
      (candidate) => candidate.id === node.id,
    );
    if (position < 0 || !canShiftNode(node, direction)) return;
    closeItemMenu();
    error = "";
    try {
      await moveNode(node, node.parent_id, position + direction);
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to reorder journal item";
    }
  }

  async function submitManage(event: SubmitEvent) {
    event.preventDefault();
    if (!managedNode || managing || manageMode !== "move") return;
    managing = true;
    manageError = "";
    try {
      await moveNode(managedNode, manageParentId || null);
      manageDialog?.close();
      managingNodeId = null;
    } catch (reason: unknown) {
      manageError =
        reason instanceof Error
          ? reason.message
          : "Unable to update journal item";
    } finally {
      managing = false;
    }
  }

  async function updateManagedLogo(emoji: string | null) {
    if (!managedNode || managing) return;
    managing = true;
    manageError = "";
    try {
      const updated = await updateJournalNode(managedNode.id, { emoji });
      nodes = nodes.map((node) => (node.id === updated.id ? updated : node));
      manageDialog?.close();
      managingNodeId = null;
    } catch (reason: unknown) {
      manageError =
        reason instanceof Error
          ? reason.message
          : "Unable to update journal logo";
    } finally {
      managing = false;
    }
  }

  function chooseEmoji(emoji: string) {
    void updateManagedLogo(emoji);
  }

  async function removeSelected() {
    if (!managedNode || managing) return;
    managing = true;
    manageError = "";
    const removedId = managedNode.id;
    const removed = descendantIds(removedId);
    removed.add(removedId);
    try {
      await deleteJournalNode(removedId);
      nodes = nodes.filter((node) => !removed.has(node.id));
      manageDialog?.close();
      managingNodeId = null;
      if (selectedId && removed.has(selectedId)) {
        selectedId = null;
        draftContent = "";
        savedContent = "";
        const nextDocument = nodes[0];
        if (nextDocument) setSelectedDocument(nextDocument);
      }
    } catch (reason: unknown) {
      manageError =
        reason instanceof Error
          ? reason.message
          : "Unable to delete journal item";
    } finally {
      managing = false;
    }
  }

  function canMoveTo(nodeId: string, parentId: string | null) {
    if (nodeId === parentId) return false;
    return parentId === null || !descendantIds(nodeId).has(parentId);
  }

  function beginDrag(event: DragEvent, node: JournalNode) {
    if (renamingNodeId === node.id || movingNodeId) {
      event.preventDefault();
      return;
    }
    draggingNodeId = node.id;
    closeItemMenu();
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", node.id);
    }
  }

  function draggedId(event: DragEvent) {
    return draggingNodeId ?? event.dataTransfer?.getData("text/plain") ?? "";
  }

  function allowRootDrop(event: DragEvent) {
    const nodeId = draggedId(event);
    if (!nodeId || !canMoveTo(nodeId, null)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    dragTarget = { nodeId: "root", placement: "inside" };
  }

  function nodeDropPlacement(event: DragEvent) {
    const element = event.currentTarget;
    if (!(element instanceof HTMLElement)) return "inside";
    const bounds = element.getBoundingClientRect();
    const offset = (event.clientY - bounds.top) / bounds.height;
    if (offset < 0.28) return "before";
    if (offset > 0.72) return "after";
    return "inside";
  }

  function allowNodeDrop(event: DragEvent, target: JournalNode) {
    const nodeId = draggedId(event);
    if (!nodeId || nodeId === target.id) return;
    const placement = nodeDropPlacement(event);
    const parentId = placement === "inside" ? target.id : target.parent_id;
    if (!canMoveTo(nodeId, parentId)) return;
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    dragTarget = { nodeId: target.id, placement };
  }

  function leaveDropTarget(event: DragEvent, nodeId: string | "root") {
    const current = event.currentTarget;
    const related = event.relatedTarget;
    if (
      current instanceof HTMLElement &&
      related instanceof Node &&
      current.contains(related)
    ) {
      return;
    }
    if (dragTarget?.nodeId === nodeId) dragTarget = null;
  }

  function dropDestination(
    target: JournalNode,
    placement: DropPlacement,
    movingId: string,
  ) {
    const parentId = placement === "inside" ? target.id : target.parent_id;
    const siblings = nodes
      .filter((node) => node.parent_id === parentId && node.id !== movingId)
      .sort(sortNodes);
    if (placement === "inside") {
      return { parentId, position: siblings.length };
    }
    const targetPosition = siblings.findIndex((node) => node.id === target.id);
    return {
      parentId,
      position: Math.max(0, targetPosition + (placement === "after" ? 1 : 0)),
    };
  }

  async function dropNode(event: DragEvent, target: JournalNode | null) {
    event.preventDefault();
    const nodeId =
      draggingNodeId ?? event.dataTransfer?.getData("text/plain") ?? "";
    const node = nodes.find((candidate) => candidate.id === nodeId);
    const placement = target
      ? dragTarget?.nodeId === target.id
        ? dragTarget.placement
        : nodeDropPlacement(event)
      : "inside";
    draggingNodeId = null;
    dragTarget = null;
    if (!node) return;
    const destination = target
      ? dropDestination(target, placement, node.id)
      : {
          parentId: null,
          position: nodes.filter(
            (candidate) =>
              candidate.parent_id === null && candidate.id !== node.id,
          ).length,
        };
    if (!canMoveTo(node.id, destination.parentId)) return;
    error = "";
    try {
      await moveNode(node, destination.parentId, destination.position);
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to move journal item";
    }
  }

  function endDrag() {
    draggingNodeId = null;
    dragTarget = null;
  }
</script>

<svelte:window onkeydown={handleShortcut} onclick={handleWindowClick} />

{#snippet journalNodeLogo(node: JournalNode)}
  {#if node.emoji}
    <span class="journal-node-emoji" aria-hidden="true">{node.emoji}</span>
  {:else}
    <FileText size={15} strokeWidth={1.7} aria-hidden="true" />
  {/if}
{/snippet}

{#snippet inlineCreateRow(depth: number)}
  <form
    class="journal-inline-form"
    style:--tree-depth={depth}
    onsubmit={submitCreate}
  >
    <span class="tree-spacer" aria-hidden="true"></span>
    <span class="journal-inline-name">
      <FileText size={15} strokeWidth={1.7} aria-hidden="true" />
      <input
        bind:value={inlineCreateName}
        {@attach captureInlineNameInput}
        placeholder="Untitled"
        maxlength="120"
        aria-label="New journal file name"
        onkeydown={(event) => event.key === "Escape" && cancelCreate()}
      />
    </span>
    <button type="submit" disabled={creating} aria-label="Create file">
      <Plus size={15} strokeWidth={2} aria-hidden="true" />
    </button>
    {#if inlineError}
      <small role="alert">{inlineError}</small>
    {/if}
  </form>
{/snippet}

<section class="journal-page product-page" data-od-id="journal-page">
  <header class="journal-header page-header">
    <div>
      <TypedHeading text="$ journal --open" odId="journal-heading" />
      <p>
        {nodes.length} documents · {nodes.filter((node) => node.parent_id)
          .length}
        nested
      </p>
    </div>
  </header>

  {#if error}
    <div class="journal-message" role="status">
      <span>{error}</span><button type="button" onclick={() => (error = "")}
        >Dismiss</button
      >
    </div>
  {/if}

  <div class={["journal-workspace", !explorerOpen && "is-collapsed"]}>
    <aside class="journal-explorer" aria-label="Journal files">
      <div class="journal-explorer-heading">
        <strong>Explorer</strong>
        <button
          type="button"
          aria-label="Collapse journal explorer"
          onclick={() => (explorerOpen = false)}
        >
          <PanelLeftClose size={17} strokeWidth={1.8} aria-hidden="true" />
        </button>
      </div>
      <div class="journal-filter-row">
        <label class="journal-search">
          <Search size={15} strokeWidth={1.8} aria-hidden="true" />
          <span class="sr-only">Search journal paths</span>
          <input
            type="search"
            bind:value={searchQuery}
            placeholder="Filter files…"
          />
        </label>
        <button
          class="journal-root-add"
          type="button"
          aria-label="Add file to journal root"
          title="Add file to journal root"
          onclick={() => beginCreate(null)}
          data-od-id="journal-add-root-file"
        >
          <Plus size={17} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>
      <nav class="journal-tree" aria-label="Journal file tree">
        {#if loading}
          <p class="journal-tree-empty">Loading journal…</p>
        {:else}
          <div
            class={[
              "journal-root-target",
              draggingNodeId && "is-dragging",
              dragTarget?.nodeId === "root" && "is-drop-target",
            ]}
            ondragover={allowRootDrop}
            ondragleave={(event) => leaveDropTarget(event, "root")}
            ondrop={(event) => dropNode(event, null)}
            role="presentation"
          >
            <strong>Journal root</strong>
            <span
              >{draggingNodeId
                ? "Drop to append at root"
                : "Top-level files"}</span
            >
          </div>
          {#if inlineCreateParentId === null}
            {@render inlineCreateRow(0)}
          {/if}
          {#each visibleRows as row (row.node.id)}
            <div
              class={[
                "journal-tree-row",
                selectedId === row.node.id && "is-selected",
                draggingNodeId === row.node.id && "is-dragging",
                dragTarget?.nodeId === row.node.id &&
                  `is-drop-${dragTarget.placement}`,
                movingNodeId === row.node.id && "is-moving",
              ]}
              style:--tree-depth={row.depth}
              oncontextmenu={(event) => openContextMenu(event, row.node)}
              ondragover={(event) => allowNodeDrop(event, row.node)}
              ondragleave={(event) => leaveDropTarget(event, row.node.id)}
              ondrop={(event) => dropNode(event, row.node)}
              role="group"
              aria-label={`${row.node.name}, level ${row.depth + 1}`}
            >
              {#if !searchQuery && (hasChildren(row.node.id) || inlineCreateParentId === row.node.id)}
                <button
                  class="journal-tree-disclosure"
                  type="button"
                  aria-label={`${expandedIds.includes(row.node.id) ? "Collapse" : "Expand"} subfiles of ${row.node.name}`}
                  aria-expanded={expandedIds.includes(row.node.id)}
                  onclick={() => toggleDocument(row.node.id)}
                >
                  {#if expandedIds.includes(row.node.id)}
                    <ChevronDown size={14} aria-hidden="true" />
                  {:else}
                    <ChevronRight size={14} aria-hidden="true" />
                  {/if}
                </button>
              {:else}
                <span class="tree-spacer" aria-hidden="true"></span>
              {/if}
              {#if renamingNodeId === row.node.id}
                <form
                  class="journal-inline-rename"
                  onsubmit={(event) => submitRename(event, row.node)}
                >
                  {@render journalNodeLogo(row.node)}
                  <input
                    bind:value={renameName}
                    {@attach captureInlineNameInput}
                    maxlength="120"
                    aria-label={`Rename ${row.node.name}`}
                    onkeydown={(event) =>
                      event.key === "Escape" && cancelRename()}
                  />
                </form>
              {:else}
                <button
                  class="journal-tree-select"
                  type="button"
                  draggable={movingNodeId === null}
                  onclick={() => selectNode(row.node)}
                  onkeydown={(event) =>
                    openKeyboardContextMenu(event, row.node)}
                  ondragstart={(event) => beginDrag(event, row.node)}
                  ondragend={endDrag}
                >
                  {@render journalNodeLogo(row.node)}
                  <span>{row.node.name}</span>
                </button>
              {/if}
              <div class="journal-row-actions">
                <button
                  class="journal-tree-add"
                  type="button"
                  aria-label={`Add subfile to ${row.node.name}`}
                  title={`Add subfile to ${row.node.name}`}
                  onclick={() => beginCreate(row.node.id)}
                >
                  <Plus size={15} strokeWidth={2} aria-hidden="true" />
                </button>
                <button
                  class="journal-tree-more"
                  type="button"
                  aria-label={`Open actions for ${row.node.name}`}
                  aria-haspopup="menu"
                  aria-expanded={menuNodeId === row.node.id}
                  onclick={(event) => toggleMenu(event, row.node.id)}
                >
                  <MoreHorizontal
                    size={15}
                    strokeWidth={1.8}
                    aria-hidden="true"
                  />
                </button>
                {#if menuNodeId === row.node.id}
                  <div
                    class={[
                      "journal-item-menu",
                      menuPosition && "is-context-menu",
                    ]}
                    role="menu"
                    tabindex="-1"
                    aria-label={`Actions for ${row.node.name}`}
                    {@attach captureItemMenu}
                    onkeydown={handleItemMenuKeydown}
                    style:--menu-x={`${menuPosition?.x ?? 0}px`}
                    style:--menu-y={`${menuPosition?.y ?? 0}px`}
                  >
                    <button
                      type="button"
                      role="menuitem"
                      onclick={() => beginRename(row.node)}
                    >
                      <Pencil size={14} aria-hidden="true" /> Rename
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      onclick={() => openManage(row.node, "logo")}
                    >
                      <SmilePlus size={14} aria-hidden="true" /> Change logo
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      disabled={!canShiftNode(row.node, -1)}
                      onclick={() => shiftNode(row.node, -1)}
                    >
                      <ArrowUp size={14} aria-hidden="true" /> Move up
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      disabled={!canShiftNode(row.node, 1)}
                      onclick={() => shiftNode(row.node, 1)}
                    >
                      <ArrowDown size={14} aria-hidden="true" /> Move down
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      onclick={() => openManage(row.node, "move")}
                    >
                      <FolderInput size={14} aria-hidden="true" /> Move
                    </button>
                    <button
                      class="is-destructive"
                      type="button"
                      role="menuitem"
                      onclick={() => openManage(row.node, "delete")}
                    >
                      <Trash2 size={14} aria-hidden="true" /> Delete
                    </button>
                  </div>
                {/if}
              </div>
            </div>
            {#if inlineCreateParentId === row.node.id}
              {@render inlineCreateRow(row.depth + 1)}
            {/if}
          {:else}
            <div class="journal-tree-empty">
              <p>{searchQuery ? "No matching paths." : "No files yet."}</p>
              {#if !searchQuery}<button
                  type="button"
                  onclick={() => beginCreate(null)}
                  >Create a Markdown file</button
                >{/if}
            </div>
          {/each}
        {/if}
      </nav>
    </aside>

    <main class="journal-document">
      {#if !explorerOpen}
        <button
          class="journal-open-explorer"
          type="button"
          aria-label="Open journal explorer"
          onclick={() => (explorerOpen = true)}
        >
          <PanelLeftOpen size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
      {/if}
      {#if selectedNode}
        <div class="journal-document-bar">
          <div>
            <span>{currentPath}</span><small
              >{dirty ? "Unsaved changes" : "Saved"}</small
            >
          </div>
          <div class="journal-document-controls">
            <div class="journal-view-switch" aria-label="Document view">
              <button
                type="button"
                class:is-active={viewMode === "raw"}
                aria-pressed={viewMode === "raw"}
                onclick={() => (viewMode = "raw")}>Raw</button
              >
              <button
                type="button"
                class:is-active={viewMode === "rendered"}
                aria-pressed={viewMode === "rendered"}
                onclick={() => (viewMode = "rendered")}>Rendered</button
              >
            </div>
            <button
              class="journal-save"
              type="button"
              disabled={!dirty || saving}
              onclick={() => saveCurrent()}
            >
              <Save size={15} strokeWidth={1.8} aria-hidden="true" />
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
        {#if viewMode === "raw"}
          <textarea
            class="journal-editor"
            bind:value={draftContent}
            aria-label={`Edit ${selectedNode.name}`}
            spellcheck="true"
            placeholder="# Start writing…"
            data-od-id="journal-markdown-editor"></textarea>
        {:else}
          <article
            class="journal-preview"
            data-od-id="journal-rendered-preview"
          >
            {#if draftContent.trim()}
              <div
                class="journal-rendered-content"
                {@attach attachRenderedPreview}
              ></div>
            {:else}
              <div class="journal-empty-document">
                <FileText
                  size={30}
                  strokeWidth={1.4}
                  aria-hidden="true"
                /><strong>This document is empty</strong>
                <p>Switch to Raw to begin writing in Markdown.</p>
              </div>
            {/if}
          </article>
        {/if}
        <footer class="journal-statusbar">
          <span>Markdown · GFM</span><span>{wordCount} words</span><span
            >{characterCount} characters</span
          ><span>Ctrl/⌘ + S</span>
        </footer>
      {:else}
        <div class="journal-welcome">
          <div>
            <span>[ NO.FILE ]</span><FileText
              size={36}
              strokeWidth={1.3}
              aria-hidden="true"
            />
          </div>
          <h3>Select a document to begin</h3>
          <p>
            Every file can contain Markdown and any number of nested subfiles.
            Rendered mode supports tables, task lists, strikethrough, fenced
            code, and autolinks.
          </p>
          <button
            class="ui-button ui-button--primary journal-primary"
            type="button"
            onclick={() => beginCreate(null)}
            ><Plus size={16} aria-hidden="true" /> Create file</button
          >
        </div>
      {/if}
    </main>
  </div>

  <dialog
    class={["journal-dialog", manageMode === "logo" && "is-emoji-picker"]}
    aria-label={manageMode === "move"
      ? "Move file"
      : manageMode === "logo"
        ? "Choose emoji"
        : "Delete file"}
    {@attach captureManageDialog}
    onclick={(event) => event.target === manageDialog && manageDialog.close()}
    data-od-id="journal-manage-dialog"
  >
    {#if manageMode !== "logo"}
      <header>
        <div>
          <h2>{manageMode === "move" ? "Move file" : "Delete file"}</h2>
        </div>
        <button
          class="ui-button ui-button--ghost ui-button--icon"
          type="button"
          aria-label="Close item settings"
          onclick={() => manageDialog?.close()}
          ><X size={18} aria-hidden="true" /></button
        >
      </header>
    {/if}
    {#if managedNode}
      {#if manageMode === "move"}
        <form onsubmit={submitManage}>
          <p class="journal-dialog-lead">
            Move <strong>{managedNode.name}</strong> and all of its subfiles.
          </p>
          <label for="journal-manage-parent">Destination</label>
          <select id="journal-manage-parent" bind:value={manageParentId}>
            <option value="">Journal root</option>
            {#each availableParents(managedNode) as document (document.id)}
              <option value={document.id}>{journalPath(document)}</option>
            {/each}
          </select>
          {#if manageError}
            <p class="journal-form-error" role="alert">{manageError}</p>
          {/if}
          <footer>
            <button
              class="ui-button ui-button--secondary journal-secondary"
              type="button"
              onclick={() => manageDialog?.close()}>Cancel</button
            >
            <button
              class="ui-button ui-button--primary journal-primary"
              type="submit"
              disabled={managing}>{managing ? "Moving…" : "Move file"}</button
            >
          </footer>
        </form>
      {:else if manageMode === "logo"}
        <div class="journal-emoji-picker" aria-busy={managing}>
          <label class="journal-emoji-search" for="journal-emoji-search">
            <Search size={16} strokeWidth={1.8} aria-hidden="true" />
            <span class="sr-only">Search emoji</span>
            <input
              id="journal-emoji-search"
              type="search"
              bind:value={emojiSearch}
              {@attach captureEmojiSearchInput}
              maxlength="64"
              autocomplete="off"
              placeholder="Search emoji or paste one…"
              data-od-id="journal-emoji-search"
            />
          </label>

          <div
            class="journal-emoji-tabs"
            role="group"
            aria-label="Emoji categories"
          >
            {#each emojiGroups as group (group.id)}
              <button
                type="button"
                title={group.label}
                aria-label={`${group.label} emojis`}
                class:is-active={activeEmojiGroup === group.id &&
                  !emojiSearch.trim()}
                aria-pressed={activeEmojiGroup === group.id &&
                  !emojiSearch.trim()}
                onclick={() => selectEmojiGroup(group.id)}
              >
                <span aria-hidden="true">{group.icon}</span>
              </button>
            {/each}
          </div>

          <div class="journal-emoji-results-heading">
            <span>
              {#if emojiSearch.trim()}
                Search results
              {:else}
                {emojiGroups.find((group) => group.id === activeEmojiGroup)
                  ?.label ?? "Journal"}
              {/if}
            </span>
            <small>{visibleEmojiOptionCount} options</small>
          </div>

          <div class="journal-emoji-grid" aria-label="Available emoji">
            {#if showDefaultLogoOption}
              <button
                class="is-file-logo"
                class:is-selected={!managedNode.emoji}
                type="button"
                title="Use file logo"
                aria-label="Use file logo"
                aria-pressed={!managedNode.emoji}
                disabled={managing}
                onclick={() => updateManagedLogo(null)}
              >
                <FileText size={22} strokeWidth={1.5} aria-hidden="true" />
              </button>
            {/if}
            {#each visibleEmojiOptions as option (option.emoji)}
              <button
                type="button"
                title={option.label}
                aria-label={option.label}
                aria-pressed={managedNode.emoji === option.emoji}
                class:is-selected={managedNode.emoji === option.emoji}
                disabled={managing}
                onclick={() => chooseEmoji(option.emoji)}
              >
                <span aria-hidden="true">{option.emoji}</span>
              </button>
            {:else}
              <p class="journal-emoji-empty">
                No emoji found. Try another name or paste an emoji.
              </p>
            {/each}
          </div>

          {#if manageError}
            <p class="journal-form-error" role="alert">{manageError}</p>
          {/if}
        </div>
      {:else}
        <div class="journal-delete-confirmation">
          <Trash2 size={24} strokeWidth={1.5} aria-hidden="true" />
          <div>
            <strong>Delete {managedNode.name}?</strong>
            <p>
              This permanently deletes the file and every subfile beneath it.
            </p>
          </div>
          {#if manageError}
            <p class="journal-form-error" role="alert">{manageError}</p>
          {/if}
          <footer>
            <button
              class="ui-button ui-button--secondary journal-secondary"
              type="button"
              onclick={() => manageDialog?.close()}>Cancel</button
            >
            <button
              class="ui-button ui-button--danger journal-danger"
              type="button"
              disabled={managing}
              onclick={removeSelected}
              >{managing ? "Deleting…" : "Delete file"}</button
            >
          </footer>
        </div>
      {/if}
    {/if}
  </dialog>
</section>

<style>
  /* The page fills the canvas so the workspace, and with it the explorer, can take
     the whole column rather than stopping at a fixed height. */
  .journal-page {
    min-width: 0;
    min-height: var(--product-view-height, auto);
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: clamp(24px, 3vw, 42px);
  }
  .journal-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  .journal-header span,
  .journal-welcome span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
  }
  .journal-header p {
    margin-top: 8px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .journal-document-controls,
  .journal-dialog footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  button,
  input,
  select,
  textarea {
    font: inherit;
  }
  button {
    min-height: 42px;
  }
  button:focus-visible,
  input:focus-visible,
  select:focus-visible,
  textarea:focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }
  .journal-primary,
  .journal-secondary,
  .journal-danger,
  .journal-save {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 0 14px;
    border: 1px solid var(--border);
    border-radius: 7px;
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 560;
    letter-spacing: 0.02em;
  }
  .journal-primary {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .journal-primary:hover {
    background: transparent;
    color: var(--fg);
  }
  .journal-secondary,
  .journal-save {
    background: var(--page-surface, var(--surface));
    color: var(--fg);
  }
  .journal-secondary:hover,
  .journal-save:hover:not(:disabled) {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .journal-danger {
    border-color: color-mix(in oklch, var(--fg) 55%, var(--border));
    background: transparent;
    color: var(--fg);
  }
  .journal-danger:hover {
    background: var(--fg);
    color: var(--surface);
  }
  .journal-message {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 11px 13px;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .journal-message button {
    min-height: auto;
    color: var(--fg);
    text-decoration: underline;
  }
  .journal-workspace {
    position: relative;
    min-height: 420px;
    flex: 1;
    display: grid;
    grid-template-columns: minmax(240px, 300px) minmax(0, 1fr);
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    overflow: hidden;
  }
  .journal-workspace.is-collapsed {
    grid-template-columns: 0 minmax(0, 1fr);
  }
  .journal-explorer {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-right: 1px solid var(--border);
    background: color-mix(
      in oklch,
      var(--page-surface, var(--surface)) 96%,
      var(--bg)
    );
    transition: opacity 150ms ease-out;
  }
  .is-collapsed .journal-explorer {
    opacity: 0;
    pointer-events: none;
  }
  .journal-explorer-heading {
    min-height: 54px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px 0 15px;
    border-bottom: 1px solid var(--border);
  }
  .journal-explorer-heading strong {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 560;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .journal-explorer-heading button,
  .journal-open-explorer {
    width: 38px;
    min-height: 38px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .journal-explorer-heading button:hover,
  .journal-open-explorer:hover {
    color: var(--fg);
  }
  .journal-filter-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 44px;
    align-items: center;
    gap: 8px;
    padding: 10px;
  }
  .journal-search {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--muted);
  }
  .journal-search input {
    width: 100%;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .journal-root-add {
    width: 44px;
    min-height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--page-surface, var(--surface));
    color: var(--muted);
  }
  .journal-root-add:hover {
    border-color: var(--fg);
    color: var(--fg);
  }
  .journal-tree {
    min-height: 0;
    flex: 1;
    padding: 0 6px 12px;
    overflow: auto;
    scrollbar-gutter: stable;
  }
  .journal-root-target {
    min-height: 0;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px;
    margin: 2px 2px 6px;
    padding: 0 8px;
    border: 1px dashed transparent;
    border-radius: 5px;
    color: transparent;
    font-family: var(--font-mono);
    font-size: 9px;
    line-height: 24px;
    pointer-events: none;
  }
  .journal-root-target strong {
    font-weight: 560;
  }
  .journal-root-target span {
    overflow: hidden;
    text-align: right;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .journal-root-target.is-dragging {
    border-color: var(--border);
    color: var(--muted);
    pointer-events: auto;
  }
  .journal-root-target.is-drop-target {
    border-color: var(--fg);
    background: var(--fg-soft);
    color: var(--fg);
  }
  .journal-tree-row {
    position: relative;
    min-width: 0;
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) auto;
    align-items: center;
    padding-left: calc(var(--tree-depth) * 16px);
    border: 1px solid transparent;
    border-radius: 5px;
    transition:
      background 120ms ease-out,
      border-color 120ms ease-out,
      opacity 120ms ease-out;
  }
  .journal-tree-row:hover,
  .journal-tree-row.is-selected {
    background: var(--fg-soft);
  }
  .journal-tree-row.is-dragging {
    opacity: 0.34;
  }
  .journal-tree-row.is-drop-inside {
    border-color: var(--fg);
    background: var(--fg-soft);
  }
  .journal-tree-row.is-drop-before,
  .journal-tree-row.is-drop-after {
    background: color-mix(in oklch, var(--fg-soft) 62%, transparent);
  }
  .journal-tree-row.is-drop-before::before,
  .journal-tree-row.is-drop-after::after {
    position: absolute;
    z-index: 2;
    right: 4px;
    left: calc(var(--tree-depth) * 16px + 4px);
    height: 2px;
    border-radius: 999px;
    background: var(--fg);
    content: "";
  }
  .journal-tree-row.is-drop-before::before {
    top: -1px;
  }
  .journal-tree-row.is-drop-after::after {
    bottom: -1px;
  }
  .journal-tree-row.is-moving {
    opacity: 0.58;
    pointer-events: none;
  }
  .journal-tree-select {
    min-width: 0;
    min-height: 34px;
    display: flex;
    align-items: center;
    gap: 6px;
    text-align: left;
  }
  .journal-tree-disclosure {
    width: 28px;
    min-height: 30px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .journal-tree-disclosure:hover {
    color: var(--fg);
  }
  .journal-tree-select > span:last-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .journal-tree-select :global(svg) {
    flex: 0 0 auto;
    color: var(--muted);
  }
  .journal-node-emoji {
    width: 15px;
    flex: 0 0 15px;
    font-family:
      "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif;
    font-size: 15px;
    line-height: 1;
    text-align: center;
  }
  .tree-spacer {
    width: 28px;
  }
  .journal-row-actions {
    position: relative;
    display: flex;
    align-items: center;
    opacity: 0;
    transition: opacity 100ms ease-out;
  }
  .journal-tree-row:hover .journal-row-actions,
  .journal-tree-row:focus-within .journal-row-actions,
  .journal-row-actions:has(.journal-item-menu) {
    opacity: 1;
  }
  .journal-tree-add,
  .journal-tree-more {
    width: 30px;
    min-height: 30px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .journal-tree-add:hover,
  .journal-tree-more:hover {
    color: var(--fg);
  }
  .journal-item-menu {
    position: absolute;
    z-index: 12;
    top: calc(100% - 2px);
    right: 2px;
    width: 152px;
    display: grid;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--page-surface, var(--surface));
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.34);
  }
  .journal-item-menu.is-context-menu {
    position: fixed;
    z-index: 40;
    inset: auto;
    top: var(--menu-y);
    left: var(--menu-x);
  }
  .journal-item-menu button {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 9px;
    border-radius: 4px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    text-align: left;
  }
  .journal-item-menu button:hover,
  .journal-item-menu button:focus-visible {
    background: var(--fg-soft);
    color: var(--fg);
  }
  .journal-item-menu button:disabled {
    opacity: 0.38;
    pointer-events: none;
  }
  .journal-item-menu .is-destructive {
    color: var(--danger);
  }
  .journal-item-menu .is-destructive:hover,
  .journal-item-menu .is-destructive:focus-visible {
    background: color-mix(in oklch, var(--danger) 12%, transparent);
    color: var(--danger);
  }
  .journal-inline-form {
    min-width: 0;
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr) 30px;
    align-items: center;
    padding-left: calc(var(--tree-depth) * 16px);
    border: 1px solid var(--fg);
    border-radius: 5px;
    background: var(--fg-soft);
  }
  .journal-inline-name,
  .journal-inline-rename {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .journal-inline-name :global(svg),
  .journal-inline-rename :global(svg) {
    flex: 0 0 auto;
    color: var(--muted);
  }
  .journal-inline-name input,
  .journal-inline-rename input {
    width: 100%;
    min-width: 0;
    min-height: 32px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .journal-inline-name input::placeholder {
    color: var(--muted);
  }
  .journal-inline-form > button {
    width: 30px;
    min-height: 30px;
    display: grid;
    place-items: center;
    color: var(--fg);
  }
  .journal-inline-form > small {
    grid-column: 2 / -1;
    padding: 3px 4px 7px;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .journal-tree-empty {
    display: grid;
    gap: 8px;
    padding: 18px 10px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .journal-tree-empty button {
    min-height: auto;
    width: fit-content;
    color: var(--fg);
    text-decoration: underline;
  }
  .journal-document {
    position: relative;
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    background: var(--bg);
  }
  .journal-open-explorer {
    position: absolute;
    top: 12px;
    left: 8px;
    z-index: 2;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--page-surface, var(--surface));
  }
  .journal-document-bar {
    min-height: 62px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 8px 13px 8px 18px;
    border-bottom: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
  }
  .is-collapsed .journal-document-bar {
    padding-left: 58px;
  }
  .journal-document-bar > div:first-child {
    min-width: 0;
    display: grid;
    gap: 3px;
  }
  .journal-document-bar span {
    overflow: hidden;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .journal-document-bar small {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .journal-view-switch {
    display: grid;
    grid-template-columns: 1fr 1fr;
    padding: 3px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg);
  }
  .journal-view-switch button {
    min-height: 34px;
    padding: 0 10px;
    border-radius: 4px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .journal-view-switch button.is-active {
    background: var(--fg);
    color: var(--surface);
  }
  .journal-save {
    min-height: 42px;
  }
  .journal-editor {
    width: 100%;
    min-height: 520px;
    resize: none;
    padding: clamp(24px, 4vw, 54px);
    border: 0;
    outline: 0;
    background: var(--bg);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 14px;
    line-height: 1.75;
    tab-size: 2;
  }
  .journal-preview {
    min-height: 520px;
    padding: clamp(30px, 5vw, 70px);
    overflow: auto;
    color: var(--fg);
    font-size: 15px;
    line-height: 1.7;
  }
  .journal-rendered-content {
    max-width: 76ch;
    margin-inline: auto;
  }
  .journal-preview :global(h1),
  .journal-preview :global(h2),
  .journal-preview :global(h3) {
    margin-top: 1.6em;
    margin-bottom: 0.6em;
    font-family: var(--font-display);
    font-weight: 600;
    letter-spacing: -0.02em;
    line-height: 1.2;
  }
  .journal-preview :global(h1) {
    margin-top: 0;
    font-size: 36px;
  }
  .journal-preview :global(h2) {
    padding-bottom: 0.3em;
    border-bottom: 1px solid var(--border);
    font-size: 26px;
  }
  .journal-preview :global(h3) {
    font-size: 20px;
  }
  .journal-preview :global(p),
  .journal-preview :global(ul),
  .journal-preview :global(ol),
  .journal-preview :global(blockquote),
  .journal-preview :global(pre),
  .journal-preview :global(table) {
    margin-top: 1em;
    margin-bottom: 1em;
  }
  .journal-preview :global(a) {
    color: var(--fg);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .journal-preview :global(blockquote) {
    padding-left: 18px;
    border-left: 2px solid var(--border);
    color: var(--muted);
  }
  .journal-preview :global(code) {
    padding: 0.12em 0.35em;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--page-surface, var(--surface));
    font-family: var(--font-mono);
    font-size: 0.88em;
  }
  .journal-preview :global(pre) {
    overflow: auto;
    padding: 18px;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
  }
  .journal-preview :global(pre code) {
    padding: 0;
    border: 0;
    background: transparent;
  }
  .journal-preview :global(table) {
    width: 100%;
    border-collapse: collapse;
  }
  .journal-preview :global(th),
  .journal-preview :global(td) {
    padding: 9px 11px;
    border: 1px solid var(--border);
    text-align: left;
  }
  .journal-preview :global(img) {
    max-width: 100%;
    height: auto;
  }
  .journal-preview :global(input[type="checkbox"]) {
    accent-color: var(--fg);
  }
  .journal-statusbar {
    min-height: 34px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 16px;
    padding: 0 13px;
    border-top: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
  }
  .journal-statusbar span:first-child {
    margin-right: auto;
  }
  .journal-welcome {
    grid-row: 1 / -1;
    min-height: 560px;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 10px;
    padding: 40px;
    text-align: center;
  }
  .journal-welcome > div {
    display: grid;
    place-items: center;
    gap: 12px;
    color: var(--muted);
  }
  .journal-welcome h3 {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.02em;
  }
  .journal-welcome p {
    max-width: 55ch;
    color: var(--muted);
    font-size: 13px;
    line-height: 1.6;
  }
  .journal-welcome button {
    margin-top: 10px;
  }
  .journal-empty-document {
    min-height: 360px;
    display: grid;
    place-items: center;
    align-content: center;
    gap: 8px;
    color: var(--muted);
    text-align: center;
  }
  .journal-empty-document strong {
    color: var(--fg);
    font-family: var(--font-display);
    font-size: 19px;
  }
  .journal-empty-document p {
    font-size: 12px;
  }
  .journal-dialog {
    width: min(540px, calc(100vw - 32px));
    margin: auto;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--page-surface, var(--surface));
    color: var(--fg);
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.48);
  }
  .journal-dialog.is-emoji-picker {
    width: min(500px, calc(100vw - 32px));
    max-height: calc(100dvh - 32px);
    overflow: hidden;
  }
  .journal-dialog::backdrop {
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(7px);
  }
  .journal-dialog header {
    min-height: 76px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }
  .journal-dialog header h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.02em;
    text-transform: capitalize;
  }
  .journal-dialog header > button {
    width: 44px;
    min-height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: 7px;
  }
  .journal-dialog form {
    display: grid;
    gap: 10px;
    padding: 22px;
  }
  .journal-dialog form > label {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.05em;
  }
  .journal-emoji-picker {
    display: grid;
  }
  .journal-emoji-search {
    min-height: 44px;
    display: flex;
    align-items: center;
    gap: 9px;
    margin: 16px 18px 10px;
    padding: 0 11px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg);
    color: var(--muted);
  }
  .journal-emoji-search:focus-within {
    border-color: var(--fg);
  }
  .journal-emoji-search input {
    width: 100%;
    min-width: 0;
    min-height: 42px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-body);
    font-size: 13px;
  }
  .journal-emoji-search input::placeholder {
    color: var(--muted);
  }
  .journal-emoji-search input:focus-visible {
    outline: 0;
  }
  .journal-emoji-tabs {
    display: grid;
    grid-template-columns: repeat(9, minmax(0, 1fr));
    gap: 4px;
    padding: 0 18px 10px;
  }
  .journal-emoji-tabs button {
    min-height: 44px;
    min-width: 0;
    padding: 0;
    border: 1px solid transparent;
    border-radius: 6px;
    color: var(--muted);
  }
  .journal-emoji-tabs button span {
    font-family:
      "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif;
    font-size: 14px;
    line-height: 1;
  }
  .journal-emoji-tabs button:hover,
  .journal-emoji-tabs button:focus-visible {
    border-color: var(--border);
    color: var(--fg);
  }
  .journal-emoji-tabs button.is-active {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }
  .journal-emoji-results-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 18px 9px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }
  .journal-emoji-results-heading small {
    font-size: 9px;
    letter-spacing: 0.02em;
    text-transform: none;
  }
  .journal-emoji-grid {
    height: clamp(124px, 48dvh, 360px);
    display: grid;
    grid-template-columns: repeat(8, minmax(0, 1fr));
    align-content: start;
    gap: 4px;
    padding: 0 14px 14px;
    overflow-y: auto;
    scrollbar-gutter: stable;
    scrollbar-color: var(--border) transparent;
    scrollbar-width: thin;
  }
  .journal-emoji-grid::-webkit-scrollbar {
    width: 6px;
  }
  .journal-emoji-grid::-webkit-scrollbar-track {
    background: transparent;
  }
  .journal-emoji-grid::-webkit-scrollbar-thumb {
    border-radius: 999px;
    background: var(--border);
  }
  .journal-emoji-grid::-webkit-scrollbar-button {
    width: 0;
    height: 0;
    display: none;
  }
  .journal-emoji-grid button {
    min-width: 0;
    min-height: 44px;
    display: grid;
    place-items: center;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
  }
  .journal-emoji-grid button span {
    font-family:
      "Apple Color Emoji", "Segoe UI Emoji", "Noto Color Emoji", sans-serif;
    font-size: 23px;
    line-height: 1;
  }
  .journal-emoji-grid button:hover,
  .journal-emoji-grid button:focus-visible {
    border-color: var(--border);
    background: var(--fg-soft);
  }
  .journal-emoji-grid button.is-selected {
    border-color: var(--fg);
    background: var(--fg-soft);
  }
  .journal-emoji-grid button.is-file-logo {
    color: var(--muted);
  }
  .journal-emoji-grid button.is-file-logo:hover,
  .journal-emoji-grid button.is-file-logo:focus-visible,
  .journal-emoji-grid button.is-file-logo.is-selected {
    color: var(--fg);
  }
  .journal-emoji-empty {
    grid-column: 1 / -1;
    align-self: center;
    padding: 42px 20px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.6;
    text-align: center;
  }
  .journal-emoji-picker > .journal-form-error {
    margin: 0 18px 14px;
  }
  .journal-dialog select {
    width: 100%;
    min-height: 44px;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .journal-dialog-lead {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.55;
  }
  .journal-dialog-lead strong {
    color: var(--fg);
    font-weight: 560;
  }
  .journal-dialog footer {
    justify-content: flex-end;
    margin-top: 8px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  .journal-dialog footer .journal-danger {
    margin-right: auto;
  }
  .journal-delete-confirmation {
    display: grid;
    grid-template-columns: 30px minmax(0, 1fr);
    gap: 12px;
    padding: 22px;
  }
  .journal-delete-confirmation > :global(svg) {
    margin-top: 1px;
    color: var(--muted);
  }
  .journal-delete-confirmation strong {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
  }
  .journal-delete-confirmation p {
    margin-top: 6px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    line-height: 1.55;
  }
  .journal-delete-confirmation .journal-form-error,
  .journal-delete-confirmation footer {
    grid-column: 1 / -1;
  }
  .journal-delete-confirmation footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 6px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  .journal-form-error {
    padding: 10px;
    border: 1px solid color-mix(in oklch, var(--fg) 28%, var(--border));
    background: var(--fg-soft);
    color: var(--fg);
    font-size: 11px;
  }
  @media (max-width: 920px) {
    .journal-page {
      padding: 20px 16px;
    }
    .journal-header {
      align-items: start;
      flex-direction: column;
    }
    .journal-workspace {
      grid-template-columns: minmax(210px, 250px) minmax(0, 1fr);
    }
    .journal-document-bar {
      align-items: start;
      flex-direction: column;
    }
    .journal-document-controls {
      width: 100%;
    }
    .journal-view-switch {
      flex: 1;
    }
  }
  @media (max-width: 680px) {
    .journal-workspace {
      min-height: 560px;
      grid-template-columns: 1fr;
    }
    .journal-explorer {
      position: absolute;
      z-index: 4;
      top: 0;
      bottom: 0;
      width: min(320px, calc(100vw - 32px));
      box-shadow: 22px 0 50px rgba(0, 0, 0, 0.25);
    }
    .journal-workspace.is-collapsed {
      grid-template-columns: 1fr;
    }
    .journal-document {
      grid-column: 1;
    }
    .journal-statusbar {
      gap: 9px;
      overflow: auto;
      justify-content: start;
    }
    .journal-statusbar span:first-child {
      margin-right: 0;
    }
    .journal-dialog footer {
      flex-wrap: wrap;
    }
    .journal-dialog footer button {
      flex: 1;
    }
    .journal-dialog footer .journal-danger {
      flex-basis: 100%;
      margin-right: 0;
    }
    .journal-emoji-grid {
      grid-template-columns: repeat(6, minmax(0, 1fr));
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .journal-explorer {
      transition: none;
    }
  }
</style>

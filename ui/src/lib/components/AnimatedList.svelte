<script lang="ts" generics="Item">
  import type { Snippet } from "svelte";
  import { SvelteSet } from "svelte/reactivity";

  type Props = {
    items: Item[];
    children: Snippet<[item: Item, index: number, selected: boolean]>;
    getKey?: (item: Item, index: number) => string;
    onItemSelect?: (item: Item, index: number) => void;
    showGradients?: boolean;
    enableArrowNavigation?: boolean;
    class?: string;
    itemClass?: string;
    displayScrollbar?: boolean;
    initialSelectedIndex?: number;
  };

  let {
    items,
    children,
    getKey = (_item, index) => String(index),
    onItemSelect,
    showGradients = true,
    enableArrowNavigation = true,
    class: className = "",
    itemClass = "",
    displayScrollbar = true,
    initialSelectedIndex = -1,
  }: Props = $props();

  let listElement = $state<HTMLDivElement>();
  let selectedIndex = $state(-1);
  let topGradientOpacity = $state(0);
  let bottomGradientOpacity = $state(0);
  const visibleKeys = new SvelteSet<string>();

  function itemKey(item: Item, index: number) {
    return getKey(item, index);
  }

  function updateGradients(container: HTMLDivElement) {
    topGradientOpacity = Math.min(container.scrollTop / 50, 1);
    const bottomDistance =
      container.scrollHeight - (container.scrollTop + container.clientHeight);
    bottomGradientOpacity =
      container.scrollHeight <= container.clientHeight
        ? 0
        : Math.min(bottomDistance / 50, 1);
  }

  function captureList(node: HTMLDivElement) {
    listElement = node;
    selectedIndex = initialSelectedIndex;
    if (enableArrowNavigation) node.tabIndex = 0;
    updateGradients(node);
    return () => {
      if (listElement === node) listElement = undefined;
    };
  }

  function watchVisibility(node: HTMLElement, key: string) {
    if (
      typeof IntersectionObserver === "undefined" ||
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      visibleKeys.add(key);
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && entry.intersectionRatio >= 0.15) {
          visibleKeys.add(key);
        } else {
          visibleKeys.delete(key);
        }
      },
      { threshold: [0, 0.15, 0.55, 1] },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }

  function selectItem(index: number) {
    selectedIndex = Math.max(0, Math.min(index, items.length - 1));
    const selected = items[selectedIndex];
    if (selected !== undefined) onItemSelect?.(selected, selectedIndex);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!enableArrowNavigation || items.length === 0) return;

    if (event.key === "ArrowDown") {
      event.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, items.length - 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
    } else if (event.key === "Enter" && selectedIndex >= 0) {
      event.preventDefault();
      selectItem(selectedIndex);
    } else {
      return;
    }

    listElement
      ?.querySelector<HTMLElement>(`[data-animated-index="${selectedIndex}"]`)
      ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  }
</script>

<div
  class={[
    "animated-list",
    displayScrollbar ? "has-scrollbar" : "hides-scrollbar",
    className,
  ]}
  role={enableArrowNavigation ? "listbox" : "list"}
  onkeydown={handleKeydown}
  onscroll={(event) => updateGradients(event.currentTarget)}
  {@attach captureList}
>
  {#each items as item, index (itemKey(item, index))}
    {@const key = itemKey(item, index)}
    <div
      class={[
        "animated-list-item",
        enableArrowNavigation && selectedIndex === index && "is-selected",
        visibleKeys.has(key) && "is-visible",
        itemClass,
      ]}
      data-animated-index={index}
      role={enableArrowNavigation ? "option" : "listitem"}
      aria-selected={enableArrowNavigation ? selectedIndex === index : undefined}
      style:--animated-list-delay={`${Math.min(index * 14, 98)}ms`}
      onmouseenter={() => {
        if (enableArrowNavigation) selectedIndex = index;
      }}
      onclick={() => {
        if (enableArrowNavigation) selectItem(index);
      }}
      {@attach (node) => watchVisibility(node, key)}
    >
      {@render children(item, index, selectedIndex === index)}
    </div>
  {/each}

  {#if showGradients}
    <div
      class="animated-list-gradient is-top"
      style:opacity={topGradientOpacity}
      aria-hidden="true"
    ></div>
    <div
      class="animated-list-gradient is-bottom"
      style:opacity={bottomGradientOpacity}
      aria-hidden="true"
    ></div>
  {/if}
</div>

<style>
  .animated-list {
    position: relative;
    width: 100%;
    min-width: 0;
  }

  .animated-list:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
  }

  .animated-list-item {
    width: 100%;
    opacity: 0;
    transform: translateY(12px) scale(0.96);
    transform-origin: center top;
    transition:
      transform 80ms var(--ease-out) var(--animated-list-delay),
      opacity 67ms ease-out var(--animated-list-delay);
  }

  .animated-list-item.is-visible {
    opacity: 1;
    transform: translateY(0) scale(1);
  }

  .animated-list-item.is-selected {
    box-shadow: inset 2px 0 var(--accent);
  }

  .animated-list-gradient {
    position: sticky;
    z-index: 2;
    left: 0;
    width: 100%;
    height: 50px;
    pointer-events: none;
    transition: opacity 100ms ease;
  }

  .animated-list-gradient.is-top {
    top: 0;
    margin-bottom: -50px;
    background: linear-gradient(
      to bottom,
      color-mix(in oklch, var(--bg) 92%, transparent),
      transparent
    );
  }

  .animated-list-gradient.is-bottom {
    bottom: 0;
    height: 76px;
    margin-top: -76px;
    background: linear-gradient(
      to top,
      color-mix(in oklch, var(--bg) 92%, transparent),
      transparent
    );
  }

  .has-scrollbar {
    scrollbar-gutter: stable;
  }

  .hides-scrollbar {
    scrollbar-width: none;
  }

  .hides-scrollbar::-webkit-scrollbar {
    display: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .animated-list-item,
    .animated-list-item.is-visible {
      opacity: 1;
      transform: none;
      transition: none;
    }

    .animated-list-gradient {
      transition: none;
    }
  }
</style>

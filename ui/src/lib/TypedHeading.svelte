<script module lang="ts">
  /**
   * The characters currently visible in the newest heading instance.
   *
   * Product pages are mounted from the shell's `{#if activeSection}` chain, so moving
   * between sections can build the incoming heading before the outgoing one is destroyed.
   * The ownership token prevents that outgoing instance from overwriting the incoming
   * handoff while its final frame or teardown runs.
   */
  let visibleText = "";
  let newestOwner = 0;
</script>

<script lang="ts">
  import { browser } from "$app/environment";
  import { onDestroy, untrack } from "svelte";
  import { MediaQuery } from "svelte/reactivity";

  let {
    text,
    odId,
  }: {
    /** The finished title, for example `$ walls --collection`. */
    text: string;
    odId?: string;
  } = $props();

  /**
   * Budgets rather than per-character delays, so a long title takes the same time as a
   * short one and the whole swap stays inside the band for a navigation transition.
   */
  const ERASE_MS = 170;
  const TYPE_MS = 300;

  const reducedMotion = new MediaQuery("(prefers-reduced-motion: reduce)");

  const owner = browser ? ++newestOwner : 0;
  // The first page starts empty so it performs the same typing phase as later titles.
  // Navigation starts from the exact characters the outgoing instance last painted.
  const initialText = browser ? visibleText : "";
  let displayed = $state(initialText);
  let typing = $state(false);
  let frame: number | undefined;
  /** Invalidates an in-flight run when the title changes again mid-animation. */
  let run = 0;

  $effect(() => {
    // Only the incoming title is a dependency. `retype` reads and writes `displayed`,
    // and tracking that would restart the animation on its own first character.
    const target = text;
    const shouldReduceMotion = reducedMotion.current;
    untrack(() => retype(target, shouldReduceMotion));
  });

  onDestroy(() => {
    run += 1;
    stopFrame();
    if (browser && owner === newestOwner) visibleText = displayed;
  });

  function commonPrefix(from: string, to: string) {
    const limit = Math.min(from.length, to.length);
    let index = 0;
    while (index < limit && from[index] === to[index]) index += 1;
    return index;
  }

  function show(next: string) {
    displayed = next;
    if (browser && owner === newestOwner) visibleText = next;
  }

  function settle(target: string) {
    show(target);
    typing = false;
  }

  function stopFrame() {
    if (browser && frame !== undefined) cancelAnimationFrame(frame);
    frame = undefined;
  }

  function retype(target: string, shouldReduceMotion: boolean) {
    run += 1;
    const token = run;
    stopFrame();
    if (displayed === target) {
      settle(target);
      return;
    }
    if (shouldReduceMotion) {
      settle(target);
      return;
    }

    // Erase only as far as the titles diverge: every page title opens with the same
    // prompt, and clearing it just to retype it reads as a stutter.
    const from = displayed;
    const shared = commonPrefix(from, target);
    const eraseSteps = from.length - shared;
    const typeSteps = target.length - shared;
    const eraseDuration = eraseSteps > 0 ? ERASE_MS : 0;
    const typeDuration = typeSteps > 0 ? TYPE_MS : 0;
    const startedAt = performance.now();
    typing = true;

    // Both phases share a clock. If page rendering delays a frame, carry that
    // elapsed time into typing instead of starting another full budget afterward.
    const tick = (now: number) => {
      if (token !== run || owner !== newestOwner) return;
      const elapsed = Math.max(0, now - startedAt);

      if (elapsed >= eraseDuration + typeDuration) {
        frame = undefined;
        settle(target);
        return;
      }

      if (elapsed < eraseDuration) {
        const completed = Math.floor((elapsed / eraseDuration) * eraseSteps);
        show(from.slice(0, from.length - completed));
      } else {
        const completed = Math.floor(
          ((elapsed - eraseDuration) / typeDuration) * typeSteps,
        );
        show(target.slice(0, shared + completed));
      }

      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
  }
</script>

<!-- The animated characters are decorative: assistive technology reads the finished
     title from `aria-label` rather than whatever the caret is part-way through. -->
<h2 class="typed-heading" aria-label={text} data-od-id={odId}>
  <span aria-hidden="true">{displayed}</span><span
    class="typed-caret"
    class:is-typing={typing}
    aria-hidden="true"
  ></span>
</h2>

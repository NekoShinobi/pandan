import { tick } from "svelte";

export type ViewSwapPhase = "leaving" | "entering";
export type ViewSwapDirection = "forward" | "backward";

type ViewSwapRun = {
  /** Forward slides the incoming view in from the right, backward from the left. */
  forward: boolean;
  /** Applies the state change the transition covers. Runs once, at the swap point. */
  commit: () => void;
  /** Work the incoming view needs; awaited while the outgoing view leaves. */
  pending?: Promise<unknown> | null;
};

const LEAVE_MS = 150;
const ENTER_MS = 260;
const HEIGHT_MS = 280;
const PENDING_GRACE_MS = 240;
const HEIGHT_EASING = "cubic-bezier(0.2, 0, 0, 1)";

function waitFor(milliseconds: number) {
  return new Promise<void>((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

/**
 * Drives the page-level view swap shared by Tasks and Kanban. The container
 * publishes its phase and direction through `data-view-phase` and
 * `data-view-direction`, which the `.view-swap` rules in `app.css` animate,
 * and the container height is tweened across the swap so the layout never
 * jumps. Reduced motion falls back to the plain instant swap.
 */
export function createViewSwap() {
  let phase = $state<ViewSwapPhase | undefined>();
  let direction = $state<ViewSwapDirection>("forward");
  let container: HTMLElement | undefined;
  let token = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function animateHeight(fromHeight: number) {
    if (!container || !fromHeight) return;
    if (typeof container.animate !== "function") return;
    const toHeight = container.offsetHeight;
    if (Math.abs(toHeight - fromHeight) < 4) return;
    container.animate(
      [{ height: `${fromHeight}px` }, { height: `${toHeight}px` }],
      { duration: HEIGHT_MS, easing: HEIGHT_EASING },
    );
  }

  return {
    get phase() {
      return phase;
    },
    get direction() {
      return direction;
    },

    /** Attach to the element wrapping the content that swaps. */
    attach(node: HTMLElement) {
      container = node;
      return () => {
        if (container === node) container = undefined;
      };
    },

    async run({ forward, commit, pending = null }: ViewSwapRun) {
      const current = ++token;
      const settled = pending?.catch(() => undefined) ?? null;

      if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
        commit();
        await settled;
        return;
      }

      clearTimeout(timer);
      timer = undefined;
      direction = forward ? "forward" : "backward";
      phase = "leaving";
      const fromHeight = container?.offsetHeight ?? 0;

      // Let pending work settle while the outgoing view fades, so the swap
      // usually resolves straight into content instead of flashing a loading
      // state. Slow work still hands over on time rather than holding the
      // view blank.
      await Promise.race([
        Promise.all([waitFor(LEAVE_MS), settled]),
        waitFor(LEAVE_MS + PENDING_GRACE_MS),
      ]);
      if (current !== token) return;

      commit();
      phase = "entering";
      await tick();
      if (current !== token) return;

      animateHeight(fromHeight);
      timer = setTimeout(() => {
        if (current !== token) return;
        phase = undefined;
        timer = undefined;
      }, ENTER_MS);
    },

    /** Drops an in-flight transition, for view resets and teardown. */
    cancel() {
      token += 1;
      clearTimeout(timer);
      timer = undefined;
      phase = undefined;
    },
  };
}

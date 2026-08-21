import { animate } from "motion";
import { tick } from "svelte";
import { MOTION_EASE, motionPause } from "$lib/motion.svelte";

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
type MotionControls = {
  finished: Promise<unknown>;
  stop: () => void;
};

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
  let heightAnimation: MotionControls | undefined;
  let heightAnimationNode: HTMLElement | undefined;
  let heightAnimationOriginalHeight = "";

  function stopHeightAnimation() {
    heightAnimation?.stop();
    if (heightAnimationNode) {
      heightAnimationNode.style.height = heightAnimationOriginalHeight;
    }
    heightAnimation = undefined;
    heightAnimationNode = undefined;
    heightAnimationOriginalHeight = "";
  }

  function animateHeight(fromHeight: number) {
    if (!container || !fromHeight) return;
    const toHeight = container.offsetHeight;
    if (Math.abs(toHeight - fromHeight) < 4) return;
    stopHeightAnimation();
    const node = container;
    heightAnimationNode = node;
    heightAnimationOriginalHeight = node.style.height;
    const controls = animate(
      node,
      { height: [`${fromHeight}px`, `${toHeight}px`] },
      { duration: HEIGHT_MS / 1_000, ease: MOTION_EASE },
    ) as MotionControls;
    heightAnimation = controls;
    void controls.finished
      .then(() => {
        if (heightAnimation === controls) stopHeightAnimation();
      })
      .catch(() => undefined);
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

      direction = forward ? "forward" : "backward";
      phase = "leaving";
      const fromHeight = container?.offsetHeight ?? 0;

      // Let pending work settle while the outgoing view fades, so the swap
      // usually resolves straight into content instead of flashing a loading
      // state. Slow work still hands over on time rather than holding the
      // view blank.
      await Promise.race([
        Promise.all([motionPause(LEAVE_MS), settled]),
        motionPause(LEAVE_MS + PENDING_GRACE_MS),
      ]);
      if (current !== token) return;

      commit();
      phase = "entering";
      await tick();
      if (current !== token) return;

      animateHeight(fromHeight);
      await motionPause(ENTER_MS);
      if (current === token) phase = undefined;
    },

    /** Drops an in-flight transition, for view resets and teardown. */
    cancel() {
      token += 1;
      stopHeightAnimation();
      phase = undefined;
    },
  };
}

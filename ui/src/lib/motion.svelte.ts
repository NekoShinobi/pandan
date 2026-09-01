import { animate } from "motion";
import { prefersReducedMotion } from "svelte/motion";

export const MOTION_EASE = [0.2, 0, 0, 1] as const;

type MotionControls = {
  finished: Promise<unknown>;
  stop: () => void;
};

type DisclosureOptions = {
  duration?: number;
};

type PopoverOptions = {
  closedY?: number;
  duration?: number;
  onExitComplete?: () => void;
};

type SurfaceOptions = {
  y?: number;
  duration?: number;
};

const surfaceAnimations = new WeakMap<HTMLElement, MotionControls>();

function controlsFor(value: unknown): MotionControls {
  return value as MotionControls;
}

function settleDisclosure(element: HTMLElement, open: boolean) {
  element.style.height = open ? "auto" : "0px";
  element.style.opacity = open ? "1" : "0";
}

/**
 * Height-and-opacity disclosure shared by accordions and expandable detail regions.
 * Keep the attached shell mounted and pair it with `aria-hidden` plus `inert`.
 */
export function motionDisclosure(
  open: boolean,
  { duration = 0.22 }: DisclosureOptions = {},
) {
  return (element: HTMLElement) => {
    const initialized = element.dataset.motionDisclosure === "ready";
    element.dataset.motionDisclosure = "ready";
    element.style.overflow = "hidden";

    if (prefersReducedMotion.current || !initialized) {
      settleDisclosure(element, open);
      return;
    }

    const fromHeight = element.getBoundingClientRect().height;
    const toHeight = open ? element.scrollHeight : 0;
    const computedOpacity = Number.parseFloat(
      window.getComputedStyle(element).opacity,
    );
    const fromOpacity = Number.isNaN(computedOpacity)
      ? open
        ? 0
        : 1
      : computedOpacity;
    const toOpacity = open ? 1 : 0;

    if (
      Math.abs(fromHeight - toHeight) < 0.5 &&
      Math.abs(fromOpacity - toOpacity) < 0.01
    ) {
      settleDisclosure(element, open);
      return;
    }

    let cancelled = false;
    const controls = controlsFor(
      animate(
        element,
        {
          height: [`${fromHeight}px`, `${toHeight}px`],
          opacity: [fromOpacity, toOpacity],
        },
        { duration, ease: MOTION_EASE },
      ),
    );

    void controls.finished
      .then(() => {
        if (!cancelled) settleDisclosure(element, open);
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
      controls.stop();
    };
  };
}

function settlePopover(element: HTMLElement, open: boolean, closedY: number) {
  element.style.opacity = open ? "1" : "0";
  element.style.transform = `translateY(${open ? 0 : closedY}px)`;
  element.style.visibility = open ? "visible" : "hidden";
  element.style.pointerEvents = open ? "auto" : "none";
}

/**
 * Shared floating-surface motion. The surface stays mounted so its exit can finish.
 */
export function motionPopover(
  open: boolean,
  { closedY = 6, duration = 0.18, onExitComplete }: PopoverOptions = {},
) {
  return (element: HTMLElement) => {
    const initialized = element.dataset.motionPopover === "ready";
    element.dataset.motionPopover = "ready";

    if (prefersReducedMotion.current || !initialized) {
      settlePopover(element, open, closedY);
      if (!open) onExitComplete?.();
      return;
    }

    let cancelled = false;
    if (open) {
      element.style.visibility = "visible";
      element.style.pointerEvents = "auto";
    } else {
      element.style.pointerEvents = "none";
    }

    const controls = controlsFor(
      animate(
        element,
        {
          opacity: open ? 1 : 0,
          transform: `translateY(${open ? 0 : closedY}px)`,
        },
        { duration, ease: MOTION_EASE },
      ),
    );

    void controls.finished
      .then(() => {
        if (!cancelled) {
          settlePopover(element, open, closedY);
          if (!open) onExitComplete?.();
        }
      })
      .catch(() => undefined);

    return () => {
      cancelled = true;
      controls.stop();
    };
  };
}

/** Runs the entrance used by temporary inline composers. */
export function motionSurfaceEnter({
  y = 8,
  duration = 0.18,
}: SurfaceOptions = {}) {
  return (element: HTMLElement) => {
    if (prefersReducedMotion.current) {
      element.style.opacity = "1";
      element.style.transform = "translateY(0px)";
      return;
    }

    const controls = controlsFor(
      animate(
        element,
        {
          opacity: [0, 1],
          transform: [`translateY(${y}px)`, "translateY(0px)"],
        },
        { duration, ease: MOTION_EASE },
      ),
    );
    surfaceAnimations.set(element, controls);

    return () => {
      if (surfaceAnimations.get(element) === controls) {
        surfaceAnimations.delete(element);
        controls.stop();
      }
    };
  };
}

/** Lets state removal wait for a temporary surface's Motion exit. */
export async function motionSurfaceExit(
  element: HTMLElement | undefined,
  { y = 6, duration = 0.14 }: SurfaceOptions = {},
) {
  if (!element || prefersReducedMotion.current) return;
  surfaceAnimations.get(element)?.stop();
  element.style.pointerEvents = "none";
  const controls = controlsFor(
    animate(
      element,
      { opacity: 0, transform: `translateY(${y}px)` },
      { duration, ease: [0.4, 0, 1, 1] },
    ),
  );
  surfaceAnimations.set(element, controls);
  await controls.finished.catch(() => undefined);
  if (surfaceAnimations.get(element) === controls) {
    surfaceAnimations.delete(element);
  }
}

/** Motion-backed timing for view choreography, avoiding timer-coupled DOM state. */
export async function motionPause(milliseconds: number) {
  const controls = controlsFor(
    animate(0, 1, { duration: milliseconds / 1_000, ease: "linear" }),
  );
  await controls.finished.catch(() => undefined);
}

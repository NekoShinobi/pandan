<script lang="ts">
  import { animate } from "motion";

  type AnimationSnapshot = Record<string, string | number>;

  type Props = {
    text?: string;
    delay?: number;
    class?: string;
    animateBy?: "words" | "letters";
    direction?: "top" | "bottom";
    threshold?: number;
    rootMargin?: string;
    animationFrom?: AnimationSnapshot;
    animationTo?: AnimationSnapshot[];
    easing?: string | number[] | ((time: number) => number);
    onAnimationComplete?: () => void;
    stepDuration?: number;
  };

  let {
    text = "",
    delay = 200,
    class: className = "",
    animateBy = "words",
    direction = "top",
    threshold = 0.1,
    rootMargin = "0px",
    animationFrom,
    animationTo,
    easing = (time: number) => time,
    onAnimationComplete,
    stepDuration = 0.35,
  }: Props = $props();

  let completionReported = false;

  let segments = $derived(
    animateBy === "words" ? text.split(" ") : text.split(""),
  );
  let defaultFrom = $derived<AnimationSnapshot>(
    direction === "top"
      ? { filter: "blur(10px)", opacity: 0, y: -42 }
      : { filter: "blur(10px)", opacity: 0, y: 42 },
  );
  let defaultTo = $derived<AnimationSnapshot[]>([
    {
      filter: "blur(5px)",
      opacity: 0.5,
      y: direction === "top" ? 4 : -4,
    },
    { filter: "blur(0px)", opacity: 1, y: 0 },
  ]);
  let fromSnapshot = $derived(animationFrom ?? defaultFrom);
  let toSnapshots = $derived(animationTo ?? defaultTo);

  function animateSegment(
    element: HTMLSpanElement,
    index: number,
    from: AnimationSnapshot,
    steps: AnimationSnapshot[],
    segmentCount: number,
  ) {
    applyInitialSnapshot(element, from);

    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      applyInitialSnapshot(element, steps.at(-1) ?? from);
      if (index === segmentCount - 1 && !completionReported) {
        completionReported = true;
        queueMicrotask(() => onAnimationComplete?.());
      }
      return;
    }

    type AnimationControls = {
      finished?: Promise<unknown>;
      stop?: () => void;
      cancel?: () => void;
    };
    let controls: AnimationControls | undefined;
    let animationFrame = 0;
    let observer: IntersectionObserver | undefined;

    const startAnimation = () => {
      if (controls) return;
      const stepCount = steps.length + 1;
      const totalDuration = stepDuration * (stepCount - 1);
      const times = Array.from({ length: stepCount }, (_, stepIndex) =>
        stepCount === 1 ? 0 : stepIndex / (stepCount - 1),
      );
      const keyframes = buildKeyframes(from, steps);
      const targetKeyframes: Record<string, Array<string | number>> = {};
      for (const [key, frames] of Object.entries(keyframes)) {
        targetKeyframes[key === "y" ? "transform" : key] =
          key === "y"
            ? frames.map((value) =>
                `translateY(${typeof value === "number" ? `${value}px` : value})`,
              )
            : frames;
      }

      controls = animate(element, targetKeyframes as never, {
        duration: totalDuration,
        times,
        delay: (index * delay) / 1_000,
        ease: easing as never,
      }) as unknown as AnimationControls;

      if (index === segmentCount - 1 && !completionReported) {
        controls.finished
          ?.then(() => {
            if (completionReported) return;
            completionReported = true;
            onAnimationComplete?.();
          })
          .catch(() => undefined);
      }
    };

    if (threshold <= 0) {
      animationFrame = requestAnimationFrame(startAnimation);
    } else {
      observer = new IntersectionObserver(
        ([entry]) => {
          if (!entry.isIntersecting) return;
          observer?.unobserve(element);
          startAnimation();
        },
        { threshold, rootMargin },
      );
      observer.observe(element);
    }

    return () => {
      cancelAnimationFrame(animationFrame);
      observer?.disconnect();
      controls?.stop?.();
      controls?.cancel?.();
    };
  }

  function buildKeyframes(
    from: AnimationSnapshot,
    steps: AnimationSnapshot[],
  ) {
    const keys = new Set([
      ...Object.keys(from),
      ...steps.flatMap((step) => Object.keys(step)),
    ]);
    const keyframes: Record<string, Array<string | number>> = {};
    keys.forEach((key) => {
      keyframes[key] = [from[key], ...steps.map((step) => step[key])];
    });
    return keyframes;
  }

  function applyInitialSnapshot(
    element: HTMLElement,
    snapshot: AnimationSnapshot,
  ) {
    const transforms: string[] = [];
    for (const [key, value] of Object.entries(snapshot)) {
      if (key === "y") {
        transforms.push(
          `translateY(${typeof value === "number" ? `${value}px` : value})`,
        );
      } else if (key === "x") {
        transforms.push(
          `translateX(${typeof value === "number" ? `${value}px` : value})`,
        );
      } else if (key === "filter") {
        element.style.filter = String(value);
      } else if (key === "opacity") {
        element.style.opacity = String(value);
      } else {
        (element.style as unknown as Record<string, string>)[key] = String(value);
      }
    }
    if (transforms.length) element.style.transform = transforms.join(" ");
  }
</script>

<p class={["blur-text", className]}>
  {#each segments as segment, index (`${index}-${segment}`)}
    <span
      style:display="inline-block"
      style:will-change="transform, filter, opacity"
      {@attach (element) =>
        animateSegment(
          element,
          index,
          fromSnapshot,
          toSnapshots,
          segments.length,
        )}
    >
      {segment === " " ? "\u00A0" : segment}{animateBy === "words" &&
      index < segments.length - 1
        ? "\u00A0"
        : ""}
    </span>
  {/each}
</p>

<style>
  .blur-text {
    display: flex;
    flex-wrap: wrap;
    margin: 0;
  }
</style>

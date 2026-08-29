<script lang="ts">
  import Bell from "lucide-svelte/icons/bell";
  import BookOpen from "lucide-svelte/icons/book-open";
  import Bookmark from "lucide-svelte/icons/bookmark";
  import Briefcase from "lucide-svelte/icons/briefcase";
  import CalendarDays from "lucide-svelte/icons/calendar-days";
  import Cloud from "lucide-svelte/icons/cloud";
  import Code from "lucide-svelte/icons/code";
  import Database from "lucide-svelte/icons/database";
  import Folder from "lucide-svelte/icons/folder";
  import Gamepad2 from "lucide-svelte/icons/gamepad-2";
  import GitBranch from "lucide-svelte/icons/git-branch";
  import Globe from "lucide-svelte/icons/globe";
  import Heart from "lucide-svelte/icons/heart";
  import House from "lucide-svelte/icons/house";
  import ImageIcon from "lucide-svelte/icons/image";
  import Link from "lucide-svelte/icons/link";
  import Lock from "lucide-svelte/icons/lock";
  import Mail from "lucide-svelte/icons/mail";
  import Music from "lucide-svelte/icons/music";
  import PanelTop from "lucide-svelte/icons/panel-top";
  import Podcast from "lucide-svelte/icons/podcast";
  import Rocket from "lucide-svelte/icons/rocket";
  import Rss from "lucide-svelte/icons/rss";
  import ShoppingBag from "lucide-svelte/icons/shopping-bag";
  import Star from "lucide-svelte/icons/star";
  import Terminal from "lucide-svelte/icons/terminal";
  import Video from "lucide-svelte/icons/video";
  import Wrench from "lucide-svelte/icons/wrench";
  import { SvelteURL } from "svelte/reactivity";
  import type { EmbeddedPageIconKind } from "$lib/api";

  const lucideIcons: Record<string, typeof PanelTop> = {
    bell: Bell,
    "book-open": BookOpen,
    bookmark: Bookmark,
    briefcase: Briefcase,
    "calendar-days": CalendarDays,
    cloud: Cloud,
    code: Code,
    database: Database,
    folder: Folder,
    "gamepad-2": Gamepad2,
    "git-branch": GitBranch,
    globe: Globe,
    heart: Heart,
    house: House,
    image: ImageIcon,
    link: Link,
    lock: Lock,
    mail: Mail,
    music: Music,
    podcast: Podcast,
    rocket: Rocket,
    rss: Rss,
    "shopping-bag": ShoppingBag,
    star: Star,
    terminal: Terminal,
    video: Video,
    wrench: Wrench,
  };

  type Props = {
    pageUrl: string;
    iconKind: EmbeddedPageIconKind;
    iconValue: string | null;
    size?: number;
  };

  let { pageUrl, iconKind, iconValue, size = 18 }: Props = $props();
  let failedSource = $state("");
  let Icon = $derived(lucideIcons[iconValue ?? ""] ?? PanelTop);
  let imageSource = $derived.by(() => {
    if (iconKind === "custom") return iconValue;
    if (iconKind !== "favicon") return null;
    try {
      const source = new SvelteURL(pageUrl);
      source.pathname = "/favicon.ico";
      source.search = "";
      source.hash = "";
      return source.toString();
    } catch {
      return null;
    }
  });
  let resolvedImageSource = $derived(
    imageSource && imageSource !== failedSource ? imageSource : null,
  );

  function useFallback() {
    failedSource = imageSource ?? "";
  }
</script>

{#if resolvedImageSource}
  <img
    class="embedded-page-icon-image"
    src={resolvedImageSource}
    alt=""
    decoding="async"
    referrerpolicy="no-referrer"
    onerror={useFallback}
    aria-hidden="true"
  />
{:else}
  <Icon {size} strokeWidth={1.7} aria-hidden="true" />
{/if}

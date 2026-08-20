<script lang="ts">
  import ExternalLink from "lucide-svelte/icons/external-link";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import type { EmbeddedPage } from "$lib/api";

  let { page } = $props<{ page: EmbeddedPage }>();
  let sandboxPermissions = $derived(
    page.allow_same_origin
      ? "allow-forms allow-popups allow-scripts allow-same-origin"
      : "allow-forms allow-popups allow-scripts",
  );
</script>

<section
  class="embedded-page product-page"
  data-od-id={`embedded-page-${page.id}`}
>
  <div class="embedded-page-header page-header">
    <div>
      <TypedHeading
        text={`$ ${page.title} --embed`}
        odId={`embedded-page-heading-${page.id}`}
      />
      <p>{page.description}</p>
    </div>
    <a
      class="ui-button ui-button--secondary embedded-page-external"
      href={page.url}
      target="_blank"
      rel="external noopener noreferrer"
      data-od-id={`open-embedded-page-${page.id}-externally`}
    >
      Open externally
      <ExternalLink size={17} strokeWidth={1.8} aria-hidden="true" />
    </a>
  </div>

  <div
    class="embedded-page-frame-shell"
    style:--embedded-page-height={`${page.iframe_height}px`}
    data-od-id={`embedded-page-frame-${page.id}`}
  >
    <iframe
      src={page.url}
      title={`${page.title} embedded page`}
      sandbox={sandboxPermissions}
      referrerpolicy="no-referrer"
    ></iframe>
  </div>
</section>

<script lang="ts">
  import type { EmbeddedPage } from "$lib/api";

  let { page, reloadToken } = $props<{
    page: EmbeddedPage;
    reloadToken: number;
  }>();
  let sandboxPermissions = $derived.by(() => {
    const permissions = ["allow-forms", "allow-popups"];
    if (page.allow_scripts) permissions.push("allow-scripts");
    if (page.allow_same_origin) permissions.push("allow-same-origin");
    return permissions.join(" ");
  });
</script>

<section
  class="embedded-page product-page"
  data-od-id={`embedded-page-${page.id}`}
>
  <div
    class="embedded-page-frame-shell"
    style:--embedded-page-height={`${page.iframe_height}px`}
    data-od-id={`embedded-page-frame-${page.id}`}
  >
    {#key reloadToken}
      <iframe
        src={page.url}
        title={`${page.title} embedded page`}
        sandbox={sandboxPermissions}
        referrerpolicy="no-referrer"
      ></iframe>
    {/key}
  </div>
</section>

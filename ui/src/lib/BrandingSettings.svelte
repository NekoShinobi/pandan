<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    brandAssetUrl,
    deleteBrandAsset,
    updateBrandAsset,
    type InstanceBranding,
  } from "$lib/api";

  type BrandAssetKind = "logo" | "favicon";

  let {
    branding,
    onSaved,
    onToast,
  }: {
    branding: InstanceBranding;
    onSaved: (branding: InstanceBranding) => void;
    onToast: (message: string) => void;
  } = $props();

  let logoFile = $state<File | null>(null);
  let faviconFile = $state<File | null>(null);
  let logoPreview = $state("");
  let faviconPreview = $state("");
  let removeLogo = $state(false);
  let removeFavicon = $state(false);
  let saving = $state(false);
  let error = $state("");

  let hasChanges = $derived(
    Boolean(logoFile || faviconFile || removeLogo || removeFavicon),
  );

  onDestroy(() => {
    revokePreview(logoPreview);
    revokePreview(faviconPreview);
  });

  function revokePreview(source: string) {
    if (source.startsWith("blob:")) URL.revokeObjectURL(source);
  }

  function assetSource(asset: BrandAssetKind) {
    if (asset === "logo") {
      if (removeLogo) return "";
      return (
        logoPreview ||
        (branding.has_logo
          ? brandAssetUrl("logo", branding.branding_updated_at)
          : "")
      );
    }
    if (removeFavicon) return "";
    return (
      faviconPreview ||
      (branding.has_favicon
        ? brandAssetUrl("favicon", branding.branding_updated_at)
        : "")
    );
  }

  function selectAsset(asset: BrandAssetKind, event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    if (
      ![
        "image/svg+xml",
        "image/jpeg",
        "image/png",
        "image/webp",
        "image/avif",
      ].includes(file.type)
    ) {
      error = "Choose an SVG, JPEG, PNG, WebP, or AVIF image.";
      input.value = "";
      return;
    }
    const maximumBytes = asset === "favicon" ? 1024 * 1024 : 10 * 1024 * 1024;
    if (file.size > maximumBytes) {
      error =
        asset === "favicon"
          ? "Favicon images must be 1 MB or smaller."
          : "Logo images must be 10 MB or smaller.";
      input.value = "";
      return;
    }

    const preview = URL.createObjectURL(file);
    if (asset === "logo") {
      revokePreview(logoPreview);
      logoFile = file;
      logoPreview = preview;
      removeLogo = false;
    } else {
      revokePreview(faviconPreview);
      faviconFile = file;
      faviconPreview = preview;
      removeFavicon = false;
    }
    error = "";
  }

  function useDefault(asset: BrandAssetKind) {
    if (asset === "logo") {
      revokePreview(logoPreview);
      logoFile = null;
      logoPreview = "";
      removeLogo = branding.has_logo;
    } else {
      revokePreview(faviconPreview);
      faviconFile = null;
      faviconPreview = "";
      removeFavicon = branding.has_favicon;
    }
    error = "";
  }

  function clearDrafts() {
    revokePreview(logoPreview);
    revokePreview(faviconPreview);
    logoFile = null;
    faviconFile = null;
    logoPreview = "";
    faviconPreview = "";
    removeLogo = false;
    removeFavicon = false;
  }

  async function saveBranding(event: SubmitEvent) {
    event.preventDefault();
    if (!hasChanges || saving) return;
    saving = true;
    error = "";
    try {
      let current: InstanceBranding = branding;
      if (logoFile) {
        current = await updateBrandAsset("logo", logoFile);
      } else if (removeLogo) {
        current = await deleteBrandAsset("logo");
      }
      if (faviconFile) {
        current = await updateBrandAsset("favicon", faviconFile);
      } else if (removeFavicon) {
        current = await deleteBrandAsset("favicon");
      }
      onSaved(current);
      clearDrafts();
      onToast("Instance branding saved");
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : "Unable to save instance branding";
    } finally {
      saving = false;
    }
  }
</script>

<form
  class="settings-surface branding-settings"
  onsubmit={saveBranding}
  data-od-id="instance-branding-form"
>
  <div class="settings-surface-heading">
    <div>
      <p class="widget-kicker">[ INSTANCE IDENTITY ]</p>
      <h4 id="instance-branding-heading">Logo and favicon</h4>
    </div>
    <span>Public assets</span>
  </div>

  <p class="settings-supporting-copy">
    Replace Pandan's mark across the login and application shell, and set the
    icon shown by browser tabs. Uploads are stored locally and never fetched
    from a third party. SVG uploads are rendered to safe PNGs before storage.
  </p>

  <div class="branding-asset-grid" aria-labelledby="instance-branding-heading">
    <article class="branding-asset-card" data-od-id="instance-logo-settings">
      <div
        class="branding-asset-preview branding-asset-preview--logo"
        aria-label="Instance logo preview"
        role="img"
      >
        {#if assetSource("logo")}
          <img src={assetSource("logo")} alt="" />
        {:else}
          <span aria-hidden="true">P&gt;</span>
        {/if}
      </div>
      <div class="branding-asset-copy">
        <strong>Instance logo</strong>
        <span>
          SVG or PNG is recommended. JPEG, WebP, and AVIF are also supported, up
          to 10 MB.
        </span>
      </div>
      <div class="branding-asset-actions">
        <label class="ui-button ui-button--secondary branding-upload">
          Choose logo
          <input
            type="file"
            accept="image/svg+xml,image/jpeg,image/png,image/webp,image/avif"
            onchange={(event) => selectAsset("logo", event)}
            data-od-id="choose-instance-logo"
          />
        </label>
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={() => useDefault("logo")}
          data-od-id="reset-instance-logo"
        >
          Use default
        </button>
      </div>
    </article>

    <article class="branding-asset-card" data-od-id="instance-favicon-settings">
      <div
        class="branding-asset-preview branding-asset-preview--favicon"
        aria-label="Instance favicon preview"
        role="img"
      >
        {#if assetSource("favicon")}
          <img src={assetSource("favicon")} alt="" />
        {:else}
          <img src="/favicon-32.png" alt="" />
        {/if}
      </div>
      <div class="branding-asset-copy">
        <strong>Browser favicon</strong>
        <span>
          Use a square SVG or raster image with a transparent background.
          Maximum file size: 1 MB.
        </span>
      </div>
      <div class="branding-asset-actions">
        <label class="ui-button ui-button--secondary branding-upload">
          Choose favicon
          <input
            type="file"
            accept="image/svg+xml,image/jpeg,image/png,image/webp,image/avif"
            onchange={(event) => selectAsset("favicon", event)}
            data-od-id="choose-instance-favicon"
          />
        </label>
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={() => useDefault("favicon")}
          data-od-id="reset-instance-favicon"
        >
          Use default
        </button>
      </div>
    </article>
  </div>

  {#if error}
    <p class="form-error" role="alert">{error}</p>
  {/if}

  <div class="settings-surface-actions">
    <button
      class="ui-button ui-button--primary"
      type="submit"
      disabled={!hasChanges || saving}
      data-od-id="save-instance-branding"
    >
      {saving ? "Saving…" : "Save branding"}
    </button>
  </div>
</form>

<script lang="ts">
  import RotateCcw from "lucide-svelte/icons/rotate-ccw";
  import { onDestroy, untrack } from "svelte";
  import {
    brandAssetUrl,
    deleteWallpaper,
    updateAppearance,
    updateLoginAppearance,
    updateWallpaper,
    type AuthenticationConfig,
    type LoginAppearance,
    type UserAppearance,
    type WallpaperSlot,
  } from "$lib/api";

  type BackgroundSlot = Extract<WallpaperSlot, "welcome" | "login">;

  type Props = {
    slot: BackgroundSlot;
    appearance: UserAppearance;
    authConfig: AuthenticationConfig;
    revision: number;
    onMainSaved: (appearance: UserAppearance) => void;
    onLoginSaved: (appearance: LoginAppearance) => void;
    onRevision: (slot: BackgroundSlot, hasCustom: boolean) => void;
    onOpenWalls: () => void;
    onToast: (message: string) => void;
  };

  let {
    slot,
    appearance,
    authConfig,
    revision,
    onMainSaved,
    onLoginSaved,
    onRevision,
    onOpenWalls,
    onToast,
  }: Props = $props();

  let file = $state<File | null>(null);
  let preview = $state("");
  let useDefault = $state(false);
  let hasCustom = $state(
    untrack(() =>
      slot === "welcome"
        ? appearance.has_welcome_wallpaper
        : appearance.has_login_wallpaper,
    ),
  );
  let currentRevision = $state(untrack(() => revision));
  let blur = $state(
    untrack(() =>
      slot === "welcome"
        ? appearance.background_blur
        : authConfig.login_background_blur,
    ),
  );
  let brightness = $state(
    untrack(() =>
      slot === "welcome"
        ? appearance.background_brightness
        : authConfig.login_background_brightness,
    ),
  );
  let contrast = $state(
    untrack(() =>
      slot === "welcome"
        ? appearance.background_contrast
        : authConfig.login_background_contrast,
    ),
  );
  let saturation = $state(
    untrack(() =>
      slot === "welcome"
        ? appearance.background_saturation
        : authConfig.login_background_saturation,
    ),
  );
  let saving = $state(false);
  let error = $state("");

  let isLogin = $derived(slot === "login");
  let title = $derived(isLogin ? "Login background" : "Main background");
  let code = $derived(isLogin ? "PUBLIC SURFACE" : "MAIN");
  let description = $derived(
    isLogin
      ? "The global pre-authentication image for every visitor."
      : "Used by the Welcome loading screen and throughout authenticated pages.",
  );
  let backgroundSource = $derived.by(() => {
    if (useDefault) return "/wired-terminal-wallpaper.png";
    if (preview) return preview;
    const endpoint = isLogin
      ? "/api/appearance/login-wallpaper"
      : "/api/settings/wallpapers/welcome";
    return `${endpoint}?v=${currentRevision}`;
  });
  let backgroundImage = $derived(
    backgroundSource === "/wired-terminal-wallpaper.png"
      ? 'url("/wired-terminal-wallpaper.png")'
      : `url("${backgroundSource}"), url("/wired-terminal-wallpaper.png")`,
  );
  let fileLabel = $derived(
    file?.name ??
      (useDefault || !hasCustom ? "Wired terminal default" : "Custom image"),
  );
  let logoSource = $derived(
    authConfig.has_logo
      ? brandAssetUrl("logo", authConfig.branding_updated_at)
      : "",
  );

  onDestroy(() => {
    if (preview.startsWith("blob:")) URL.revokeObjectURL(preview);
  });

  function selectBackground(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const selected = input.files?.[0];
    if (!selected) return;
    if (
      !["image/jpeg", "image/png", "image/webp", "image/avif"].includes(
        selected.type,
      )
    ) {
      error = "Choose a JPEG, PNG, WebP, or AVIF image.";
      input.value = "";
      return;
    }
    if (selected.size > 30 * 1024 * 1024) {
      error = "Wallpaper images must be 30 MB or smaller.";
      input.value = "";
      return;
    }
    if (preview.startsWith("blob:")) URL.revokeObjectURL(preview);
    file = selected;
    preview = URL.createObjectURL(selected);
    useDefault = false;
    error = "";
  }

  function resetBackground() {
    if (preview.startsWith("blob:")) URL.revokeObjectURL(preview);
    file = null;
    preview = "";
    useDefault = true;
    error = "";
  }

  function resetProcessing() {
    blur = 0;
    brightness = 78;
    contrast = 108;
    saturation = 72;
  }

  async function saveBackground(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true;
    error = "";
    try {
      if (file) {
        await updateWallpaper(slot, file);
      } else if (useDefault) {
        await deleteWallpaper(slot);
      }

      if (isLogin) {
        const updated = await updateLoginAppearance({
          background_blur: blur,
          background_brightness: brightness,
          background_contrast: contrast,
          background_saturation: saturation,
        });
        onLoginSaved(updated);
      } else {
        const updated = await updateAppearance({
          background_blur: blur,
          background_brightness: brightness,
          background_contrast: contrast,
          background_saturation: saturation,
        });
        onMainSaved(updated);
      }

      if (file) hasCustom = true;
      if (useDefault) hasCustom = false;
      if (preview.startsWith("blob:")) URL.revokeObjectURL(preview);
      file = null;
      preview = "";
      useDefault = false;
      currentRevision = Date.now();
      onRevision(slot, hasCustom);
      onToast(`${title} saved`);
    } catch (reason: unknown) {
      error =
        reason instanceof Error
          ? reason.message
          : `Unable to save ${title.toLowerCase()}`;
    } finally {
      saving = false;
    }
  }
</script>

<form
  class="appearance-editor background-settings-editor"
  onsubmit={saveBackground}
  data-od-id={`${slot}-background-form`}
>
  <section
    class="appearance-surface-card"
    aria-labelledby={`${slot}-background-title`}
    data-od-id={`wallpaper-${slot}-settings`}
  >
    {#if slot === "welcome"}
      <div
        class="background-preview appearance-preview main-page-preview"
        style:--background-preview={backgroundImage}
        style:--preview-blur={`${blur}px`}
        style:--preview-brightness={`${brightness}%`}
        style:--preview-contrast={`${contrast}%`}
        style:--preview-saturation={`${saturation}%`}
        aria-label="Main page preview with the selected wallpaper and processing"
        role="img"
        data-od-id="main-page-image-preview"
      >
        <div class="main-preview-rail" aria-hidden="true">
          <b>
            {#if logoSource}
              <img src={logoSource} alt="" />
            {:else}
              P&gt;
            {/if}
          </b>
          <i></i><i></i><i></i><i></i>
        </div>
        <div class="main-preview-canvas" aria-hidden="true">
          <header>
            <b>$ dashboard --overview</b>
            <i></i>
          </header>
          <div>
            <article></article>
            <article></article>
            <article></article>
          </div>
        </div>
        <span>[ {code} ]</span>
      </div>
    {:else}
      <div
        class="login-page-preview"
        style:--background-preview={backgroundImage}
        style:--preview-blur={`${blur}px`}
        style:--preview-brightness={`${brightness}%`}
        style:--preview-contrast={`${contrast}%`}
        style:--preview-saturation={`${saturation}%`}
        aria-label="Login page preview with the selected wallpaper and processing"
        role="img"
        data-od-id="login-page-image-preview"
      >
        <div class="login-preview-brand" aria-hidden="true">
          <span>
            {#if logoSource}
              <img src={logoSource} alt="" />
            {:else}
              P&gt;
            {/if}
          </span>
          <strong>PANDAN</strong>
        </div>
        <div class="login-preview-context" aria-hidden="true">
          <div>
            <small>[ PRIVATE WORKSPACE ]</small>
            <strong>Your private workspace.</strong>
            <p>Dashboards, tasks, calendars, feeds, and journal.</p>
          </div>
        </div>
        <div class="login-preview-access" aria-hidden="true">
          <div class="login-preview-copy">
            <small>[ ACCOUNT ACCESS ]</small>
            <strong>Welcome back.</strong>
            <p>Sign in to return to your dashboard.</p>
          </div>
          <div class="login-preview-modes">
            <span>Sign in</span>
            <span>Create account</span>
          </div>
          <div class="login-preview-form">
            <span>Email</span>
            <i></i>
            <span>Password</span>
            <i></i>
            <b>Enter dashboard</b>
          </div>
        </div>
      </div>
    {/if}

    <div class="appearance-surface-summary">
      <div class="wallpaper-slot-copy">
        <strong id={`${slot}-background-title`}>{title}</strong>
        <p>{description}</p>
        <small>
          {isLogin
            ? "Administrator managed · publicly retrievable"
            : "Personal to your account"}
        </small>
      </div>
      <span class="background-file-name">{fileLabel}</span>
    </div>

    <div class="wallpaper-slot-actions">
      <label
        class="ui-button ui-button--secondary secondary-btn background-upload"
      >
        Choose image
        <input
          type="file"
          accept="image/jpeg,image/png,image/webp,image/avif"
          onchange={selectBackground}
          data-od-id={`choose-${slot}-wallpaper`}
        />
      </label>
      <button
        class="ui-button ui-button--secondary"
        type="button"
        onclick={onOpenWalls}
        data-od-id={`browse-walls-${slot}`}>Browse Walls</button
      >
      <button
        class="ui-button ui-button--danger background-reset"
        type="button"
        onclick={resetBackground}
        data-od-id={`reset-${slot}-wallpaper`}>Use default</button
      >
    </div>

    <div class="appearance-control-heading">
      <strong>{title}</strong>
      <span>Processing applies only to this background.</span>
    </div>

    <div class="appearance-controls">
      <label>
        <span><strong>Blur</strong><output>{blur}px</output></span>
        <input type="range" min="0" max="24" step="1" bind:value={blur} />
      </label>
      <label>
        <span><strong>Brightness</strong><output>{brightness}%</output></span>
        <input
          type="range"
          min="40"
          max="140"
          step="1"
          bind:value={brightness}
        />
      </label>
      <label>
        <span><strong>Contrast</strong><output>{contrast}%</output></span>
        <input type="range" min="50" max="160" step="1" bind:value={contrast} />
      </label>
      <label>
        <span><strong>Saturation</strong><output>{saturation}%</output></span>
        <input
          type="range"
          min="0"
          max="180"
          step="1"
          bind:value={saturation}
        />
      </label>
    </div>

    <div class="appearance-surface-actions">
      <button
        class="ui-button ui-button--secondary secondary-btn"
        type="button"
        onclick={resetProcessing}
        data-od-id={`reset-${slot}-background-filters`}
      >
        <RotateCcw size={16} strokeWidth={1.8} aria-hidden="true" />
        Reset processing
      </button>
    </div>
  </section>

  {#if error}
    <p class="form-error" role="alert">{error}</p>
  {/if}

  <div class="appearance-actions">
    <button
      class="ui-button ui-button--primary primary-btn"
      type="submit"
      disabled={saving}
      data-od-id={`save-${slot}-background`}
    >
      {saving ? "Saving…" : `Save ${title.toLowerCase()}`}
    </button>
  </div>
</form>

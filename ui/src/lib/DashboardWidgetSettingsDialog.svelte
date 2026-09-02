<script lang="ts">
  import Check from "lucide-svelte/icons/check";
  import ImageIcon from "lucide-svelte/icons/image";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import {
    AUDIO_VISUALIZATION_GROUPS,
    isAudioVisualizationMode,
    type AudioVisualizationMode,
  } from "$lib/audioVisualizationCatalog";
  import {
    dashboardWidgetImageUrl,
    deleteDashboardWidgetImage,
    updateDashboardWidgetConfig,
    uploadDashboardWidgetImage,
    type DashboardWidget,
  } from "$lib/api";

  let {
    widget,
    onUpdate,
    onClose,
    onToast,
  }: {
    widget: DashboardWidget | null;
    onUpdate: (widget: DashboardWidget) => void;
    onClose: () => void;
    onToast: (message: string) => void;
  } = $props();

  let dialog = $state<HTMLDialogElement>();
  let activeWidgetId = "";
  let title = $state("");
  let sectionLabel = $state("");
  let showWelcomeStatus = $state(true);
  let clockStyle = $state<"analog" | "digital">("analog");
  let showCalendarMarkers = $state(true);
  let showBookmarkHosts = $state(true);
  let dividerStyle = $state<"solid" | "dashed" | "dotted">("solid");
  let taskSummaryStyle = $state<"agenda" | "progress">("agenda");
  let focusDefaultMinutes = $state(25);
  let showTaskPriorities = $state(true);
  let caption = $state("");
  let altText = $state("");
  let fit = $state<"contain" | "cover">("contain");
  let visualizationMode = $state<AudioVisualizationMode>("spectrum");
  let background = $state<"transparent" | "surface" | "artwork" | "custom">(
    "transparent",
  );
  let imageFile = $state<File>();
  let removeImage = $state(false);
  let saving = $state(false);
  let error = $state("");

  const widgetNames: Record<string, string> = {
    welcome: "Welcome",
    "section-header": "Category header",
    divider: "Line divider",
    "task-summary": "Today",
    focus: "Next focus",
    "task-list": "Task list",
    "calendar-overview": "Calendar overview",
    "local-time": "Local time",
    bookmarks: "Bookmarks",
    "bible-verse": "Bible Verse",
    iframe: "Custom iframe",
    html: "Custom HTML",
    "image-frame": "Image frame",
    "music-visualizer": "Music visualizer",
  };
  let supportsImage = $derived(
    widget?.kind === "image-frame" || widget?.kind === "music-visualizer",
  );
  let showImageControl = $derived(
    widget?.kind === "image-frame" ||
      (widget?.kind === "music-visualizer" && background === "custom"),
  );
  let hasImage = $derived(widget?.config.has_image === true);
  let displayName = $derived(
    title.trim() || (widget ? (widgetNames[widget.kind] ?? "Widget") : "Widget"),
  );

  function captureDialog(node: HTMLDialogElement) {
    dialog = node;
    return () => {
      if (dialog === node) dialog = undefined;
    };
  }

  function initialize(target: DashboardWidget) {
    title = typeof target.config.title === "string" ? target.config.title : "";
    sectionLabel =
      typeof target.config.label === "string" ? target.config.label : "";
    showWelcomeStatus = target.config.show_status !== false;
    clockStyle = target.config.clock_style === "digital" ? "digital" : "analog";
    showCalendarMarkers = target.config.show_event_markers !== false;
    showBookmarkHosts = target.config.show_hostnames !== false;
    dividerStyle =
      target.config.line_style === "dashed" ||
      target.config.line_style === "dotted"
        ? target.config.line_style
        : "solid";
    taskSummaryStyle =
      target.config.summary_style === "progress" ? "progress" : "agenda";
    focusDefaultMinutes = Math.min(
      240,
      Math.max(1, Number(target.config.default_minutes) || 25),
    );
    showTaskPriorities = target.config.show_priorities !== false;
    caption =
      typeof target.config.caption === "string" ? target.config.caption : "";
    altText =
      typeof target.config.alt_text === "string" ? target.config.alt_text : "";
    fit = target.config.fit === "cover" ? "cover" : "contain";
    visualizationMode =
      isAudioVisualizationMode(target.config.mode) && target.config.mode !== "off"
        ? target.config.mode
        : "spectrum";
    background =
      target.config.background === "surface" ||
      target.config.background === "artwork" ||
      target.config.background === "custom"
        ? target.config.background
        : "transparent";
    imageFile = undefined;
    removeImage = false;
    error = "";
  }

  $effect(() => {
    if (!widget) {
      activeWidgetId = "";
      return;
    }
    if (activeWidgetId === widget.id) return;
    activeWidgetId = widget.id;
    initialize(widget);
    if (dialog && !dialog.open) dialog.showModal();
  });

  function closeDialog() {
    dialog?.close();
  }

  function handleFile(event: Event) {
    const next = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!next) {
      imageFile = undefined;
      return;
    }
    if (!new Set(["image/jpeg", "image/png", "image/webp", "image/avif"]).has(next.type)) {
      error = "Choose a JPEG, PNG, WebP, or AVIF image.";
      return;
    }
    if (next.size > 10 * 1024 * 1024) {
      error = "Widget images must be 10 MB or smaller.";
      return;
    }
    imageFile = next;
    removeImage = false;
    error = "";
  }

  async function saveSettings(event: SubmitEvent) {
    event.preventDefault();
    if (!widget || saving) return;
    if (widget.kind === "section-header" && !sectionLabel.trim()) {
      error = "Enter a category label.";
      return;
    }
    if (
      widget.kind === "music-visualizer" &&
      background === "custom" &&
      !imageFile &&
      !hasImage
    ) {
      error = "Choose an image for the custom background.";
      return;
    }

    saving = true;
    error = "";
    try {
      let mediaWidget = widget;
      let nextHasImage = hasImage;
      if (supportsImage && removeImage && hasImage) {
        mediaWidget = await deleteDashboardWidgetImage(widget.id);
        nextHasImage = false;
      }
      if (supportsImage && imageFile) {
        mediaWidget = await uploadDashboardWidgetImage(widget.id, imageFile);
        nextHasImage = true;
      }

      const config: Record<string, unknown> = { ...mediaWidget.config };
      const trimmedTitle = title.trim();
      if (trimmedTitle) config.title = trimmedTitle;
      else delete config.title;
      if (widget.kind === "welcome") config.show_status = showWelcomeStatus;
      if (widget.kind === "local-time") config.clock_style = clockStyle;
      if (widget.kind === "calendar-overview") {
        config.show_event_markers = showCalendarMarkers;
      }
      if (widget.kind === "bookmarks") config.show_hostnames = showBookmarkHosts;
      if (widget.kind === "section-header") config.label = sectionLabel.trim();
      if (widget.kind === "divider") config.line_style = dividerStyle;
      if (widget.kind === "task-summary") {
        config.summary_style = taskSummaryStyle;
      }
      if (widget.kind === "focus") {
        config.default_minutes = Math.min(
          240,
          Math.max(1, Math.round(focusDefaultMinutes || 1)),
        );
      }
      if (widget.kind === "task-list") {
        config.show_priorities = showTaskPriorities;
      }
      if (widget.kind === "image-frame") {
        config.caption = caption.trim();
        config.alt_text = altText.trim();
        config.fit = fit;
        config.has_image = nextHasImage;
      }
      if (widget.kind === "music-visualizer") {
        config.mode = visualizationMode;
        config.background = background;
        config.has_image = nextHasImage;
      }
      const updated = await updateDashboardWidgetConfig(widget.id, { config });
      onUpdate(updated);
      onToast("Widget settings saved");
      closeDialog();
    } catch (reason: unknown) {
      error =
        reason instanceof Error ? reason.message : "Widget settings were not saved";
    } finally {
      saving = false;
    }
  }
</script>

<dialog
  class="settings-dialog dashboard-widget-settings-dialog"
  {@attach captureDialog}
  onclose={onClose}
  onclick={(event) => event.target === dialog && closeDialog()}
  data-od-id="dashboard-widget-settings-dialog"
>
  {#if widget}
    <div class="settings-heading">
      <div>
        <h2>Edit {displayName}</h2>
        <p>Changes apply only to this widget.</p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        type="button"
        aria-label="Close widget settings"
        onclick={closeDialog}
      >
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>
    <form class="settings-form" onsubmit={saveSettings}>
      <div class="settings-form-scroll dashboard-widget-settings-fields">
        <label for={`widget-title-${widget.id}`}>Widget name</label>
        <input
          id={`widget-title-${widget.id}`}
          bind:value={title}
          maxlength="80"
          placeholder="Use the default name"
        />
        <p class="field-note">Shown in edit mode and the widget action menu.</p>

        {#if widget.kind === "section-header"}
          <label for={`widget-section-label-${widget.id}`}>Category label</label>
          <input
            id={`widget-section-label-${widget.id}`}
            bind:value={sectionLabel}
            maxlength="80"
            required
          />
        {:else if widget.kind === "welcome"}
          <button
            class="ui-toggle-button widget-setting-toggle"
            type="button"
            aria-pressed={showWelcomeStatus}
            onclick={() => (showWelcomeStatus = !showWelcomeStatus)}
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>Show session status line</span>
          </button>
        {:else if widget.kind === "local-time"}
          <label for={`widget-clock-style-${widget.id}`}>Clock display</label>
          <select id={`widget-clock-style-${widget.id}`} bind:value={clockStyle}>
            <option value="analog">Analog with digital time</option>
            <option value="digital">Digital only</option>
          </select>
        {:else if widget.kind === "calendar-overview"}
          <button
            class="ui-toggle-button widget-setting-toggle"
            type="button"
            aria-pressed={showCalendarMarkers}
            onclick={() => (showCalendarMarkers = !showCalendarMarkers)}
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>Show event markers</span>
          </button>
        {:else if widget.kind === "bookmarks"}
          <button
            class="ui-toggle-button widget-setting-toggle"
            type="button"
            aria-pressed={showBookmarkHosts}
            onclick={() => (showBookmarkHosts = !showBookmarkHosts)}
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>Show destination hostnames</span>
          </button>
        {:else if widget.kind === "divider"}
          <label for={`widget-divider-style-${widget.id}`}>Line treatment</label>
          <select id={`widget-divider-style-${widget.id}`} bind:value={dividerStyle}>
            <option value="solid">Solid</option>
            <option value="dashed">Dashed</option>
            <option value="dotted">Dotted</option>
          </select>
        {:else if widget.kind === "task-summary"}
          <label for={`widget-summary-style-${widget.id}`}>Task detail</label>
          <select
            id={`widget-summary-style-${widget.id}`}
            bind:value={taskSummaryStyle}
          >
            <option value="agenda">Completion and task preview</option>
            <option value="progress">Completion only</option>
          </select>
        {:else if widget.kind === "focus"}
          <label for={`widget-focus-minutes-${widget.id}`}>Default timer</label>
          <span class="widget-number-setting">
            <input
              id={`widget-focus-minutes-${widget.id}`}
              type="number"
              bind:value={focusDefaultMinutes}
              min="1"
              max="240"
              step="1"
              required
            />
            <small>minutes</small>
          </span>
        {:else if widget.kind === "task-list"}
          <button
            class="ui-toggle-button widget-setting-toggle"
            type="button"
            aria-pressed={showTaskPriorities}
            onclick={() => (showTaskPriorities = !showTaskPriorities)}
          >
            <span class="ui-toggle-indicator" aria-hidden="true"></span>
            <span>Show task priorities</span>
          </button>
        {:else if widget.kind === "image-frame"}
          <label for={`widget-caption-${widget.id}`}>Caption</label>
          <input
            id={`widget-caption-${widget.id}`}
            bind:value={caption}
            maxlength="160"
            placeholder="Optional caption"
          />
          <label for={`widget-alt-${widget.id}`}>Image description</label>
          <textarea
            id={`widget-alt-${widget.id}`}
            bind:value={altText}
            maxlength="240"
            rows="3"
            placeholder="Describe the image for screen readers"
          ></textarea>
          <label for={`widget-fit-${widget.id}`}>Frame fit</label>
          <select id={`widget-fit-${widget.id}`} bind:value={fit}>
            <option value="contain">Show the full image</option>
            <option value="cover">Fill and crop the frame</option>
          </select>
        {:else if widget.kind === "music-visualizer"}
          <label for={`widget-visualizer-mode-${widget.id}`}>Visualization</label>
          <select
            id={`widget-visualizer-mode-${widget.id}`}
            bind:value={visualizationMode}
          >
            {#each AUDIO_VISUALIZATION_GROUPS as group (group.id)}
              <optgroup label={group.label}>
                {#each group.styles as style (style.id)}
                  <option value={style.id}>{style.label}</option>
                {/each}
              </optgroup>
            {/each}
          </select>
          <fieldset class="widget-background-options">
            <legend>Background</legend>
            <div role="radiogroup" aria-label="Visualizer background">
              {#each [
                ["transparent", "Transparent"],
                ["surface", "Panel"],
                ["artwork", "Album artwork"],
                ["custom", "Custom image"],
              ] as option (option[0])}
                <button
                  type="button"
                  role="radio"
                  aria-checked={background === option[0]}
                  onclick={() =>
                    (background = option[0] as typeof background)}
                >
                  {option[1]}
                </button>
              {/each}
            </div>
          </fieldset>
        {/if}

        {#if showImageControl}
          <div class="widget-image-setting">
            <span>Image</span>
            {#if hasImage && !removeImage}
              <img
                src={dashboardWidgetImageUrl(widget.id, widget.updated_at)}
                alt="Current widget media"
              />
            {/if}
            <label class="widget-image-picker" for={`widget-image-${widget.id}`}>
              <ImageIcon size={16} strokeWidth={1.8} aria-hidden="true" />
              <span>{imageFile?.name ?? (hasImage ? "Replace image" : "Choose image")}</span>
              <input
                id={`widget-image-${widget.id}`}
                type="file"
                accept="image/jpeg,image/png,image/webp,image/avif"
                onchange={handleFile}
              />
            </label>
            {#if hasImage}
              <button
                class="ui-toggle-button widget-image-remove"
                type="button"
                aria-pressed={removeImage}
                onclick={() => {
                  removeImage = !removeImage;
                  if (removeImage) imageFile = undefined;
                }}
              >
                <span class="ui-toggle-indicator" aria-hidden="true">
                  {#if removeImage}<Check size={13} />{/if}
                </span>
                <Trash2 size={15} strokeWidth={1.8} aria-hidden="true" />
                <span>Remove current image when saved</span>
              </button>
            {/if}
            <small>JPEG, PNG, WebP, or AVIF · 10 MB maximum</small>
          </div>
        {/if}

        {#if error}<p class="form-error" role="alert">{error}</p>{/if}
      </div>
      <div class="settings-actions">
        <button
          class="ui-button ui-button--secondary"
          type="button"
          onclick={closeDialog}
        >Cancel</button>
        <button
          class="ui-button ui-button--primary"
          type="submit"
          disabled={saving}
        >{saving ? "Saving…" : "Save settings"}</button>
      </div>
    </form>
  {/if}
</dialog>

<script lang="ts">
  import Cloud from "lucide-svelte/icons/cloud";
  import CloudFog from "lucide-svelte/icons/cloud-fog";
  import CloudLightning from "lucide-svelte/icons/cloud-lightning";
  import CloudRain from "lucide-svelte/icons/cloud-rain";
  import CloudSun from "lucide-svelte/icons/cloud-sun";
  import Droplets from "lucide-svelte/icons/droplets";
  import Gauge from "lucide-svelte/icons/gauge";
  import MapPin from "lucide-svelte/icons/map-pin";
  import Snowflake from "lucide-svelte/icons/snowflake";
  import Sun from "lucide-svelte/icons/sun";
  import Wind from "lucide-svelte/icons/wind";
  import X from "lucide-svelte/icons/x";
  import { onDestroy } from "svelte";
  import {
    updateDashboardWidgetConfig,
    type DashboardWidget,
    type UserSettings,
  } from "$lib/api";
  import {
    compassDirection,
    fetchWeatherSnapshot,
    isWeatherLocation,
    searchWeatherLocations,
    weatherCodeLabel,
    type TemperatureUnit,
    type WeatherLocation,
    type WeatherSnapshot,
  } from "$lib/weather";

  let {
    widget,
    settings,
    onUpdate,
    onToast,
  }: {
    widget: DashboardWidget;
    settings: UserSettings;
    onUpdate: (widget: DashboardWidget) => void;
    onToast: (message: string) => void;
  } = $props();

  let snapshots = $state.raw<WeatherSnapshot[]>([]);
  let selectedLocationId = $state<number | null>(null);
  let loading = $state(false);
  let loadError = $state("");
  let saving = $state(false);
  let formError = $state("");
  let searchQuery = $state("");
  let searchResults = $state.raw<WeatherLocation[]>([]);
  let searchLoading = $state(false);
  let draftLocations = $state.raw<WeatherLocation[]>([]);
  let draftUnit = $state<TemperatureUnit>("celsius");
  let configDialog = $state<HTMLDialogElement>();
  let detailDialog = $state<HTMLDialogElement>();
  let searchController: AbortController | undefined;
  let weatherController: AbortController | undefined;
  let initialized = false;

  let locations = $derived(readLocations(widget.config.locations));
  let unit = $derived<TemperatureUnit>(
    widget.config.unit === "fahrenheit" || widget.config.unit === "celsius"
      ? widget.config.unit
      : settings.temperature_unit,
  );
  let selectedLocation = $derived(
    locations.find((location) => location.id === selectedLocationId) ??
      locations[0] ??
      null,
  );
  let selectedWeather = $derived(
    snapshots.find(
      (snapshot) => snapshot.location.id === selectedLocation?.id,
    ) ??
      snapshots[0] ??
      null,
  );
  let upcomingHours = $derived.by(() => {
    if (!selectedWeather) return [];
    const currentHour = selectedWeather.current.time.slice(0, 13);
    const start = Math.max(
      0,
      selectedWeather.hourly.findIndex(
        (hour) => hour.time.slice(0, 13) >= currentHour,
      ),
    );
    return selectedWeather.hourly.slice(start, start + 8);
  });
  let CurrentWeatherIcon = $derived(
    weatherIcon(selectedWeather?.current.weatherCode ?? 0),
  );

  $effect(() => {
    if (initialized) return;
    initialized = true;
    selectedLocationId = locations[0]?.id ?? null;
    if (locations.length > 0) void loadWeather(locations, unit);
  });

  onDestroy(() => {
    searchController?.abort();
    weatherController?.abort();
  });

  function readLocations(value: unknown): WeatherLocation[] {
    return Array.isArray(value) ? value.filter(isWeatherLocation) : [];
  }

  function captureConfigDialog(node: HTMLDialogElement) {
    configDialog = node;
    return () => (configDialog = undefined);
  }

  function captureDetailDialog(node: HTMLDialogElement) {
    detailDialog = node;
    return () => (detailDialog = undefined);
  }

  function openConfig() {
    draftLocations = locations.map((location) => ({ ...location }));
    draftUnit = unit;
    searchQuery = "";
    searchResults = [];
    formError = "";
    configDialog?.showModal();
  }

  function openDetails() {
    if (selectedWeather) detailDialog?.showModal();
  }

  async function searchCities() {
    const query = searchQuery.trim();
    if (query.length < 2) {
      formError = "Enter at least two characters to search.";
      return;
    }
    searchController?.abort();
    searchController = new AbortController();
    searchLoading = true;
    formError = "";
    try {
      searchResults = await searchWeatherLocations(
        query,
        searchController.signal,
      );
      if (searchResults.length === 0) formError = "No matching cities found.";
    } catch (reason: unknown) {
      if ((reason as Error).name !== "AbortError") {
        formError =
          reason instanceof Error ? reason.message : "City search failed";
      }
    } finally {
      searchLoading = false;
    }
  }

  function addLocation(location: WeatherLocation) {
    if (draftLocations.some((candidate) => candidate.id === location.id))
      return;
    if (draftLocations.length >= 8) {
      formError = "A weather widget can track up to eight cities.";
      return;
    }
    draftLocations = [...draftLocations, location];
    searchResults = [];
    searchQuery = "";
    formError = "";
  }

  function removeLocation(locationId: number) {
    draftLocations = draftLocations.filter(
      (location) => location.id !== locationId,
    );
  }

  async function saveConfig(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    saving = true;
    formError = "";
    try {
      const updated = await updateDashboardWidgetConfig(widget.id, {
        config: { locations: draftLocations, unit: draftUnit },
      });
      onUpdate(updated);
      selectedLocationId = draftLocations[0]?.id ?? null;
      configDialog?.close();
      onToast("Weather cities saved");
      await loadWeather(draftLocations, draftUnit);
    } catch (reason: unknown) {
      formError =
        reason instanceof Error
          ? reason.message
          : "Weather settings were not saved";
    } finally {
      saving = false;
    }
  }

  async function loadWeather(
    targetLocations: WeatherLocation[] = locations,
    targetUnit: TemperatureUnit = unit,
  ) {
    if (targetLocations.length === 0) {
      snapshots = [];
      loadError = "";
      return;
    }
    weatherController?.abort();
    weatherController = new AbortController();
    loading = true;
    loadError = "";
    try {
      const results = await Promise.allSettled(
        targetLocations.map((location) =>
          fetchWeatherSnapshot(location, targetUnit, weatherController?.signal),
        ),
      );
      snapshots = results.flatMap((result) =>
        result.status === "fulfilled" ? [result.value] : [],
      );
      const failed = results.length - snapshots.length;
      if (snapshots.length === 0) {
        throw new Error("Open-Meteo did not return weather for these cities");
      }
      if (failed > 0)
        loadError = `${failed} ${failed === 1 ? "city" : "cities"} could not be refreshed.`;
    } catch (reason: unknown) {
      if ((reason as Error).name !== "AbortError") {
        snapshots = [];
        loadError =
          reason instanceof Error ? reason.message : "Weather is unavailable";
      }
    } finally {
      loading = false;
    }
  }

  function weatherIcon(code: number) {
    if (code === 0) return Sun;
    if (code <= 2) return CloudSun;
    if (code === 3) return Cloud;
    if (code === 45 || code === 48) return CloudFog;
    if ((code >= 51 && code <= 67) || (code >= 80 && code <= 82))
      return CloudRain;
    if ((code >= 71 && code <= 77) || (code >= 85 && code <= 86))
      return Snowflake;
    if (code >= 95) return CloudLightning;
    return Cloud;
  }

  function locationLabel(location: WeatherLocation) {
    return [location.admin1, location.country].filter(Boolean).join(", ");
  }

  function formatHour(value: string) {
    return value.slice(11, 16);
  }

  function formatDay(value: string, index: number) {
    if (index === 0) return "Today";
    return new Intl.DateTimeFormat("en", { weekday: "short" }).format(
      new Date(`${value}T12:00:00`),
    );
  }

  function formatSunTime(value: string) {
    const time = value.slice(11, 16);
    return time || "—";
  }
</script>

<div class="weather-widget" data-od-id={`weather-content-${widget.id}`}>
  <div class="weather-toolbar">
    <div>
      <p class="widget-kicker">Open-Meteo forecast</p>
      <span class="weather-status">
        {#if loading}
          <span
            class="weather-spinner weather-spinner--inline"
            aria-hidden="true"
          ></span>
          Updating conditions…
        {:else}
          City-based · independent of profile
        {/if}
      </span>
    </div>
    <div class="weather-actions">
      {#if locations.length > 0}
        <button
          class="ui-button ui-button--ghost text-button"
          type="button"
          disabled={loading}
          onclick={() => loadWeather()}
        >
          Refresh
        </button>
      {/if}
      <button class="ui-button ui-button--ghost text-button" type="button" onclick={openConfig}
        >Configure</button
      >
    </div>
  </div>

  {#if locations.length === 0}
    <button
      class="weather-empty"
      type="button"
      onclick={openConfig}
      data-od-id={`weather-add-city-${widget.id}`}
    >
      <MapPin size={24} strokeWidth={1.7} aria-hidden="true" />
      <strong>Choose cities to follow</strong>
      <span>This weather widget keeps its own locations.</span>
    </button>
  {:else}
    <div class="city-tabs" aria-label="Tracked cities">
      {#each locations as location (location.id)}
        <button
          type="button"
          aria-pressed={selectedLocation?.id === location.id}
          onclick={() => (selectedLocationId = location.id)}
          data-od-id={`weather-city-${widget.id}-${location.id}`}
        >
          {location.name}
        </button>
      {/each}
    </div>

    {#if snapshots.length === 0 && !loadError}
      <div class="weather-loading" role="status" aria-live="polite">
        <span class="weather-spinner" aria-hidden="true"></span>
        Loading live weather…
      </div>
    {:else if selectedWeather}
      {#key selectedWeather.location.id}
        <div class="weather-current weather-reveal">
          <div class="weather-reading">
            <div class="weather-place">
              <h2>{selectedWeather.location.name}</h2>
              <span>{locationLabel(selectedWeather.location)}</span>
            </div>
            <div class="weather-temperature-row">
              <strong class="weather-temperature mono"
                >{Math.round(selectedWeather.current.temperature)}°</strong
              >
              <div class="weather-condition">
                <CurrentWeatherIcon
                  size={44}
                  strokeWidth={1.45}
                  aria-hidden="true"
                />
                <span
                  >{weatherCodeLabel(selectedWeather.current.weatherCode)}</span
                >
                <small
                  >Feels like {Math.round(
                    selectedWeather.current.apparentTemperature,
                  )}°</small
                >
              </div>
            </div>
          </div>
          <div class="weather-quick-stats">
            <span
              ><Droplets size={16} strokeWidth={1.7} aria-hidden="true" />
              {Math.round(selectedWeather.current.humidity)}%</span
            >
            <span
              ><Wind size={16} strokeWidth={1.7} aria-hidden="true" />
              {Math.round(selectedWeather.current.windSpeed)}
              {selectedWeather.windUnit}</span
            >
          </div>
        </div>

        <div
          class="weather-hour-strip weather-reveal"
          aria-label="Upcoming hourly forecast"
        >
          {#each upcomingHours.slice(0, widget.size === "compact" ? 3 : widget.size === "standard" ? 5 : 8) as hour (hour.time)}
            <div>
              <span class="mono">{formatHour(hour.time)}</span>
              <strong class="mono">{Math.round(hour.temperature)}°</strong>
              <small>{Math.round(hour.precipitationProbability)}% rain</small>
            </div>
          {/each}
        </div>

        <button
          class="weather-details-button weather-reveal"
          type="button"
          onclick={openDetails}
          data-od-id={`weather-details-${widget.id}`}
        >
          View detailed forecast
          <span aria-hidden="true">↗</span>
        </button>
      {/key}
    {:else}
      <div class="weather-error" role="status">
        <strong>Weather is unavailable</strong>
        <span>{loadError || "Try refreshing the forecast."}</span>
      </div>
    {/if}
    {#if loadError && selectedWeather}<p class="weather-partial" role="status">
        {loadError}
      </p>{/if}
  {/if}
</div>

<dialog
  class="settings-dialog weather-config-dialog"
  {@attach captureConfigDialog}
  onclick={(event) => event.target === configDialog && configDialog.close()}
  data-od-id={`configure-weather-${widget.id}`}
>
  <div class="settings-heading">
    <div>
      <h2>Weather locations</h2>
      <p>Track up to eight cities in this widget.</p>
    </div>
    <button
      class="ui-button ui-button--ghost ui-button--icon dialog-close"
      type="button"
      aria-label="Close weather settings"
      onclick={() => configDialog?.close()}
    >
      <X size={18} strokeWidth={1.8} aria-hidden="true" />
    </button>
  </div>
  <form class="settings-form weather-config-form" onsubmit={saveConfig}>
    <div class="settings-form-scroll weather-config-fields">
      <label for={`weather-search-${widget.id}`}>Find a city</label>
      <div class="weather-search-row">
        <input
          id={`weather-search-${widget.id}`}
          class="text-input"
          bind:value={searchQuery}
          placeholder="City or postal code"
          autocomplete="off"
          onkeydown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void searchCities();
            }
          }}
        />
        <button
          class="ui-button ui-button--secondary secondary-btn"
          type="button"
          disabled={searchLoading}
          onclick={searchCities}
        >
          {searchLoading ? "Searching…" : "Search"}
        </button>
      </div>

      {#if searchResults.length > 0}
        <div class="weather-search-results" aria-label="City search results">
          {#each searchResults as result (result.id)}
            <button type="button" onclick={() => addLocation(result)}>
              <span
                ><strong>{result.name}</strong><small
                  >{locationLabel(result)}</small
                ></span
              >
              <span>Add</span>
            </button>
          {/each}
        </div>
      {/if}

      <div class="weather-config-section">
        <span class="form-section-label">Tracked cities</span>
        <div class="tracked-city-list">
          {#each draftLocations as location (location.id)}
            <div>
              <span
                ><strong>{location.name}</strong><small
                  >{locationLabel(location)}</small
                ></span
              >
              <button
                class="ui-button ui-button--danger ui-button--icon"
                type="button"
                aria-label={`Remove ${location.name}`}
                onclick={() => removeLocation(location.id)}
              >
                <X size={16} strokeWidth={1.8} aria-hidden="true" />
              </button>
            </div>
          {:else}
            <p>No cities selected. The widget will show a setup prompt.</p>
          {/each}
        </div>
      </div>

      <label for={`weather-unit-${widget.id}`}>Temperature unit</label>
      <select
        id={`weather-unit-${widget.id}`}
        class="select-input"
        bind:value={draftUnit}
      >
        <option value="celsius">Celsius</option>
        <option value="fahrenheit">Fahrenheit</option>
      </select>

      {#if formError}<p class="form-error" role="alert">{formError}</p>{/if}
    </div>
    <div class="settings-actions weather-form-actions">
      <button
        class="ui-button ui-button--secondary secondary-btn"
        type="button"
        onclick={() => configDialog?.close()}>Cancel</button
      >
      <button class="ui-button ui-button--primary primary-btn" type="submit" disabled={saving}
        >{saving ? "Saving…" : "Save cities"}</button
      >
    </div>
  </form>
</dialog>

<dialog
  class="settings-dialog weather-detail-dialog"
  {@attach captureDetailDialog}
  onclick={(event) => event.target === detailDialog && detailDialog.close()}
  data-od-id={`weather-detail-view-${widget.id}`}
>
  {#if selectedWeather}
    <div class="settings-heading weather-detail-heading">
      <div>
        <p class="widget-kicker">7-day forecast</p>
        <h2>{selectedWeather.location.name}</h2>
        <p>
          {locationLabel(selectedWeather.location)} · {selectedWeather.location.timezone.replaceAll(
            "_",
            " ",
          )}
        </p>
      </div>
      <button
        class="ui-button ui-button--ghost ui-button--icon dialog-close"
        type="button"
        aria-label="Close detailed forecast"
        onclick={() => detailDialog?.close()}
      >
        <X size={18} strokeWidth={1.8} aria-hidden="true" />
      </button>
    </div>
    <div class="weather-detail-body">
      <div class="weather-detail-hero">
        <CurrentWeatherIcon size={56} strokeWidth={1.35} aria-hidden="true" />
        <strong class="mono"
          >{Math.round(selectedWeather.current.temperature)}°</strong
        >
        <div>
          <h3>{weatherCodeLabel(selectedWeather.current.weatherCode)}</h3>
          <p>
            Feels like {Math.round(
              selectedWeather.current.apparentTemperature,
            )}{selectedWeather.temperatureSymbol}
          </p>
        </div>
      </div>

      <div class="weather-metric-grid">
        <div>
          <Droplets size={18} strokeWidth={1.7} aria-hidden="true" /><span
            >Humidity</span
          ><strong>{Math.round(selectedWeather.current.humidity)}%</strong>
        </div>
        <div>
          <Wind size={18} strokeWidth={1.7} aria-hidden="true" /><span
            >Wind</span
          ><strong
            >{Math.round(selectedWeather.current.windSpeed)}
            {selectedWeather.windUnit}
            {compassDirection(selectedWeather.current.windDirection)}</strong
          >
        </div>
        <div>
          <Gauge size={18} strokeWidth={1.7} aria-hidden="true" /><span
            >Pressure</span
          ><strong>{Math.round(selectedWeather.current.pressure)} hPa</strong>
        </div>
        <div>
          <CloudRain size={18} strokeWidth={1.7} aria-hidden="true" /><span
            >Precipitation</span
          ><strong
            >{selectedWeather.current.precipitation.toFixed(1)}
            {selectedWeather.precipitationUnit}</strong
          >
        </div>
      </div>

      <section
        class="weather-detail-section"
        aria-labelledby={`hourly-title-${widget.id}`}
      >
        <h3 id={`hourly-title-${widget.id}`}>Next hours</h3>
        <div class="weather-detail-hours">
          {#each upcomingHours as hour (hour.time)}
            <div>
              <span class="mono">{formatHour(hour.time)}</span>
              <strong class="mono">{Math.round(hour.temperature)}°</strong>
              <small
                >{Math.round(hour.precipitationProbability)}% · {Math.round(
                  hour.windSpeed,
                )}
                {selectedWeather.windUnit}</small
              >
            </div>
          {/each}
        </div>
      </section>

      <section
        class="weather-detail-section"
        aria-labelledby={`daily-title-${widget.id}`}
      >
        <h3 id={`daily-title-${widget.id}`}>This week</h3>
        <div class="weather-day-list">
          {#each selectedWeather.daily as day, index (day.date)}
            {@const DayIcon = weatherIcon(day.weatherCode)}
            <div>
              <span>{formatDay(day.date, index)}</span>
              <span class="weather-day-condition"
                ><DayIcon size={18} strokeWidth={1.6} aria-hidden="true" />
                {weatherCodeLabel(day.weatherCode)}</span
              >
              <span class="mono"
                >{Math.round(day.temperatureMin)}° / {Math.round(
                  day.temperatureMax,
                )}°</span
              >
              <small>{Math.round(day.precipitationProbability)}% rain</small>
              <small
                >{formatSunTime(day.sunrise)}–{formatSunTime(day.sunset)}</small
              >
            </div>
          {/each}
        </div>
      </section>
    </div>
  {/if}
</dialog>

<style>
  .weather-widget,
  .weather-reading,
  .weather-condition,
  .weather-config-form,
  .weather-detail-body,
  .weather-detail-section {
    min-width: 0;
  }

  .weather-toolbar,
  .weather-actions,
  .weather-temperature-row,
  .weather-quick-stats,
  .weather-details-button,
  .weather-search-row,
  .weather-day-condition {
    display: flex;
    align-items: center;
  }

  .weather-toolbar {
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 15px;
  }

  .weather-status {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 3px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .weather-spinner {
    width: 18px;
    height: 18px;
    flex: 0 0 auto;
    border: 2px solid var(--border);
    border-top-color: var(--fg);
    border-radius: 50%;
    animation: weather-spin 700ms linear infinite;
  }

  .weather-spinner--inline {
    width: 10px;
    height: 10px;
    border-width: 1.5px;
  }

  .weather-reveal {
    animation: weather-reveal 320ms var(--ease-out) both;
  }

  .weather-current.weather-reveal {
    animation-delay: 0ms;
  }

  .weather-hour-strip.weather-reveal {
    animation-delay: 70ms;
  }

  .weather-details-button.weather-reveal {
    animation-delay: 140ms;
  }

  @keyframes weather-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes weather-reveal {
    from {
      opacity: 0;
      transform: translateY(8px);
    }

    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .weather-spinner {
      animation: none;
    }

    .weather-reveal {
      opacity: 1;
      transform: none;
      animation: none;
    }
  }

  .weather-actions {
    flex: 0 0 auto;
    gap: 2px;
  }

  .weather-empty,
  .weather-loading,
  .weather-error {
    width: 100%;
    min-height: 188px;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 7px;
    padding: 24px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-sm);
    background: var(--fg-soft);
    color: var(--fg);
    text-align: center;
  }

  .weather-empty span,
  .weather-error span {
    color: var(--muted);
    font-size: 12px;
  }

  .city-tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 22px;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .city-tabs::-webkit-scrollbar {
    display: none;
  }

  .city-tabs button {
    min-height: 34px;
    flex: 0 0 auto;
    padding: 0 11px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    font-size: 11px;
  }

  .city-tabs button:hover,
  .city-tabs button[aria-pressed="true"] {
    border-color: var(--fg);
    background: var(--fg);
    color: var(--surface);
  }

  .weather-current {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 24px;
    align-items: end;
  }

  .weather-place h2 {
    font-family: var(--font-display);
    font-size: clamp(24px, 4vw, 38px);
    font-weight: 590;
    letter-spacing: -0.02em;
    line-height: 1.1;
  }

  .weather-place span {
    color: var(--muted);
    font-size: 12px;
  }

  .weather-temperature-row {
    gap: 20px;
    margin-top: 14px;
  }

  .weather-temperature {
    font-size: clamp(62px, 8vw, 104px);
    font-weight: 450;
    letter-spacing: -0.07em;
    line-height: 0.9;
  }

  .weather-condition {
    align-items: flex-start;
    flex-direction: column;
    gap: 4px;
  }

  .weather-condition span {
    margin-top: 5px;
    font-weight: 620;
  }

  .weather-condition small,
  .weather-hour-strip small,
  .tracked-city-list small,
  .weather-search-results small {
    color: var(--muted);
    font-size: 10px;
  }

  .weather-quick-stats {
    align-items: flex-start;
    flex-direction: column;
    gap: 10px;
    padding-bottom: 8px;
  }

  .weather-quick-stats span {
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }

  .weather-hour-strip,
  .weather-detail-hours {
    display: grid;
    grid-template-columns: repeat(8, minmax(72px, 1fr));
    margin-top: 26px;
    overflow-x: auto;
    border-top: 1px solid var(--border);
  }

  .weather-hour-strip > div,
  .weather-detail-hours > div {
    min-width: 72px;
    display: grid;
    gap: 4px;
    padding: 13px 10px 2px;
    border-left: 1px solid var(--border);
  }

  .weather-hour-strip > div:first-child,
  .weather-detail-hours > div:first-child {
    padding-left: 0;
    border-left: 0;
  }

  .weather-hour-strip span,
  .weather-detail-hours span {
    color: var(--muted);
    font-size: 10px;
  }

  .weather-hour-strip strong,
  .weather-detail-hours strong {
    font-size: 17px;
  }

  .weather-details-button {
    width: 100%;
    min-height: 44px;
    justify-content: space-between;
    margin-top: 22px;
    padding: 0;
    border: 0;
    border-top: 1px solid var(--border);
    background: transparent;
    color: var(--fg);
    font-weight: 620;
    text-align: left;
  }

  .weather-details-button:hover {
    padding-inline: 8px;
    background: var(--fg-soft);
  }

  .weather-partial {
    margin-top: 10px;
    color: var(--muted);
    font-size: 10px;
  }

  .weather-config-dialog {
    width: min(620px, calc(100vw - 32px));
  }

  .weather-config-dialog[open],
  .weather-detail-dialog[open] {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .weather-config-fields {
    display: grid;
    gap: 12px;
  }

  .weather-search-row {
    gap: 8px;
  }

  .weather-search-row input {
    min-width: 0;
    flex: 1 1 auto;
    margin-bottom: 0;
  }

  .weather-config-fields > .select-input {
    margin-bottom: 0;
  }

  .weather-search-results,
  .tracked-city-list {
    display: grid;
    border: 1px solid var(--border);
  }

  .weather-search-results button,
  .tracked-city-list > div {
    min-height: 52px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 12px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    text-align: left;
  }

  .weather-search-results button:last-child,
  .tracked-city-list > div:last-child {
    border-bottom: 0;
  }

  .weather-search-results button:hover {
    background: var(--fg-soft);
  }

  .weather-search-results button > span:first-child,
  .tracked-city-list > div > span {
    min-width: 0;
    display: grid;
  }

  .weather-config-section {
    display: grid;
    gap: 8px;
    margin-block: 6px;
  }

  .form-section-label {
    font-weight: 620;
  }

  .tracked-city-list button {
    width: 40px;
    height: 40px;
    display: grid;
    flex: 0 0 auto;
    place-items: center;
    border: 0;
    background: transparent;
  }

  .tracked-city-list p {
    padding: 18px;
    color: var(--muted);
    font-size: 12px;
  }

  .weather-form-actions {
    justify-content: flex-end;
  }

  .weather-detail-dialog {
    width: min(880px, calc(100vw - 32px));
  }

  .weather-detail-heading .widget-kicker {
    margin-bottom: 5px;
  }

  .weather-detail-body {
    min-height: 0;
    flex: 1;
    display: grid;
    gap: 28px;
    padding: 24px;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
  }

  .weather-detail-hero {
    display: grid;
    grid-template-columns: auto auto minmax(0, 1fr);
    gap: 18px;
    align-items: center;
  }

  .weather-detail-hero > strong {
    font-size: 64px;
    font-weight: 450;
    letter-spacing: -0.06em;
    line-height: 1;
  }

  .weather-detail-hero h3 {
    font-family: var(--font-display);
    font-size: 24px;
    font-weight: 590;
    letter-spacing: -0.01em;
  }

  .weather-detail-hero p {
    color: var(--muted);
  }

  .weather-metric-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    border: 1px solid var(--border);
  }

  .weather-metric-grid > div {
    min-width: 0;
    display: grid;
    gap: 7px;
    padding: 15px;
    border-left: 1px solid var(--border);
  }

  .weather-metric-grid > div:first-child {
    border-left: 0;
  }

  .weather-metric-grid span {
    color: var(--muted);
    font-size: 10px;
  }

  .weather-metric-grid strong {
    overflow-wrap: anywhere;
    font-size: 12px;
  }

  .weather-detail-section {
    display: grid;
    gap: 12px;
  }

  .weather-detail-section h3 {
    font-family: var(--font-display);
    font-size: 19px;
    font-weight: 590;
  }

  .weather-detail-hours {
    margin-top: 0;
  }

  .weather-day-list {
    display: grid;
    border-top: 1px solid var(--border);
  }

  .weather-day-list > div {
    min-height: 52px;
    display: grid;
    grid-template-columns: 70px minmax(150px, 1.5fr) minmax(
        100px,
        0.8fr
      ) 74px 84px;
    gap: 12px;
    align-items: center;
    border-bottom: 1px solid var(--border);
  }

  .weather-day-list small,
  .weather-day-list > div > span:first-child {
    color: var(--muted);
    font-size: 10px;
  }

  .weather-day-condition {
    gap: 8px;
  }

  :global(.widget-size-compact) .weather-current {
    grid-template-columns: 1fr;
  }

  :global(.widget-size-compact) .weather-quick-stats,
  :global(.widget-size-compact) .weather-condition small,
  :global(.widget-size-compact) .weather-status {
    display: none;
  }

  :global(.widget-size-compact) .weather-temperature {
    font-size: 62px;
  }

  @media (max-width: 620px) {
    .weather-current,
    .weather-metric-grid {
      grid-template-columns: 1fr;
    }

    .weather-quick-stats {
      display: grid;
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .weather-metric-grid > div,
    .weather-metric-grid > div:first-child {
      border-top: 1px solid var(--border);
      border-left: 0;
    }

    .weather-metric-grid > div:first-child {
      border-top: 0;
    }

    .weather-day-list > div {
      grid-template-columns: 58px minmax(0, 1fr) auto;
      padding-block: 10px;
    }

    .weather-day-list small {
      display: none;
    }

    .weather-detail-hero {
      grid-template-columns: auto auto;
    }

    .weather-detail-hero > div {
      grid-column: 1 / -1;
    }
  }
</style>

<script lang="ts">
  import Cloud from "lucide-svelte/icons/cloud";
  import CloudFog from "lucide-svelte/icons/cloud-fog";
  import CloudLightning from "lucide-svelte/icons/cloud-lightning";
  import CloudRain from "lucide-svelte/icons/cloud-rain";
  import CloudSun from "lucide-svelte/icons/cloud-sun";
  import MapPin from "lucide-svelte/icons/map-pin";
  import Settings2 from "lucide-svelte/icons/settings-2";
  import Snowflake from "lucide-svelte/icons/snowflake";
  import Sun from "lucide-svelte/icons/sun";
  import X from "lucide-svelte/icons/x";
  import { onDestroy, onMount } from "svelte";
  import { updateUserSettings, type UserSettings } from "$lib/api";
  import {
    fetchWeatherSnapshot,
    searchWeatherLocations,
    weatherCodeLabel,
    type TemperatureUnit,
    type WeatherSnapshot,
  } from "$lib/weather";

  interface SidebarUtilityPreferences {
    timezones: string[];
    unit: TemperatureUnit;
  }

  let {
    settings,
    onToast,
    onSettingsChange,
  }: {
    settings: UserSettings;
    onToast: (message: string) => void;
    onSettingsChange: (settings: UserSettings) => void;
  } = $props();

  let timezones = $state<string[]>([]);
  let unit = $state<TemperatureUnit>("celsius");
  let weather = $state.raw<WeatherSnapshot | null>(null);
  let weatherError = $state("");
  let weatherLoading = $state(false);
  let now = $state(new Date());
  let draftZones = $state("");
  let draftUnit = $state<TemperatureUnit>("celsius");
  let formError = $state("");
  let saving = $state(false);
  let utilityDialog = $state<HTMLDialogElement>();
  let searchController: AbortController | undefined;
  let weatherController: AbortController | undefined;
  let timer: number | undefined;

  const storageKey = () => `pandan-sidebar-utilities:${settings.user_id}`;
  let SidebarWeatherIcon = $derived(
    weatherIcon(weather?.current.weatherCode ?? 0),
  );
  onMount(() => {
    const hasLegacyPreferences = localStorage.getItem(storageKey()) !== null;
    const saved = readPreferences();
    timezones = saved.timezones;
    unit = saved.unit;
    if (hasLegacyPreferences) void migrateLegacyPreferences(saved);
    void loadDesignatedWeather();
    timer = window.setInterval(() => (now = new Date()), 1_000);
  });

  onDestroy(() => {
    if (timer) window.clearInterval(timer);
    searchController?.abort();
    weatherController?.abort();
  });

  function captureUtilityDialog(node: HTMLDialogElement) {
    utilityDialog = node;
    return () => (utilityDialog = undefined);
  }

  function readPreferences(): SidebarUtilityPreferences {
    const savedTimezones = settings.sidebar_timezones.filter((timezone) =>
      isValidTimezone(timezone),
    );
    const fallback: SidebarUtilityPreferences = {
      timezones:
        savedTimezones.length > 0
          ? savedTimezones.slice(0, 5)
          : [isValidTimezone(settings.timezone) ? settings.timezone : "UTC"],
      unit: settings.temperature_unit,
    };
    try {
      const raw = localStorage.getItem(storageKey());
      if (!raw) return fallback;
      const parsed = JSON.parse(raw) as Partial<SidebarUtilityPreferences>;
      const zones = Array.isArray(parsed.timezones)
        ? parsed.timezones.filter(
            (value): value is string =>
              typeof value === "string" && isValidTimezone(value),
          )
        : [];
      return {
        timezones: zones.length > 0 ? zones.slice(0, 5) : fallback.timezones,
        unit:
          parsed.unit === "celsius" || parsed.unit === "fahrenheit"
            ? parsed.unit
            : fallback.unit,
      };
    } catch {
      return fallback;
    }
  }

  function openSettings() {
    draftZones = timezones.join("\n");
    draftUnit = unit;
    formError = "";
    utilityDialog?.showModal();
  }

  function persistPreferences(preferences: SidebarUtilityPreferences) {
    return updateUserSettings({
      display_name: settings.display_name,
      location: settings.location,
      timezone: settings.timezone,
      sidebar_timezones: preferences.timezones,
      temperature_unit: preferences.unit,
      lines_default_visibility: settings.lines_default_visibility,
    });
  }

  async function migrateLegacyPreferences(
    preferences: SidebarUtilityPreferences,
  ) {
    try {
      const updated = await persistPreferences(preferences);
      timezones = updated.sidebar_timezones;
      unit = updated.temperature_unit;
      localStorage.removeItem(storageKey());
      onSettingsChange(updated);
    } catch {
      // Keep the local copy available until the account-backed save succeeds.
    }
  }

  async function saveSettings(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    const nextZones = [
      ...new Set(
        draftZones
          .split(/[\n,]/)
          .map((value) => value.trim())
          .filter(Boolean),
      ),
    ];
    if (nextZones.length === 0) {
      formError = "Add at least one IANA timezone.";
      return;
    }
    if (nextZones.length > 5) {
      formError = "The sidebar clock supports up to five timezones.";
      return;
    }
    const invalid = nextZones.find((value) => !isValidTimezone(value));
    if (invalid) {
      formError = `${invalid} is not a valid IANA timezone.`;
      return;
    }

    saving = true;
    formError = "";
    try {
      const updated = await persistPreferences({
        timezones: nextZones,
        unit: draftUnit,
      });
      timezones = updated.sidebar_timezones;
      unit = updated.temperature_unit;
      localStorage.removeItem(storageKey());
      onSettingsChange(updated);
      utilityDialog?.close();
      onToast("Sidebar clocks and weather saved to your account");
      void loadDesignatedWeather();
    } catch (reason: unknown) {
      formError =
        reason instanceof Error
          ? reason.message
          : "Unable to save sidebar monitor";
    } finally {
      saving = false;
    }
  }

  async function loadDesignatedWeather() {
    const city = settings.location.trim();
    searchController?.abort();
    weatherController?.abort();
    weather = null;
    weatherError = "";
    if (city.length < 2) {
      weatherError = "Set a city in user settings";
      return;
    }
    searchController = new AbortController();
    weatherController = new AbortController();
    weatherLoading = true;
    try {
      const matches = await searchWeatherLocations(
        city,
        searchController.signal,
      );
      const location = matches[0];
      if (!location) {
        weatherError = `No weather match for ${city}`;
        return;
      }
      weather = await fetchWeatherSnapshot(
        location,
        unit,
        weatherController.signal,
      );
    } catch (reason: unknown) {
      if ((reason as Error).name !== "AbortError") {
        weatherError =
          reason instanceof Error ? reason.message : "Weather unavailable";
      }
    } finally {
      weatherLoading = false;
    }
  }

  function isValidTimezone(value: string) {
    try {
      new Intl.DateTimeFormat("en", { timeZone: value }).format();
      return true;
    } catch {
      return false;
    }
  }

  function formatTime(timezone: string) {
    return new Intl.DateTimeFormat("en", {
      timeZone: timezone,
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).format(now);
  }

  function formatZone(timezone: string) {
    return timezone.split("/").at(-1)?.replaceAll("_", " ") ?? timezone;
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
</script>

<section
  class="sidebar-utilities"
  aria-label="World clocks and weather"
  data-od-id="sidebar-utilities"
>
  <div class="sidebar-utility-heading">
    <span>Local monitor</span>
    <button
      type="button"
      aria-label="Configure sidebar clocks and weather"
      aria-describedby="sidebar-desc-monitor"
      onclick={openSettings}
      data-sidebar-title="Local monitor"
      data-sidebar-description="Choose the timezones, city, and temperature unit shown in the sidebar."
      data-od-id="configure-sidebar-utilities"
    >
      <Settings2 size={15} strokeWidth={1.7} aria-hidden="true" />
    </button>
    <span id="sidebar-desc-monitor" class="sr-only"
      >Choose the timezones, city, and temperature unit shown in the
      sidebar.</span
    >
  </div>

  <div class="sidebar-clocks" data-od-id="sidebar-digital-clocks">
    {#each timezones as timezone (timezone)}
      <div
        class="sidebar-clock"
        data-sidebar-title={formatZone(timezone)}
        data-sidebar-description={`Current local time in ${timezone}.`}
      >
        <strong class="mono">{formatTime(timezone)}</strong>
        <span>{formatZone(timezone)}</span>
      </div>
    {/each}
  </div>

  <button
    class="sidebar-weather"
    type="button"
    aria-describedby="sidebar-desc-weather"
    onclick={openSettings}
    data-sidebar-title="Weather"
    data-sidebar-description="View current conditions and configure the sidebar monitor."
    data-od-id="sidebar-current-weather"
  >
    {#if weather}
      <SidebarWeatherIcon size={19} strokeWidth={1.55} aria-hidden="true" />
      <span>
        <strong>{weather.location.name}</strong>
        <small>{weatherCodeLabel(weather.current.weatherCode)}</small>
      </span>
      <strong class="mono sidebar-weather-temp"
        >{Math.round(weather.current.temperature)}°</strong
      >
    {:else}
      <MapPin size={18} strokeWidth={1.6} aria-hidden="true" />
      <span>
        <strong
          >{weatherLoading
            ? `Loading ${settings.location}`
            : settings.location || "Set your city"}</strong
        >
        <small>{weatherError || "Current conditions"}</small>
      </span>
    {/if}
  </button>
  <span id="sidebar-desc-weather" class="sr-only"
    >View current conditions and configure the sidebar monitor.</span
  >
</section>

<dialog
  class="settings-dialog sidebar-utility-dialog"
  {@attach captureUtilityDialog}
  onclick={(event) => event.target === utilityDialog && utilityDialog.close()}
  data-od-id="sidebar-utility-settings"
>
  <div class="settings-heading">
    <div>
      <h2>Sidebar monitor</h2>
      <p>Independent clocks and weather for your saved city.</p>
    </div>
    <button
      class="ui-button ui-button--ghost ui-button--icon dialog-close"
      type="button"
      aria-label="Close sidebar monitor settings"
      onclick={() => utilityDialog?.close()}
    >
      <X size={18} strokeWidth={1.8} aria-hidden="true" />
    </button>
  </div>
  <form class="settings-form sidebar-utility-form" onsubmit={saveSettings}>
    <label for="sidebar-timezones">IANA timezones</label>
    <textarea
      id="sidebar-timezones"
      class="text-input"
      bind:value={draftZones}
      rows="5"
      placeholder="America/New_York&#10;Europe/London"></textarea>
    <p class="field-note">
      One per line, up to five. Examples: Asia/Tokyo, Europe/Paris.
    </p>
    <p class="field-note">
      Weather uses <strong>{settings.location}</strong>. Change this in User
      settings.
    </p>

    <label for="sidebar-weather-unit">Temperature unit</label>
    <select
      id="sidebar-weather-unit"
      class="select-input"
      bind:value={draftUnit}
    >
      <option value="celsius">Celsius</option>
      <option value="fahrenheit">Fahrenheit</option>
    </select>

    {#if formError}<p class="form-error" role="alert">{formError}</p>{/if}
    <div class="settings-actions sidebar-utility-actions">
      <button
        class="ui-button ui-button--secondary secondary-btn"
        type="button"
        disabled={saving}
        onclick={() => utilityDialog?.close()}>Cancel</button
      >
      <button
        class="ui-button ui-button--primary primary-btn"
        type="submit"
        disabled={saving}>{saving ? "Saving…" : "Save monitor"}</button
      >
    </div>
  </form>
</dialog>

<style>
  .sidebar-utilities {
    min-width: 0;
    display: grid;
    gap: 7px;
    padding-bottom: 10px;
    color: var(--fg);
    border-bottom: 1px solid color-mix(in oklch, var(--fg) 10%, transparent);
  }

  .sidebar-utility-heading,
  .sidebar-clock,
  .sidebar-weather {
    display: flex;
    align-items: center;
  }

  .sidebar-utility-heading {
    justify-content: space-between;
    min-height: 30px;
    padding-left: 8px;
    color: color-mix(in oklch, var(--fg) 62%, transparent);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .sidebar-utility-heading button {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--fg);
  }

  .sidebar-clocks {
    display: grid;
    gap: 1px;
  }

  .sidebar-clock {
    min-height: 31px;
    justify-content: space-between;
    gap: 10px;
    padding: 3px 8px;
  }

  .sidebar-clock strong {
    color: var(--fg);
    font-size: 16px;
    font-weight: 540;
    letter-spacing: -0.04em;
  }

  .sidebar-clock span {
    min-width: 0;
    overflow: hidden;
    color: color-mix(in oklch, var(--fg) 64%, transparent);
    font-size: 9px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sidebar-weather {
    width: 100%;
    min-height: 52px;
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: 9px;
    padding: 7px 8px;
    border: 1px solid color-mix(in oklch, var(--fg) 16%, transparent);
    background: color-mix(in oklch, var(--fg) 5%, transparent);
    color: var(--fg);
    text-align: left;
  }

  .sidebar-weather:hover {
    border-color: color-mix(in oklch, var(--fg) 32%, transparent);
    background: color-mix(in oklch, var(--fg) 10%, transparent);
  }

  .sidebar-weather > span {
    min-width: 0;
    display: grid;
  }

  .sidebar-weather strong {
    overflow: hidden;
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sidebar-weather small {
    overflow: hidden;
    color: color-mix(in oklch, var(--fg) 62%, transparent);
    font-size: 9px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sidebar-weather .sidebar-weather-temp {
    font-size: 16px;
    font-weight: 540;
  }

  .sidebar-utility-form {
    align-content: start;
    display: grid;
    gap: 10px;
    overflow-y: auto;
    padding: 22px 24px 24px;
  }

  .sidebar-utility-actions {
    justify-content: flex-end;
    margin-top: 8px;
  }

  @media (max-width: 980px) and (min-width: 721px) {
    .sidebar-utilities {
      justify-items: center;
    }

    .sidebar-utility-heading > span,
    .sidebar-clock span,
    .sidebar-weather > span,
    .sidebar-weather-temp {
      display: none;
    }

    .sidebar-utility-heading {
      justify-content: center;
      padding: 0;
    }

    .sidebar-clock {
      min-height: 28px;
      justify-content: center;
      padding: 0;
    }

    .sidebar-clock strong {
      font-size: 11px;
    }

    .sidebar-weather {
      width: 44px;
      min-height: 44px;
      grid-template-columns: 1fr;
      place-items: center;
      padding: 0;
    }
  }
</style>

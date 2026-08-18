export type TemperatureUnit = "celsius" | "fahrenheit";

export interface WeatherLocation {
  id: number;
  name: string;
  admin1: string;
  country: string;
  latitude: number;
  longitude: number;
  timezone: string;
}

export interface WeatherCurrent {
  time: string;
  temperature: number;
  apparentTemperature: number;
  humidity: number;
  precipitation: number;
  weatherCode: number;
  isDay: boolean;
  cloudCover: number;
  pressure: number;
  windSpeed: number;
  windDirection: number;
  windGusts: number;
}

export interface WeatherHour {
  time: string;
  temperature: number;
  precipitationProbability: number;
  weatherCode: number;
  windSpeed: number;
}

export interface WeatherDay {
  date: string;
  weatherCode: number;
  temperatureMax: number;
  temperatureMin: number;
  precipitationProbability: number;
  sunrise: string;
  sunset: string;
}

export interface WeatherSnapshot {
  location: WeatherLocation;
  unit: TemperatureUnit;
  temperatureSymbol: "°C" | "°F";
  windUnit: "km/h" | "mph";
  precipitationUnit: "mm" | "in";
  current: WeatherCurrent;
  hourly: WeatherHour[];
  daily: WeatherDay[];
  fetchedAt: string;
}

interface GeocodingResponse {
  results?: Array<{
    id: number;
    name: string;
    admin1?: string;
    country?: string;
    latitude: number;
    longitude: number;
    timezone: string;
  }>;
  reason?: string;
}

interface ForecastResponse {
  current?: Record<string, number | string>;
  hourly?: Record<string, Array<number | string>>;
  daily?: Record<string, Array<number | string>>;
  reason?: string;
}

export function isWeatherLocation(value: unknown): value is WeatherLocation {
  if (!value || typeof value !== "object") return false;
  const location = value as Partial<WeatherLocation>;
  return (
    typeof location.id === "number" &&
    typeof location.name === "string" &&
    typeof location.latitude === "number" &&
    typeof location.longitude === "number" &&
    typeof location.timezone === "string"
  );
}

export async function searchWeatherLocations(
  query: string,
  signal?: AbortSignal,
): Promise<WeatherLocation[]> {
  const name = query.trim();
  if (name.length < 2) return [];
  const params = new URLSearchParams({
    name,
    count: "8",
    language: "en",
    format: "json",
  });
  const response = await fetch(
    `https://geocoding-api.open-meteo.com/v1/search?${params}`,
    { signal },
  );
  const payload = (await response.json()) as GeocodingResponse;
  if (!response.ok) {
    throw new Error(payload.reason ?? "City search is unavailable");
  }
  return (payload.results ?? []).map((result) => ({
    id: result.id,
    name: result.name,
    admin1: result.admin1 ?? "",
    country: result.country ?? "",
    latitude: result.latitude,
    longitude: result.longitude,
    timezone: result.timezone,
  }));
}

export async function fetchWeatherSnapshot(
  location: WeatherLocation,
  unit: TemperatureUnit,
  signal?: AbortSignal,
): Promise<WeatherSnapshot> {
  const params = new URLSearchParams({
    latitude: String(location.latitude),
    longitude: String(location.longitude),
    current:
      "temperature_2m,relative_humidity_2m,apparent_temperature,is_day,precipitation,weather_code,cloud_cover,surface_pressure,wind_speed_10m,wind_direction_10m,wind_gusts_10m",
    hourly:
      "temperature_2m,precipitation_probability,weather_code,wind_speed_10m",
    daily:
      "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max,sunrise,sunset",
    timezone: "auto",
    forecast_days: "7",
    temperature_unit: unit,
    wind_speed_unit: unit === "fahrenheit" ? "mph" : "kmh",
    precipitation_unit: unit === "fahrenheit" ? "inch" : "mm",
  });
  const response = await fetch(
    `https://api.open-meteo.com/v1/forecast?${params}`,
    { signal },
  );
  const payload = (await response.json()) as ForecastResponse;
  if (!response.ok || !payload.current || !payload.hourly || !payload.daily) {
    throw new Error(
      payload.reason ?? `Weather for ${location.name} is unavailable`,
    );
  }

  const current = payload.current;
  const hourly = payload.hourly;
  const daily = payload.daily;
  const hourlyTimes = stringValues(hourly.time);
  const dailyTimes = stringValues(daily.time);

  return {
    location,
    unit,
    temperatureSymbol: unit === "fahrenheit" ? "°F" : "°C",
    windUnit: unit === "fahrenheit" ? "mph" : "km/h",
    precipitationUnit: unit === "fahrenheit" ? "in" : "mm",
    current: {
      time: stringValue(current.time),
      temperature: numberValue(current.temperature_2m),
      apparentTemperature: numberValue(current.apparent_temperature),
      humidity: numberValue(current.relative_humidity_2m),
      precipitation: numberValue(current.precipitation),
      weatherCode: numberValue(current.weather_code),
      isDay: numberValue(current.is_day) === 1,
      cloudCover: numberValue(current.cloud_cover),
      pressure: numberValue(current.surface_pressure),
      windSpeed: numberValue(current.wind_speed_10m),
      windDirection: numberValue(current.wind_direction_10m),
      windGusts: numberValue(current.wind_gusts_10m),
    },
    hourly: hourlyTimes.map((time, index) => ({
      time,
      temperature: numberAt(hourly.temperature_2m, index),
      precipitationProbability: numberAt(
        hourly.precipitation_probability,
        index,
      ),
      weatherCode: numberAt(hourly.weather_code, index),
      windSpeed: numberAt(hourly.wind_speed_10m, index),
    })),
    daily: dailyTimes.map((date, index) => ({
      date,
      weatherCode: numberAt(daily.weather_code, index),
      temperatureMax: numberAt(daily.temperature_2m_max, index),
      temperatureMin: numberAt(daily.temperature_2m_min, index),
      precipitationProbability: numberAt(
        daily.precipitation_probability_max,
        index,
      ),
      sunrise: stringAt(daily.sunrise, index),
      sunset: stringAt(daily.sunset, index),
    })),
    fetchedAt: new Date().toISOString(),
  };
}

export function weatherCodeLabel(code: number): string {
  if (code === 0) return "Clear";
  if (code <= 2) return "Partly cloudy";
  if (code === 3) return "Overcast";
  if (code === 45 || code === 48) return "Fog";
  if (code >= 51 && code <= 57) return "Drizzle";
  if ((code >= 61 && code <= 67) || (code >= 80 && code <= 82)) return "Rain";
  if ((code >= 71 && code <= 77) || (code >= 85 && code <= 86)) return "Snow";
  if (code >= 95) return "Thunderstorm";
  return "Mixed conditions";
}

export function compassDirection(degrees: number): string {
  const points = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
  return points[Math.round(((degrees % 360) + 360) / 45) % 8];
}

function numberValue(value: number | string | undefined): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

function stringValue(value: number | string | undefined): string {
  return typeof value === "string" ? value : "";
}

function stringValues(values: Array<number | string> | undefined): string[] {
  return (values ?? []).map((value) => String(value));
}

function numberAt(
  values: Array<number | string> | undefined,
  index: number,
): number {
  return numberValue(values?.[index]);
}

function stringAt(
  values: Array<number | string> | undefined,
  index: number,
): string {
  return stringValue(values?.[index]);
}

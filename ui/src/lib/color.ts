export type HexColor = `#${string}`;

export interface HslColor {
  hue: number;
  saturation: number;
  lightness: number;
}

export interface RgbColor {
  red: number;
  green: number;
  blue: number;
}

export interface ColorPreset {
  value: HexColor;
  label: string;
}

export const DEFAULT_COLOR_PRESETS: readonly ColorPreset[] = [
  { value: "#2DD4BF", label: "Signal teal" },
  { value: "#60A5FA", label: "Terminal blue" },
  { value: "#A78BFA", label: "Violet" },
  { value: "#FB7185", label: "Rose" },
  { value: "#FB923C", label: "Orange" },
  { value: "#FBBF24", label: "Amber" },
  { value: "#A3E635", label: "Lime" },
  { value: "#94A3B8", label: "Slate" },
];

export function clampColorChannel(
  value: number,
  minimum: number,
  maximum: number,
): number {
  return Math.min(maximum, Math.max(minimum, Math.round(value)));
}

export function isHexColor(value: unknown): value is HexColor {
  return typeof value === "string" && /^#[0-9A-F]{6}$/i.test(value);
}

export function normalizeHexColor(value: string): HexColor | null {
  const candidate = value.trim().startsWith("#")
    ? value.trim()
    : `#${value.trim()}`;
  return isHexColor(candidate) ? (candidate.toUpperCase() as HexColor) : null;
}

export function hexToRgb(value: HexColor): RgbColor {
  return {
    red: Number.parseInt(value.slice(1, 3), 16),
    green: Number.parseInt(value.slice(3, 5), 16),
    blue: Number.parseInt(value.slice(5, 7), 16),
  };
}

export function rgbToHex(red: number, green: number, blue: number): HexColor {
  const channel = (value: number) =>
    clampColorChannel(value, 0, 255)
      .toString(16)
      .padStart(2, "0")
      .toUpperCase();
  return `#${channel(red)}${channel(green)}${channel(blue)}`;
}

export function hexToHsl(value: HexColor): HslColor {
  const rgb = hexToRgb(value);
  const red = rgb.red / 255;
  const green = rgb.green / 255;
  const blue = rgb.blue / 255;
  const maximum = Math.max(red, green, blue);
  const minimum = Math.min(red, green, blue);
  const delta = maximum - minimum;
  let hue = 0;
  if (delta !== 0) {
    if (maximum === red) hue = 60 * (((green - blue) / delta) % 6);
    else if (maximum === green) hue = 60 * ((blue - red) / delta + 2);
    else hue = 60 * ((red - green) / delta + 4);
  }
  if (hue < 0) hue += 360;
  const lightness = (maximum + minimum) / 2;
  const saturation =
    delta === 0 ? 0 : delta / (1 - Math.abs(2 * lightness - 1));
  return {
    hue: Math.round(hue) % 360,
    saturation: Math.round(saturation * 100),
    lightness: Math.round(lightness * 100),
  };
}

export function hslToHex(
  hue: number,
  saturation: number,
  lightness: number,
): HexColor {
  const normalizedHue = ((Math.round(hue) % 360) + 360) % 360;
  const normalizedSaturation = clampColorChannel(saturation, 0, 100) / 100;
  const normalizedLightness = clampColorChannel(lightness, 0, 100) / 100;
  const chroma =
    (1 - Math.abs(2 * normalizedLightness - 1)) * normalizedSaturation;
  const component = chroma * (1 - Math.abs(((normalizedHue / 60) % 2) - 1));
  const offset = normalizedLightness - chroma / 2;
  let red = 0;
  let green = 0;
  let blue = 0;
  if (normalizedHue < 60) [red, green] = [chroma, component];
  else if (normalizedHue < 120) [red, green] = [component, chroma];
  else if (normalizedHue < 180) [green, blue] = [chroma, component];
  else if (normalizedHue < 240) [green, blue] = [component, chroma];
  else if (normalizedHue < 300) [red, blue] = [component, chroma];
  else [red, blue] = [chroma, component];
  return rgbToHex(
    (red + offset) * 255,
    (green + offset) * 255,
    (blue + offset) * 255,
  );
}

export function rotateHexHue(value: HexColor, offset: number): HexColor {
  const hsl = hexToHsl(value);
  return hslToHex(hsl.hue + offset, hsl.saturation, hsl.lightness);
}

export function adjustHexLightness(value: HexColor, offset: number): HexColor {
  const hsl = hexToHsl(value);
  return hslToHex(
    hsl.hue,
    hsl.saturation,
    clampColorChannel(hsl.lightness + offset, 0, 100),
  );
}

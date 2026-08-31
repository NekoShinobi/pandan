import { isHexColor, normalizeHexColor } from "$lib/color";

const legacyLabelColors: Record<string, string> = {
  accent: "var(--accent)",
  blue: "oklch(62% 0.14 245)",
  amber: "oklch(72% 0.14 78)",
  red: "var(--danger)",
  violet: "oklch(58% 0.12 315)",
  gray: "var(--muted)",
};

export function kanbanLabelColorCss(value: string): string {
  if (isHexColor(value)) return normalizeHexColor(value) ?? value;
  return legacyLabelColors[value] ?? "var(--muted)";
}

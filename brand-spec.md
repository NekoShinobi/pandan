# Pandan brand specification

Pandan is a near-black, terminal-inspired product system with crisp translucent structure, restrained signal green, and monospaced working typography.

## Tokens

```css
:root {
  --bg: oklch(11% 0.012 165);
  --surface: oklch(15% 0.014 165);
  --fg: oklch(91% 0.016 150);
  --muted: oklch(65% 0.02 155);
  --border: oklch(38% 0.025 155);
  --accent: oklch(79% 0.16 145);

  --font-display: "Avenir Next", "Segoe UI Variable Display", "Helvetica Neue", sans-serif;
  --font-body: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  --font-mono: "JetBrains Mono", "IBM Plex Mono", ui-monospace, Menlo, monospace;
}
```

## Visual language

- Use near-black surfaces with fine, green-shifted borders; avoid pure black, pure white, soft shadows, and ornamental gradients.
- Set utility text, controls, navigation, and data in the monospaced stack so the interface reads like an operational terminal.
- Reserve the green accent for the current signal and the primary action, with no competing accent hue.
- Keep corners square on authenticated terminal surfaces and let translucency reveal wallpaper without reducing text contrast.
- Use restrained state motion: fast color feedback, the shared upward dialog entrance, and explicit reduced-motion fallbacks for transforms.

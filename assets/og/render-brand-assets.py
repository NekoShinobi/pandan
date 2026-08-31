#!/usr/bin/env python3
"""Render Pandan's link-preview card and application icons.

This is an offline design tool, not part of the build or CI. Pandan ships the
rendered PNGs in `ui/static/`; re-run this only when the brand card changes.

    pip install pillow
    python3 assets/og/render-brand-assets.py --font-dir /path/to/jetbrains-mono/ttf

Fonts: JetBrains Mono (SIL OFL 1.1), the first family in the interface's
`--font-mono` stack. The TTFs are not vendored; point `--font-dir` at a local
copy or install the family system-wide. Colours are the authenticated terminal
tokens from `ui/src/app.css`, converted from OKLCH to sRGB below so the card
matches the signed-in shell exactly.
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw, ImageFont
except ModuleNotFoundError:  # pragma: no cover - tooling guard
    sys.exit("this tool needs Pillow: pip install pillow")

REPO_ROOT = Path(__file__).resolve().parents[2]
STATIC_DIR = REPO_ROOT / "ui" / "static"

# Supersampling factor. Geometry is authored in final 1200x630 coordinates and
# drawn at SCALE, then resampled down, so hairlines and text stay crisp.
SCALE = 2

CARD_W, CARD_H = 1200, 630


def oklch(lightness: float, chroma: float, hue_deg: float) -> tuple[int, int, int]:
    """Convert an `ui/src/app.css` OKLCH token to 8-bit sRGB."""
    hue = math.radians(hue_deg)
    a, b = chroma * math.cos(hue), chroma * math.sin(hue)
    l_ = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m_ = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s_ = (lightness - 0.0894841775 * a - 1.2914855480 * b) ** 3
    linear = (
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    )

    def encode(channel: float) -> int:
        channel = min(max(channel, 0.0), 1.0)
        srgb = (
            1.055 * channel ** (1 / 2.4) - 0.055
            if channel > 0.0031308
            else 12.92 * channel
        )
        return round(min(max(srgb, 0.0), 1.0) * 255)

    return tuple(encode(channel) for channel in linear)  # type: ignore[return-value]


BG = oklch(0.11, 0.012, 165)  # --bg
SURFACE = oklch(0.15, 0.014, 165)  # --surface
FG = oklch(0.91, 0.016, 150)  # --fg
MUTED = oklch(0.65, 0.020, 155)  # --muted
BORDER = oklch(0.38, 0.025, 155)  # --border
ACCENT = oklch(0.79, 0.160, 145)  # --accent


def rgba(color: tuple[int, int, int], alpha: float = 1.0) -> tuple[int, int, int, int]:
    return (*color, round(min(max(alpha, 0.0), 1.0) * 255))


def px(*values: float) -> tuple[float, ...]:
    """Author-space pixels to render-space pixels."""
    return tuple(value * SCALE for value in values)


class Layer:
    """A transparent overlay drawn in author space and composited in one pass.

    `ImageDraw` writes raw RGBA rather than blending, so every translucent
    element has to land on its own layer and be composited onto the card.
    """

    def __init__(self, base: Image.Image) -> None:
        self._base = base
        self.image = Image.new("RGBA", base.size, (0, 0, 0, 0))
        self.draw = ImageDraw.Draw(self.image)

    def rectangle(self, box, *, fill=None, outline=None, width: float = 1) -> None:
        self.draw.rectangle(px(*box), fill=fill, outline=outline, width=round(width * SCALE))

    def line(self, points, *, fill, width: float = 1) -> None:
        self.draw.line(px(*points), fill=fill, width=round(width * SCALE))

    def commit(self) -> None:
        self._base.alpha_composite(self.image)


class Type:
    """Monospaced text with CSS-style tracking, authored in final-size pixels."""

    WEIGHTS = {
        "regular": "JetBrainsMono-Regular.ttf",
        "medium": "JetBrainsMono-Medium.ttf",
        "bold": "JetBrainsMono-Bold.ttf",
    }

    def __init__(self, fonts: dict[str, Path]) -> None:
        self._paths = fonts
        self._cache: dict[tuple[str, int], ImageFont.FreeTypeFont] = {}

    def font(self, weight: str, size: float) -> ImageFont.FreeTypeFont:
        key = (weight, round(size * SCALE))
        if key not in self._cache:
            self._cache[key] = ImageFont.truetype(str(self._paths[weight]), key[1])
        return self._cache[key]

    def width(self, text: str, weight: str, size: float, tracking: float = 0.0) -> float:
        font = self.font(weight, size)
        advance = sum(font.getlength(char) for char in text)
        return advance / SCALE + tracking * max(len(text) - 1, 0)

    def draw(
        self,
        layer: Layer,
        xy: tuple[float, float],
        text: str,
        *,
        weight: str = "regular",
        size: float = 16,
        fill: tuple[int, ...],
        tracking: float = 0.0,
        anchor: str = "lt",
    ) -> None:
        """Draw `text` glyph by glyph so tracking behaves like CSS letter-spacing.

        Every glyph is placed on the shared baseline derived from the font's own
        metrics. Pillow's `t`/`m` anchors measure each glyph's ink box, which
        would float commas and periods up out of the line.
        """
        font = self.font(weight, size)
        ascent, descent = font.getmetrics()
        horizontal, vertical = anchor[0], anchor[1]

        x = xy[0] * SCALE
        if horizontal == "m":
            x -= self.width(text, weight, size, tracking) * SCALE / 2
        elif horizontal == "r":
            x -= self.width(text, weight, size, tracking) * SCALE

        baseline = xy[1] * SCALE
        if vertical == "t":
            baseline += ascent
        elif vertical == "m":
            baseline += (ascent - descent) / 2
        elif vertical == "b":
            baseline -= descent

        for char in text:
            layer.draw.text((x, baseline), char, font=font, fill=fill, anchor="ls")
            x += font.getlength(char) + tracking * SCALE


FONT_SEARCH = (
    Path(__file__).resolve().parent / "fonts",
    Path("/usr/share/fonts/truetype/jetbrains-mono"),
    Path("/usr/share/fonts/TTF"),
    Path.home() / ".local/share/fonts",
    Path.home() / "Library/Fonts",
)


def load_fonts(font_dir: Path | None) -> dict[str, Path]:
    candidates = (font_dir, *FONT_SEARCH) if font_dir else FONT_SEARCH
    for directory in candidates:
        if directory and all(
            (directory / name).is_file() for name in Type.WEIGHTS.values()
        ):
            return {weight: directory / name for weight, name in Type.WEIGHTS.items()}
    searched = "\n  ".join(str(path) for path in candidates if path)
    sys.exit(
        "JetBrains Mono was not found. Pass --font-dir, or install the family into one of:\n  "
        + searched
    )


def radial_glow(
    size: tuple[int, int],
    center: tuple[float, float],
    radius: float,
    color: tuple[int, int, int],
    alpha: float,
) -> Image.Image:
    """A soft radial falloff, built small and scaled up for a smooth gradient."""
    steps = 96
    small = Image.new("L", (steps * 2, steps * 2), 0)
    plot = ImageDraw.Draw(small)
    for step in range(steps, 0, -1):
        value = round(255 * alpha * (1 - step / steps) ** 2.2)
        plot.ellipse((steps - step, steps - step, steps + step, steps + step), fill=value)
    mask = small.resize((round(radius * 2), round(radius * 2)), Image.LANCZOS)
    layer = Image.new("RGBA", size, (*color, 0))
    layer.paste(
        Image.new("RGBA", mask.size, (*color, 255)),
        (round(center[0] - radius), round(center[1] - radius)),
        mask,
    )
    return layer


def draw_grid(base: Image.Image) -> None:
    """The faint dashboard-canvas ruling behind the card."""
    layer = Layer(base)
    line = rgba(FG, 0.05)
    for step in range(0, CARD_W, 48):
        layer.line((step, 0, step, CARD_H), fill=line)
    for step in range(0, CARD_H, 48):
        layer.line((0, step, CARD_W, step), fill=line)
    layer.commit()


def draw_canvas_wireframe(base: Image.Image, box: tuple[float, float, float, float]) -> None:
    """A wireframe of the dashboard canvas: GridStack tiles, never a screenshot."""
    layer = Layer(base)
    left, top, right, bottom = box
    columns, rows, gutter = 4, 6, 10
    cell_w = (right - left - gutter * (columns - 1)) / columns
    cell_h = (bottom - top - gutter * (rows - 1)) / rows

    # (column, row, column span, row span, accented)
    tiles = (
        (0, 0, 2, 2, True),
        (2, 0, 2, 1, False),
        (2, 1, 2, 2, False),
        (0, 2, 2, 1, False),
        (0, 3, 1, 2, False),
        (1, 3, 3, 2, False),
        (0, 5, 4, 1, False),
    )
    for column, row, span_x, span_y, accented in tiles:
        x0 = left + column * (cell_w + gutter)
        y0 = top + row * (cell_h + gutter)
        x1 = x0 + span_x * cell_w + (span_x - 1) * gutter
        y1 = y0 + span_y * cell_h + (span_y - 1) * gutter
        layer.rectangle(
            (x0, y0, x1, y1),
            fill=rgba(ACCENT, 0.09) if accented else rgba(FG, 0.03),
            outline=rgba(ACCENT, 0.5) if accented else rgba(BORDER, 0.75),
        )
        # Title and content rules, so each tile reads as a widget.
        rules = [(14, 0.46)] if y1 - y0 < 46 else [(15, 0.5), (30, 0.3), (43, 0.22)]
        for offset, extent in rules:
            if y0 + offset > y1 - 8:
                continue
            tint = rgba(ACCENT, 0.55) if accented and offset == rules[0][0] else None
            layer.line(
                (x0 + 13, y0 + offset, x0 + 13 + (x1 - x0 - 26) * extent, y0 + offset),
                fill=tint or rgba(MUTED, 0.3 if offset == rules[0][0] else 0.16),
            )
    layer.commit()


def render_card(type_: Type, output: Path) -> None:
    base = Image.new("RGBA", px(CARD_W, CARD_H), rgba(BG))
    draw_grid(base)
    base.alpha_composite(radial_glow(base.size, px(310, 80), 720 * SCALE, ACCENT, 0.14))
    base.alpha_composite(radial_glow(base.size, px(1140, 640), 560 * SCALE, ACCENT, 0.06))

    panel = (56, 44, 1144, 586)
    pad_x, right_x = 112, 1088

    frame = Layer(base)
    frame.rectangle(panel, fill=rgba(SURFACE, 0.86), outline=rgba(BORDER, 0.95))
    # Register marks at the panel corners.
    for x, y, dx, dy in (
        (panel[0], panel[1], 1, 1),
        (panel[2], panel[1], -1, 1),
        (panel[0], panel[3], 1, -1),
        (panel[2], panel[3], -1, -1),
    ):
        frame.line((x, y, x + 26 * dx, y), fill=rgba(ACCENT, 0.75))
        frame.line((x, y, x, y + 26 * dy), fill=rgba(ACCENT, 0.75))
    frame.line((pad_x, 200, right_x, 200), fill=rgba(BORDER, 0.9))
    frame.line((panel[0], 534, panel[2], 534), fill=rgba(BORDER, 0.9))

    glyph_top, glyph = 88, 66
    frame.rectangle(
        (pad_x, glyph_top, pad_x + glyph, glyph_top + glyph),
        fill=rgba(ACCENT, 0.1),
        outline=rgba(ACCENT, 0.6),
    )
    frame.commit()

    draw_canvas_wireframe(base, (772, 248, right_x, 514))

    ink = Layer(base)
    type_.draw(
        ink,
        (pad_x + glyph / 2, glyph_top + glyph / 2 + 1),
        "P>",
        weight="bold",
        size=27,
        fill=rgba(ACCENT),
        anchor="mm",
    )
    type_.draw(
        ink,
        (pad_x + glyph + 30, glyph_top + glyph / 2 + 1),
        "PANDAN",
        weight="bold",
        size=31,
        fill=rgba(FG),
        tracking=7.5,
        anchor="lm",
    )
    type_.draw(
        ink,
        (right_x, glyph_top + glyph / 2 + 1),
        "[ SELF-HOSTED ]",
        weight="medium",
        size=18,
        fill=rgba(MUTED),
        tracking=2.9,
        anchor="rm",
    )
    type_.draw(
        ink,
        (pad_x, 246),
        "[ PRIVATE WORKSPACE ]",
        weight="medium",
        size=19,
        fill=rgba(ACCENT),
        tracking=3.4,
        anchor="lt",
    )

    headline = ("Your private", "command center.")
    size, leading, tracking = 66, 76, -1.4
    for index, line in enumerate(headline):
        top = 288 + index * leading
        type_.draw(
            ink,
            (pad_x, top),
            line,
            weight="medium",
            size=size,
            fill=rgba(FG),
            tracking=tracking,
            anchor="lt",
        )
    # The terminal block cursor, resting after the last headline word and
    # squared off against the cap height rather than the ascender.
    cursor_x = pad_x + type_.width(headline[-1], "medium", size, tracking) + 20
    cursor_top = 288 + (len(headline) - 1) * leading
    ink.rectangle((cursor_x, cursor_top + 18, cursor_x + 17, cursor_top + 66), fill=rgba(ACCENT))

    body = (
        "A terminal-inspired home for the things",
        "you follow and do, behind your own login.",
    )
    for index, line in enumerate(body):
        type_.draw(
            ink,
            (pad_x, 462 + index * 33),
            line,
            weight="regular",
            size=22,
            fill=rgba(MUTED),
            anchor="lt",
        )

    # Status bar.
    type_.draw(
        ink,
        (pad_x, 552),
        "DASHBOARD · TASKS · KANBAN · CALENDAR · FEEDS · JOURNAL",
        weight="medium",
        size=16,
        fill=rgba(MUTED, 0.88),
        tracking=1.9,
        anchor="lt",
    )
    status = "SIGN-IN REQUIRED"
    type_.draw(
        ink,
        (right_x, 552),
        status,
        weight="medium",
        size=16,
        fill=rgba(MUTED, 0.88),
        tracking=1.9,
        anchor="rt",
    )
    indicator = right_x - type_.width(status, "medium", 16, 1.9) - 18
    ink.rectangle((indicator, 557, indicator + 8, 565), fill=rgba(ACCENT))
    ink.commit()

    base.convert("RGB").resize((CARD_W, CARD_H), Image.LANCZOS).save(
        output, "PNG", optimize=True
    )
    report(output)


def render_wordmark_icon(type_: Type, output: Path, size: int) -> None:
    """Render the text-only wordmark used by the native PWA launch screen."""
    base = Image.new("RGBA", (size * SCALE, size * SCALE), rgba(BG))
    layer = Layer(base)
    text = "Pandan"
    font_size = 92 * size / 512
    tracking = -0.04 * font_size
    font = type_.font("medium", font_size)
    _, ink_top, _, ink_bottom = font.getbbox(text, anchor="ls")
    baseline = size / 2 - (ink_top + ink_bottom) / (2 * SCALE)

    type_.draw(
        layer,
        (size / 2, baseline),
        text,
        weight="medium",
        size=font_size,
        fill=rgba(FG),
        tracking=tracking,
        anchor="ms",
    )
    layer.commit()

    base.convert("RGB").resize((size, size), Image.LANCZOS).save(
        output, "PNG", optimize=True
    )
    report(output)


def render_icon(type_: Type, output: Path, size: int, *, bleed: bool) -> None:
    """The `P>` mark as a square app icon; `bleed` fills the tile, as iOS expects."""
    scale = size / 64
    base = Image.new("RGBA", (size * SCALE, size * SCALE), rgba(BG))
    base.alpha_composite(radial_glow(base.size, (0, 0), size * SCALE * 1.3, ACCENT, 0.2))

    layer = Layer(base)
    if not bleed:
        inset = 3 * scale
        layer.rectangle(
            (inset, inset, size - inset - 1, size - inset - 1),
            fill=rgba(ACCENT, 0.08),
            outline=rgba(ACCENT, 0.75),
            width=max(1.0, scale),
        )
    type_.draw(
        layer,
        (size / 2, size / 2 + scale),
        "P>",
        weight="bold",
        size=26 * scale,
        fill=rgba(ACCENT),
        anchor="mm",
    )
    layer.commit()

    base.convert("RGB").resize((size, size), Image.LANCZOS).save(output, "PNG", optimize=True)
    report(output)


def report(output: Path) -> None:
    try:
        shown: Path | str = output.relative_to(REPO_ROOT)
    except ValueError:
        shown = output
    print(f"wrote {shown} ({output.stat().st_size / 1024:.0f} KB)")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--font-dir", type=Path, help="directory holding the JetBrains Mono TTFs")
    parser.add_argument("--out-dir", type=Path, default=STATIC_DIR)
    args = parser.parse_args()

    type_ = Type(load_fonts(args.font_dir))
    args.out_dir.mkdir(parents=True, exist_ok=True)
    render_card(type_, args.out_dir / "og-card.png")
    render_icon(type_, args.out_dir / "icon-192.png", 192, bleed=False)
    render_wordmark_icon(type_, args.out_dir / "icon-512.png", 512)
    render_icon(type_, args.out_dir / "icon-maskable-192.png", 192, bleed=True)
    render_icon(type_, args.out_dir / "icon-maskable-512.png", 512, bleed=True)
    render_icon(type_, args.out_dir / "apple-touch-icon.png", 180, bleed=True)
    # The tab favicon drops the frame; at 32px the border only crowds the mark.
    render_icon(type_, args.out_dir / "favicon-32.png", 32, bleed=True)


if __name__ == "__main__":
    main()

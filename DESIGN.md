# Pandan interface design rules

This file is the persistent visual and interaction contract for Pandan. Read it before changing frontend components or shared styles.

## Product language

- Preserve the terminal visual system: near-black surfaces, restrained green accents, monospaced utility text, crisp borders, and translucent structure over wallpaper.
- Reuse shared tokens and control classes from `ui/src/app.css`; do not create isolated component palettes.
- Interactive targets must be at least 44 px, keyboard accessible, visibly focused, and usable with reduced motion.

## Toggle switches

- Never use a native checkbox or a checkbox-shaped button as an application control in the Pandan interface.
- Represent every independent binary or multi-select state with a `<button type="button">` using `aria-pressed="true|false"` and the conventional switch appearance: an oval track with a circular thumb that slides left and right.
- Start from the shared `ui-toggle-button` and `ui-toggle-indicator` classes. `ui-toggle-indicator` is the switch track and its pseudo-element is the thumb. Feature-specific classes may adjust layout, but must not turn it back into a checkbox or checkmark control.
- The state must be communicated by `aria-pressed`, the thumb position, and a visible track change. Color alone is insufficient.
- Keep labels explicit and stable. When the visible label lacks context, add an `aria-label` that names the setting and its current state.
- Disable the button while a non-idempotent state change is pending, and update the pressed state only from authoritative or intentionally optimistic state.
- Use radio groups, selects, or segmented controls for mutually exclusive choices; toggle buttons are for independent states.

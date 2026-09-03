# Pandan interface design rules

This file is the persistent visual and interaction contract for Pandan. Read it before changing frontend components or shared styles.

## Product language

- Preserve the terminal visual system: near-black surfaces, restrained account-selected highlights (green by default), monospaced utility text, crisp borders, and translucent structure over wallpaper.
- Reuse shared tokens and control classes from `ui/src/app.css`; do not create isolated component palettes.
- Terminal colors derive from the root `--highlight-base` palette. Feature scopes may consume `--accent`, `--muted`, `--border`, and the shared terminal tokens, but must not hard-code a replacement highlight family.
- Interactive targets must be at least 44 px, keyboard accessible, visibly focused, and usable with reduced motion.

## Toggle switches

- Never use a native checkbox or a checkbox-shaped button as an application control in the Pandan interface.
- Represent every independent binary or multi-select state with a `<button type="button">` using `aria-pressed="true|false"` and the conventional switch appearance: an oval track with a circular thumb that slides left and right.
- Start from the shared `ui-toggle-button` and `ui-toggle-indicator` classes. `ui-toggle-indicator` is the switch track and its pseudo-element is the thumb. Feature-specific classes may adjust layout, but must not turn it back into a checkbox or checkmark control.
- The state must be communicated by `aria-pressed`, the thumb position, and a visible track change. Color alone is insufficient.
- Keep labels explicit and stable. When the visible label lacks context, add an `aria-label` that names the setting and its current state.
- Disable the button while a non-idempotent state change is pending, and update the pressed state only from authoritative or intentionally optimistic state.
- Use radio groups, selects, or segmented controls for mutually exclusive choices; toggle buttons are for independent states.

## Selects and dropdown menus

- Every native select trigger, option, and option group must resolve its background and foreground from the active theme scope. Use `--page-surface` with `--surface` as the fallback for the background and `--fg` for text; never rely on the browser's default popup colors.
- Terminal scopes must declare `color-scheme: dark` so browser-rendered dropdown chrome cannot fall back to a white menu. Light scopes keep the root light color scheme.
- Extend the shared select rules in `ui/src/app.css` instead of adding feature-specific dropdown palettes. Component rules may change spacing or geometry, but must preserve the shared surface and readable hover, focus, selected, and disabled states.

## Motion and dialogs

- Repeated non-dialog motion uses the primitives in `ui/src/lib/motion.svelte.ts`: `motionDisclosure` for height-revealing regions, `motionPopover` for floating surfaces, and the surface enter/exit pair for temporary inline composers. These primitives own the shared `cubic-bezier(0.2, 0, 0, 1)` timing, interruption cleanup, and reduced-motion behavior; do not add feature-specific keyframes or removal timers for the same patterns.
- Standard dialogs use the shared bidirectional motion defined in `ui/src/app.css`: they fade from transparent to opaque while moving from `-18px` on the Y axis to their resting position over `220ms` with `--ease-out`, then reverse those properties smoothly on exit. Their backdrop fades in and out over `180ms`, and the shared discrete `display` and `overlay` transitions keep the closing dialog in the top layer until its outro finishes.
- An animated modal must always own a paired enter and exit lifecycle. Every dismissal path — close control, Cancel, backdrop press, Escape, successful submission, and programmatic close — uses the same exit. Never add an entrance-only `[open]` animation, remove the modal from the DOM, or set `display: none` before the outro completes. Native dialogs may call `close()` directly because the shared discrete transitions retain them while they leave; a modal that cannot use the shared dialog transition must set an explicit leaving state and defer `close()` or unmounting until that state finishes. Reduced-motion users may close immediately.
- Dialogs must not add feature-specific entrance keyframes, scale effects, springs, or alternate travel distances. Component styles may define size, surface, and backdrop color without overriding the shared motion.
- Full-screen experiences such as the focus session may use a distinct transition when it communicates a screen-level state change. Nested settings panels enter upward over `240ms` or less.
- Every transform animation must have a reduced-motion fallback. The global reduced-motion rule removes standard dialog and panel movement.

## Scrollbars

- Every scrollbar is the shared custom one defined in `ui/src/app.css`: a `--scrollbar-size` gutter over a transparent track, a 1 px rail against the content edge, and a square thumb whose colour resolves from the scroll container's own `--fg` and `--bg`. The thumb brightens on hover and turns `--accent` only while it is being dragged.
- Compact floating result feeds use the shared `overlay-scroll-region` variant: a permanently reserved 5 px gutter, no visible rail, and a low-contrast thumb. Use it for the Bell notification feed and command-search results so their headers remain outside the scroll region and row widths never shift when overflow appears.
- Do not restyle a scrollbar per component. A component may only hide one it deliberately does not want, with `scrollbar-width: none` plus `::-webkit-scrollbar { display: none }`.
- Styling `::-webkit-scrollbar` opts an element out of macOS overlay scrollbars, so the scrollbar occupies real layout width. Every layout-level scroll container must therefore declare `scrollbar-gutter: stable` and appear in the `@supports not (scrollbar-gutter: stable)` fallback at the end of `ui/src/app.css`. A scroll container without a reserved gutter shifts its content sideways each time a page gains or loses scroll.

## Page headers and busy states

- Every product page opens with the same terminal header: `page-header` beside the page's own class, a `$ {page} --{view}` title rendered by `$lib/TypedHeading.svelte`, and a muted monospaced standfirst, ruled off from the body with a bottom border. Do not give one page a display-face title, a separate kicker line, or its own `h2` rule — the heading element belongs to the component, so its type lives on `.typed-heading` in `ui/src/app.css`.
- The title is the one place a page announces itself. Because every page title shares a font and a size, `TypedHeading` backspaces the outgoing one and types the incoming one instead of cross-fading, erasing only as far as the two titles diverge so the shared prompt does not stutter. The first hydrated heading starts empty and types once; later headings inherit the exact visible characters from the newest outgoing instance through an ownership-guarded handoff, so rapid navigation cannot revive stale text. Erase and type are separate frame-driven time budgets, font ligatures stay disabled while the command text changes, and reduced motion sets the text outright. The caret exists only while the animation runs, so nothing blinks indefinitely.
- A control that reloads data keeps that data on screen. Only a first load may replace the content with a loading state; a refresh shows its own busy state on the control — the shared `.spinning` class in `ui/src/app.css` — and leaves the records in place. A failed refresh reports itself alongside the current data and never trades a working view for an error card.
- `--product-view-height` is published on `.product-view`. A page that should fill the canvas sizes against that token rather than restating the viewport maths, and lets its own flex column hand the leftover height to the region that scrolls.

## Installed and mobile application shell

- Pandan uses `viewport-fit=cover`, so every fixed shell surface must include the relevant
  `safe-area-inset-*`: the dashboard header and sidebar, fixed player and toast, authentication
  chrome, dialogs, and their persistent action rails. A standalone launch in portrait or landscape
  must never place a control under a notch, status area, or home indicator.
- At phone widths the header keeps navigation, command search, and notifications visible. The
  GitHub link is desktop-only, and the dashboard hides its duplicate header label while Edit layout
  and Add widget are present; the overview heading immediately below continues to identify the page.
- Installation guidance belongs in Account settings. Offline and update state are shell-level
  status surfaces, not page-specific content, and must stay usable before authentication.

## Kanban board surfaces

- Creating something shows where it went. A new card wipes in and a new column travels in, played once from a flag the page clears; do not animate the whole board on load or on every refetch. A card's entrance must not animate `transform`: dnd-kit owns that property on a sortable card and a drag begun mid-entrance would fight it.
- Columns reorder through the title strip at the top of each column. Keep that strip as the only column drag handle, keep keyboard sorting available through the focused handle, and persist the result through `list:edit`. Column commands live behind one three-dot menu with Add card plus permission-gated Edit column and Delete column actions. Editing reuses the centered column workflow dialog and must not move the column or its cards. Destructive column actions confirm on a second press in that same menu; the server still refuses a column that holds active cards and the board surfaces that message rather than predicting it.
- Add card remains available from the column menu and the full-width footer control. Both entry points open the same centered native dialog, matching Add column and using the shared modal surface and motion. These board-creation dialogs use a light backdrop veil rather than obscuring the board, and their footer actions place Cancel before the primary action. Every Kanban dialog dismisses when its backdrop is pressed through its normal close lifecycle, including card-detail autosave. Compact Kanban dialogs overlay their close control in the top-right corner so the eyebrow keeps its natural top-left start. Every dismissal path retains the shared dialog exit, with movement removed for reduced motion.
- Card descriptions stay in the detail dialog; a card summary shows only a paragraph indicator when a description exists. Right-click and Shift+F10 open a compact, cursor-positioned card action dialog containing only permission-gated Edit, Duplicate, and Delete buttons; a second right-click closes it. Duplicate copies the editable card fields into the same column without copying comments, checklists, or attachments. Delete confirms on a second press and uses the server-authorized card deletion path.

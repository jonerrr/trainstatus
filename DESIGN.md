# trainstat.us design system

Use a dense, boxy industrial transit interface with dark OLED surfaces and one
interactive accent. Preserve the existing data flow, accessible names, roles,
pressed/selected states, keyboard navigation, and DOM query contracts.

## Geometry and density

- Use `border-radius: 0px` for containers, modals, buttons, inputs, checkboxes,
  radios, range tracks and thumbs, search fields, dropdowns, cards, bus route
  chips, transfer car boxes, and stop timeline nodes.
- Reserve true circles for official MTA subway route bullets in
  `frontend/src/lib/Route/Icon.svelte` and the (+)/(−) disclosure icons in
  `frontend/src/lib/Trip/TransferThread.svelte`. Preserve official route artwork
  and provider route colors; bus badges stay rectangular.
- Keep information compact while retaining existing touch targets and scrolling.
  Stop selectors are at least 44px tall; their timeline nodes are 14×14px squares.
  Transfer preview targets are 44px wide and centered over the thread rail.
- Train diagrams keep a fixed 54px car pitch and `car_count * 54 + 12` width.
  Narrow viewports scroll horizontally instead of shrinking cars below 50px.

## Color tokens

`frontend/src/app.css` owns the palette. Use its semantic Tailwind colors for
ordinary layouts and CSS custom properties for SVG and complex selectors.

| Purpose                | Token               | Default                    |
| ---------------------- | ------------------- | -------------------------- |
| Canvas                 | `--bg-canvas`       | `#0a0a0a`                  |
| Primary surface        | `--bg-surface-1`    | `#141414`                  |
| Raised surface         | `--bg-surface-2`    | `#1f1f1f`                  |
| Hover surface          | `--bg-hover`        | `#262626`                  |
| Modal backdrop         | `--bg-backdrop`     | `rgba(0, 0, 0, 0.72)`      |
| Subtle border          | `--border-subtle`   | `#262626`                  |
| Default border         | `--border-default`  | `#404040`                  |
| Strong border          | `--border-strong`   | `#525252`                  |
| Primary text           | `--text-primary`    | `#f5f5f5`                  |
| Secondary text         | `--text-secondary`  | `#d4d4d4`                  |
| Muted text             | `--text-muted`      | `#a3a3a3`                  |
| Disabled text          | `--text-disabled`   | `#737373`                  |
| Interactive accent     | `--accent`          | `#f59e0b`                  |
| Accent hover           | `--accent-hover`    | `#fbbf24`                  |
| Text on solid accent   | `--accent-fg`       | `#0a0a0a`                  |
| Selected background    | `--accent-bg`       | `rgba(245, 158, 11, 0.11)` |
| Selected border        | `--accent-border`   | `rgba(245, 158, 11, 0.45)` |
| Highlighted train body | `--accent-car-fill` | `#2e210b`                  |
| Live status            | `--status-live`     | `#10b981`                  |
| Warning status         | `--status-warning`  | `#f97316`                  |
| Danger status          | `--status-danger`   | `#ef4444`                  |

The six accent variables define the Phosphor Amber theme. Changing them must
re-theme every interactive state, including focus outlines, controls, active
navigation, dropdown selections, links, stop selections, and transfer guidance.
Use `--accent-fg` on solid accent backgrounds and `--accent-bg` for selected row
washes. Keep live/warning/danger colors semantic rather than using them as
selection colors. Respect forced colors and reduced motion preferences.

## Typography and transit diagrams

- Use tabular numerals for arrival countdowns, clock times, walking distances,
  car positions, fleet numbers, and stop IDs. The app inherits this from the
  global base typography; use `tabular-nums` for isolated rendering where needed.
- Show a car's position above its distinct fleet ID. Position text is 9.5px,
  bold; fleet IDs are 7.5px in a monospace font. Without a distinct fleet ID,
  center the position on one line.
- Highlight train bodies with `--accent-car-fill`, an accent outline, white
  position text, and `--accent-hover` fleet text. Exit shafts start at y=25;
  transfer shafts use the accent. Center selected transfer guidance horizontally
  without scrolling the surrounding modal.
- Transfer rows show up to three unique exit car positions joined by `·` in a
  sharp rectangular box on the rail. Active rows use the selected background;
  the active rail and car box use the accent.
- Subway trip headers show the available vehicle model and car count, such as
  `R142A · 10 cars`, in the same header column used for bus vehicle IDs.

## Implementation and verification

Prefer Tailwind utilities for layout and ordinary styling. Use scoped CSS for
SVG styling, pseudo-elements, browser control parts, and complex interactive
selectors. Keep focus visible and preserve native control semantics.

Run the frontend checks and existing unit/browser component tests after UI
changes. Preserve the platform shaft origin, measured car widths, transfer
preview accessible labels and pressed states, and direct transfer option
children relied on by the existing tests.

# Frontend agent instructions

These instructions apply to `frontend/` in addition to the repository's root `AGENTS.md`.

## UI/UX & Design System Rules (`DESIGN.md`)

Before creating or modifying any Svelte component, stylesheet, SVG diagram, or UI layout, read [../DESIGN.md](../DESIGN.md) and strictly enforce its rules:

- **Strict Boxy Geometry (`border-radius: 0px`):** All containers, modals, buttons, inputs, checkboxes, sliders, bus route chips, transfer car boxes, and stop timeline nodes must use `0px` border radius.
- **Reserved Circles (`border-radius: 9999px`):** True circles are strictly limited to official MTA Subway route bullets (`Icon.svelte`) and `TransferThread.svelte` `(+)` / `(-)` disclosure toggle icons.
- **Single Interactive Accent (`--accent`):** Never introduce hard-coded sky blue (`#7dd3fc`), OS blue (`#2563eb`), or stray emerald (`#10b981`) for interactive UI states. Always use the CSS custom properties (`--accent`, `--accent-hover`, `--accent-fg`, `--accent-bg`, `--accent-border`, `--accent-car-fill`) defined in `app.css`.
- **Tabular Numerals:** All arrival countdowns, clock times, walking distances, car numbers, and stop IDs must use `font-variant-numeric: tabular-nums`.

## Svelte MCP

Use the Svelte MCP server for Svelte 5 and SvelteKit documentation and code analysis.

### `list-sections`

At the start of a chat involving Svelte or SvelteKit, call this tool first to discover available documentation sections. It returns titles, `use_cases`, and paths. Use those fields to identify the sections relevant to the task.

### `get-documentation`

After `list-sections`, fetch all sections relevant to the task before answering framework questions or implementing changes. This tool accepts a single section or multiple sections; batch related sections in one call.

### `svelte-autofixer`

Whenever writing or modifying Svelte code, run this tool before delivering the result, including code written to project files. Address the reported issues and suggestions, then rerun it until none remain. This analysis complements the repository's type checks and tests.

### `playground-link`

For standalone Svelte code supplied in the conversation, ask whether the user wants a Playground link after completing the code. Call this tool only after confirmation. Never generate a Playground link for code written to project files.

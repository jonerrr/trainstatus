# Frontend agent instructions

These instructions apply to `frontend/` in addition to the repository's root `AGENTS.md`.

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

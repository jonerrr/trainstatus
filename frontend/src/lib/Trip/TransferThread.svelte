<script lang="ts">
	import type { Snippet } from 'svelte';

	import { CircleMinus, CirclePlus } from '@lucide/svelte';

	let {
		label,
		open = $bindable(true),
		nested = false,
		children
	}: {
		label: string;
		open?: boolean;
		nested?: boolean;
		children: Snippet;
	} = $props();
	const id = $props.id();
</script>

<details class={['thread', { nested }]} bind:open>
	<summary
		><span class="toggle"
			>{#if open}<CircleMinus size={16} />{:else}<CirclePlus size={16} />{/if}</span
		>{label}</summary
	>
	{#if open}<button
			type="button"
			class="rail"
			aria-label={`Collapse ${label.toLowerCase()}`}
			aria-controls={id}
			aria-expanded={open}
			onclick={() => (open = false)}
		></button>{/if}
	<div {id}>{@render children()}</div>
</details>

<style>
	.thread {
		position: relative;
		margin-left: 22px;
	}
	summary {
		display: flex;
		align-items: center;
		gap: 6px;
		min-height: 32px;
		margin-left: -8px;
		list-style: none;
		cursor: pointer;
		font-size: 12px;
		color: var(--text-secondary);
	}
	summary::-webkit-details-marker {
		display: none;
	}
	summary:hover {
		color: white;
	}
	.toggle {
		display: grid;
		place-items: center;
		background: var(--bg-surface-1);
	}
	.rail {
		position: absolute;
		left: -12px;
		top: 24px;
		bottom: 0;
		width: 24px;
		z-index: 1;
		cursor: pointer;
	}
	.rail::before {
		content: '';
		position: absolute;
		left: 11px;
		inset-block: 0;
		width: 2px;
		background: var(--border-strong);
	}
	summary:focus-visible,
	.rail:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}
	.rail:hover::before,
	.rail:focus-visible::before {
		background: var(--text-secondary);
	}
	.nested::before {
		content: '';
		position: absolute;
		left: -23px;
		top: -1px;
		width: 15px;
		height: 18px;
		border-left: 2px solid var(--border-strong);
		border-bottom: 2px solid var(--border-strong);
		border-bottom-left-radius: 0;
		pointer-events: none;
	}
	.nested summary {
		color: var(--text-muted);
		font-size: 11px;
	}
</style>

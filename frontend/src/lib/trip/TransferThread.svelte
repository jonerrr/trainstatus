<script lang="ts">
	import type { Snippet } from 'svelte';

	import { CircleMinus, CirclePlus } from '@lucide/svelte';

	let {
		label,
		open = $bindable(true),
		children
	}: {
		label: string;
		open?: boolean;
		children: Snippet;
	} = $props();
	const id = $props.id();
</script>

<div class="thread">
	<button
		type="button"
		class="toggle"
		aria-expanded={open}
		aria-controls={id}
		onclick={() => (open = !open)}
	>
		<span class="node">
			{#if open}<CircleMinus size={16} />{:else}<CirclePlus size={16} />{/if}
		</span>
		<span class="label">{label}</span>
	</button>
	<div {id} class="content" hidden={!open}>{@render children()}</div>
</div>

<style>
	.thread {
		position: relative;
		display: flex;
		flex-direction: column;
	}
	.thread::before {
		content: '';
		position: absolute;
		left: 21px;
		top: 18px;
		bottom: 18px;
		width: 2px;
		background: var(--border-strong);
		pointer-events: none;
	}
	.toggle {
		position: relative;
		display: grid;
		grid-template-columns: 44px minmax(0, 1fr);
		align-items: center;
		min-height: 36px;
		padding-right: 8px;
		cursor: pointer;
		text-align: left;
		font-size: 12px;
		color: var(--text-secondary);
	}
	.node {
		position: relative;
		justify-self: center;
		background: var(--bg-surface-1);
	}
	.label {
		min-width: 0;
	}
	.toggle:hover {
		background: var(--bg-hover);
		color: var(--text-primary);
	}
	.toggle:hover .node {
		background: var(--bg-hover);
	}
	.toggle:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}
</style>

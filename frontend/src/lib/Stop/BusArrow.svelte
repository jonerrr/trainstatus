<script lang="ts">
	import {
		ArrowDown,
		ArrowDownLeft,
		ArrowDownRight,
		ArrowLeft,
		ArrowRight,
		ArrowUp,
		ArrowUpLeft,
		ArrowUpRight
	} from '@lucide/svelte';
	import type { CompassDirection } from '@trainstatus/client';

	const { direction, size = '1.5rem' }: { direction: CompassDirection; size?: string } = $props();
</script>

<div>
	{#if direction === 's_w'}
		<ArrowDownLeft {size} />
	{:else if direction === 's'}
		<ArrowDown {size} />
	{:else if direction === 's_e'}
		<ArrowDownRight {size} />
	{:else if direction === 'e'}
		<ArrowRight {size} />
	{:else if direction === 'w'}
		<ArrowLeft {size} />
	{:else if direction === 'n_w'}
		<ArrowUpLeft {size} />
	{:else if direction === 'n_e'}
		<ArrowUpRight {size} />
	{:else if direction === 'n'}
		<ArrowUp {size} />
	{/if}
</div>

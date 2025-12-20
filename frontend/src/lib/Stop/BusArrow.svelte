<script lang="ts">
	import { type BusStopDirection } from '$lib/static';

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

	const { direction }: { direction: BusStopDirection } = $props();
</script>

<div>
	{#if direction === 'sw'}
		<ArrowDownLeft size="1.5rem" />
	{:else if direction === 's'}
		<ArrowDown size="1.5rem" />
	{:else if direction === 'se'}
		<ArrowDownRight size="1.5rem" />
	{:else if direction === 'e'}
		<ArrowRight size="1.5rem" />
	{:else if direction === 'w'}
		<ArrowLeft size="1.5rem" />
	{:else if direction === 'nw'}
		<ArrowUpLeft size="1.5rem" />
	{:else if direction === 'ne'}
		<ArrowUpRight size="1.5rem" />
	{:else if direction === 'n'}
		<ArrowUp size="1.5rem" />
	{/if}
</div>

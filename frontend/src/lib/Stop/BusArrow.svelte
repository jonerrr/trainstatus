<script lang="ts">
	import { BusStopDirection } from '$lib/static';
	import {
		ArrowDownLeft,
		ArrowDown,
		ArrowDownRight,
		ArrowRight,
		ArrowLeft,
		ArrowUpLeft,
		ArrowUpRight,
		ArrowUp
	} from 'lucide-svelte';

	const { direction }: { direction: BusStopDirection } = $props();

	// possible directions: SW, S, "", SE, E, W , NW, NE, N
</script>

<div>
	{#if direction === 'SW'}
		<ArrowDownLeft size="1.5rem" />
	{:else if direction === 'S'}
		<ArrowDown size="1.5rem" />
	{:else if direction === 'SE'}
		<ArrowDownRight size="1.5rem" />
	{:else if direction === 'E'}
		<ArrowRight size="1.5rem" />
	{:else if direction === 'W'}
		<ArrowLeft size="1.5rem" />
	{:else if direction === 'NW'}
		<ArrowUpLeft size="1.5rem" />
	{:else if direction === 'NE'}
		<ArrowUpRight size="1.5rem" />
	{:else if direction === 'N'}
		<ArrowUp size="1.5rem" />
	{/if}
</div>

<script lang="ts">
	import type { Trip } from '$lib/api';
	import { stops } from '$lib/stores';
	import List from '$lib/components/List.svelte';
	import Icon from '$lib/components/Icon.svelte';

	export let trips: Trip[];
	// if (!trips.length) {
	// 	console.log('no trips');
	// }
</script>

<!-- TODO: fix max-h -->
<List loading={false} class="h-96">
	{#if trips.length}
		{#each trips as trip (trip.id)}
			<div
				class="border-neutral-700 bg-neutral-800 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1 text-neutral-300"
			>
				<div class="flex gap-2 items-center justify-between mx-1">
					<div class="flex gap-2 items-center">
						<div class=""><Icon name={trip.route_id} /></div>
						<div>
							{trip.eta?.toFixed(0)}m
						</div>
					</div>
					<div class="text-right">
						{$stops.find((s) => s.id === trip.stop_times[trip.stop_times.length - 1].stop_id)?.name}
					</div>
				</div>
				<!-- TODO: show current stop / how many stops away -->
				<!-- TODO: make this trip dialog button -->
			</div>
		{/each}
	{:else}
		<div class="text-indigo-300 text-center">No trips found</div>
	{/if}
</List>

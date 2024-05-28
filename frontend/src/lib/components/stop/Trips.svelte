<script lang="ts">
	import type { Trip } from '$lib/api';
	import { stops } from '$lib/stores';
	import List from '$lib/components/List.svelte';
	import Icon from '$lib/components/Icon.svelte';

	export let trips: Trip[];
</script>

<!-- TODO: fix max-h -->
<List loading={false} class="h-96">
	{#if trips.length}
		{#each trips as trip (trip.id)}
			<div
				class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
			>
				<div class="flex gap-2 items-center justify-between mx-1">
					<div class="flex gap-2 items-center">
						<div class=""><Icon name={trip.route_id} /></div>
						<div>
							{trip.eta?.toFixed(0)}m
						</div>
					</div>
					<div>
						{$stops.find((s) => s.id === trip.stop_times[trip.stop_times.length - 1].stop_id)?.name}
					</div>
				</div>
			</div>
		{/each}
	{:else}
		<div class="text-indigo-300 text-center">No trips found</div>
	{/if}
</List>

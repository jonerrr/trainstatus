<script lang="ts">
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
	import { pushState } from '$app/navigation';
	import type { BusStop } from '$lib/bus_api';
	import { pinned_bus_stops, bus_routes } from '$lib/stores';
	import Pin from '$lib/components/Pin.svelte';
	import BusArrivals from '$lib/components/Stop/BusArrivals.svelte';
	import BusIcon from '$lib/components/BusIcon.svelte';

	export let stop: BusStop;

	// possible directions: SW, S, "", SE, E, W , NW, NE, N

	$: stop_route_ids = stop.routes.map((r) => r.id);

	$: stop_routes = $bus_routes.filter((r) => stop_route_ids.includes(r.id));
</script>

<button
	id="list-item"
	class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl hover:bg-neutral-900 px-1 w-full flex justify-between items-center py-1"
	on:click={() => {
		pushState('', {
			dialog_id: stop.id,
			dialog_type: 'bus_stop',
			dialog_open: true
		});
	}}
>
	<!-- TODO: make spacing consistent (use grid maybe idk) -->
	<div class="flex flex-col text-left text-xs">
		<div class="flex gap-2">
			<div>
				{#if stop.direction === 'SW'}
					<ArrowDownLeft size={16} />
				{:else if stop.direction === 'S'}
					<ArrowDown size={16} />
				{:else if stop.direction === 'SE'}
					<ArrowDownRight size={16} />
				{:else if stop.direction === 'E'}
					<ArrowRight size={16} />
				{:else if stop.direction === 'W'}
					<ArrowLeft size={16} />
				{:else if stop.direction === 'NW'}
					<ArrowUpLeft size={16} />
				{:else if stop.direction === 'NE'}
					<ArrowUpRight size={16} />
				{:else if stop.direction === 'N'}
					<ArrowUp size={16} />
				{/if}
			</div>
			<div class="font-bold">
				{stop.name}
			</div>
			<div class="text-neutral-300 font-bold">
				#{stop.id}
			</div>
		</div>

		<div class="flex flex-col">
			{#each stop_routes as route}
				<div class="flex gap-2 items-center text-xs text-wrap text-left rounded p-1">
					<BusIcon {route} />
					<div class="text-neutral-100 max-w-[60%]">
						{stop.routes.find((r) => r.id === route.id)?.headsign}
					</div>

					<div class="">
						<BusArrivals route_id={route.id} stop_id={stop.id} />
					</div>
				</div>
			{/each}
		</div>
	</div>

	<div>
		<Pin item_id={stop.id} store={pinned_bus_stops} />
	</div>
</button>

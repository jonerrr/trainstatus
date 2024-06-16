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

	export let stop: BusStop;

	// possible directions: SW, S, "", SE, E, W , NW, NE, N

	$: stop_route_ids = stop.routes.map((r) => r.id);

	$: stop_routes = $bus_routes.filter((r) => stop_route_ids.includes(r.id));
</script>

<div class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl hover:bg-neutral-900 px-1">
	<button
		class="w-full flex justify-between items-center py-1"
		on:click={() => {
			// pushState('', {
			// 	dialog_id: stop.id,
			// 	dialog_type: 'stop',
			// 	dialog_open: true
			// });
		}}
	>
		<div class="flex flex-col text-left text-xs grow-0 font-semibold text-indigo-200">
			<div class="flex gap-2">
				<div class="">
					{stop.name}
				</div>
				<div class="text-neutral-300 font-bold">
					#{stop.id}
				</div>
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
			</div>

			<div class="flex flex-col gap-1">
				{#each stop_routes as route}
					<div class="text-xs text-indigo-100 text-wrap text-left rounded p-1">
						<span style={`background-color: #${route.color}`} class="p-1 rounded">
							{route.short_name}
						</span> Coming Soon
					</div>
				{/each}
			</div>
		</div>

		<div>
			<Pin item_id={stop.id} store={pinned_bus_stops} />
		</div>
	</button>
</div>

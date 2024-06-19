<script lang="ts">
	import { derived } from 'svelte/store';
	import { pushState } from '$app/navigation';
	import { bus_trips } from '$lib/stores';
	import type { BusRoute, BusStop, BusStopTime } from '$lib/bus_api';
	import BusIcon from '$lib/components/BusIcon.svelte';
	import BusCapacity from '$lib/components/Trip/BusCapacity.svelte';

	export let stop: BusStop;
	export let route: BusRoute;
	export let stop_time: BusStopTime;

	const trip = derived(bus_trips, ($bus_trips) => {
		return $bus_trips.find((t) => t.id === stop_time.trip_id);
	});
	// console.log($trip);
	// TODO: make sure data auto updates properly
	// TODO: show if its only scheduled and not real time (maybe check if bus has position or if it hasn't left first stop)
</script>

<button
	class="w-full flex justify-between items-center py-1"
	on:click={() => {
		pushState('', {
			dialog_open: true,
			dialog_id: stop_time.trip_id,
			dialog_type: 'bus_trip'
		});
	}}
>
	<div
		class="w-full border-neutral-700 bg-neutral-800 rounded border shadow-2xl hover:bg-neutral-900 text-neutral-300 flex items-center justify-between p-1"
	>
		{#if $trip}
			<div class="flex gap-2 items-center">
				<div class="flex flex-col gap-1">
					{#if $trip.passengers && $trip.capacity}
						<BusCapacity passengers={$trip.passengers} capacity={$trip.capacity} />
					{/if}
					<BusIcon {route} />
				</div>

				<div class="flex flex-col">
					{stop_time.eta?.toFixed(0)}m
					{#if $trip.progress_status === 'layover'}
						<div class="text-neutral-400 text-xs">+Layover</div>
					{/if}
				</div>
			</div>
		{/if}

		<div class="text-right">
			{stop.routes.find((r) => r.id === stop_time.route_id)?.headsign}
		</div>
	</div>
</button>

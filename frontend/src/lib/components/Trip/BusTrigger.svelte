<script lang="ts">
	import { Users } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { pushState } from '$app/navigation';
	import { bus_trips } from '$lib/stores';
	import BusIcon from '$lib/components/BusIcon.svelte';
	import type { BusRoute, BusStop, BusStopTime } from '$lib/bus_api';

	export let stop: BusStop;
	export let route: BusRoute;
	export let stop_time: BusStopTime;

	const trip = derived(bus_trips, ($bus_trips) => {
		return $bus_trips.find((t) => t.id === stop_time.trip_id);
	});
	// console.log($trip);
	// TODO: make sure data auto updates properly
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
					{#if $trip.passengers}
						<!-- TODO: change color of icon based on ratio of passengers and capacity -->
						<div class="flex items-center gap-1">
							<Users size="16" />
							{$trip.passengers}
						</div>
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

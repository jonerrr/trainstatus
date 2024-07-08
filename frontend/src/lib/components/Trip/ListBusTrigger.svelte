<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { bus_stop_times, bus_stops, pinned_bus_trips, bus_routes } from '$lib/stores';
	import { type BusTrip } from '$lib/bus_api';
	import Pin from '$lib/components/Pin.svelte';
	import BusIcon from '$lib/components/BusIcon.svelte';
	import TriggerButton from '$lib/components/TriggerButton.svelte';
	import BusCapacity from '$lib/components/Trip/BusCapacity.svelte';

	export let trip: BusTrip;

	$: trip_stop_times = derived(bus_stop_times, ($stop_times) =>
		$stop_times.filter((st) => st.trip_id === trip.id)
	);

	// Check if trip stop id is in trip stop times, and if it isn't look up the first stop time
	// $: current_stop_id =
	// 	trip.train_status && trip.stop_id ? trip.stop_id : $trip_stop_times.at(0)?.stop_id;
	$: current_stop_id = trip.stop_id || $trip_stop_times.at(0)?.stop_id;
	$: current_stop = $bus_stops.find((s) => s.id === current_stop_id);

	$: route = $bus_routes.find((r) => r.id === trip.route_id)!;
</script>

<!-- TODO: make button component and reuse -->
<TriggerButton
	state={{
		dialog_id: trip.id,
		dialog_type: 'bus_trip',
		dialog_open: true
	}}
>
	<div class="flex gap-1 items-center flex-wrap">
		<div class="flex flex-col gap-1">
			{#if trip.passengers && trip.capacity}
				<BusCapacity passengers={trip.passengers} capacity={trip.capacity} />
			{/if}
			<BusIcon {route} />
		</div>
		<ArrowBigRight />
		<div class={`pr-2`}>
			{trip.headsign}
		</div>
		{#if trip.deviation && Math.abs(trip.deviation) > 120}
			<div class={`text-xs ${trip.deviation > 0 ? 'text-red-400' : 'text-green-400'}`}>
				{(trip.deviation / 60).toFixed(0)}m
			</div>
		{/if}

		<div>
			<span class="text-neutral-300">Current stop:</span>
			{current_stop?.name}
		</div>
	</div>

	<Pin item_id={trip.id + '_' + trip.route_id} store={pinned_bus_trips} />
</TriggerButton>

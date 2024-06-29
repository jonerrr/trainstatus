<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { stop_times, stops, pinned_trips } from '$lib/stores';
	import { TrainStatus, type Trip } from '$lib/api';
	import Pin from '$lib/components/Pin.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import TriggerButton from '$lib/components/TriggerButton.svelte';

	export let trip: Trip;

	$: trip_stop_times = derived(stop_times, ($stop_times) =>
		$stop_times.filter((st) => st.trip_id === trip.id)
	);

	// Check if trip stop id is in trip stop times, and if it isn't look up the first stop time
	$: current_stop_id =
		trip.train_status && trip.stop_id ? trip.stop_id : $trip_stop_times.at(0)?.stop_id;
	// ? $trip_stop_times.find((s) => s.stop_id === trip.stop_id)?.stop_id
	// : $trip_stop_times.at(0)?.stop_id;
	$: current_stop = $stops.find((s) => s.id === current_stop_id);

	$: last_stop = $stops.find((s) => s.id === $trip_stop_times.at(-1)?.stop_id);
</script>

<!-- TODO: make button component and reuse -->
<TriggerButton
	state={{
		dialog_id: trip.id,
		dialog_type: 'trip',
		dialog_open: true
	}}
>
	<div class="flex gap-1 items-center">
		<Icon width="2rem" height="2rem" name={trip.route_id} />
		<ArrowBigRight />
		<div class={`${!trip.assigned ? 'italic' : ''} pr-2`}>
			{last_stop?.name}
		</div>
	</div>

	{#if trip.train_status === TrainStatus.AtStop}
		<div class="text-neutral-400">Arrived at {current_stop?.name}</div>
	{:else if trip.train_status === TrainStatus.InTransitTo}
		<div class="text-neutral-400">In transit to {current_stop?.name}</div>
	{:else if trip.train_status === TrainStatus.Incoming}
		<div class="text-neutral-400">Arriving at {current_stop?.name}</div>
	{:else if !trip.assigned}
		<div class="text-neutral-400">Not assigned</div>
	{:else}
		<div class="text-neutral-400">Next stop: {current_stop?.name}</div>
	{/if}
	<Pin item_id={trip.id} store={pinned_trips} />
</TriggerButton>

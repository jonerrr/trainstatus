<script lang="ts">
	import { derived } from 'svelte/store';
	import { TrainStatus, type Stop, type StopTime } from '$lib/api';
	import { stops, stop_times, trips } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import TriggerButton from '$lib/components/TriggerButton.svelte';

	// export let stop: Stop;
	export let stop_time: StopTime;

	$: trip = derived(trips, ($trips) => {
		return $trips.find((t) => t.id === stop_time.trip_id);
	});

	// should i sort by arrival (i think its already sorted)
	$: trip_stop_times = derived(stop_times, ($stop_times) => {
		return $stop_times.filter((st) => st.trip_id === stop_time.trip_id);
	});

	$: last_stop = $stops.find((s) => s.id === $trip_stop_times.at(-1)?.stop_id);

	// $: stops_away = $trip_stop_times.findIndex((st) => st.stop_id === stop_time.stop_id);
</script>

<TriggerButton
	state={{
		dialog_open: true,
		dialog_id: stop_time.trip_id,
		dialog_type: 'trip'
	}}
>
	<div class="flex gap-2 items-center">
		<!-- maybe enable link here -->
		<Icon
			width="20px"
			height="20px"
			express={$trip?.express}
			link={false}
			name={stop_time.route_id}
		/>
		{#if stop_time.eta && stop_time.eta > 0}
			<div class={`${!$trip?.assigned ? 'italic' : ''}`}>
				{stop_time.eta?.toFixed(0)}m
			</div>
		{:else}
			<div class={`${!$trip?.assigned ? 'italic' : ''} text-neutral-400`}>
				{stop_time.arrival.toLocaleTimeString()}
			</div>
		{/if}

		<!-- {#if stops_away > 0}
					<div class="text-indigo-200 text-xs">
						{stops_away} stop{stops_away > 1 ? 's' : ''} away
					</div>
				{/if} -->
	</div>
	<div class="text-right">
		{last_stop?.name}
	</div>
</TriggerButton>

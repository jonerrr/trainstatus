<script lang="ts">
	import { derived } from 'svelte/store';
	import { pushState } from '$app/navigation';
	import { TrainStatus, type Stop, type StopTime } from '$lib/api';
	import { stops, stop_times, trips } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';

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

<button
	id="list-item"
	class="w-full border-y border-neutral-400 mt-[-1px] p-[.5rem] flex justify-between items-center text-indigo-200 px-1"
	on:click={() => {
		pushState('', {
			dialog_open: true,
			dialog_id: stop_time.trip_id,
			dialog_type: 'trip'
		});
	}}
>
	<!-- <div
		class="w-full border-neutral-700 bg-neutral-800 rounded border shadow-2xl hover:bg-neutral-900 px-1 text-neutral-300"
	> -->
	<!-- TODO: Some sort of animation when trip status is stopped to / arriving at this stop  -->
	<div class="flex gap-2 items-center">
		<!-- maybe enable link here -->
		<Icon
			width="20px"
			height="20px"
			express={$trip?.express}
			link={false}
			name={stop_time.route_id}
		/>
		<div class={`${!$trip?.assigned ? 'italic' : ''}`}>
			{stop_time.eta?.toFixed(0)}m
		</div>

		<!-- {#if stops_away > 0}
					<div class="text-indigo-200 text-xs">
						{stops_away} stop{stops_away > 1 ? 's' : ''} away
					</div>
				{/if} -->
	</div>
	<div class="text-right">
		{last_stop?.name}
	</div>
	<!-- </div> -->
</button>

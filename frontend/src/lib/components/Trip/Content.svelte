<script lang="ts">
	import { derived } from 'svelte/store';
	import type { Trip } from '$lib/api';
	import { stops, trips, stop_times } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import Times from '$lib/components/Trip/Times.svelte';

	export let trip_id: string;

	const trip: Trip = $trips.find((t) => t.id === trip_id)!;
	const trip_stop_times = derived(stop_times, ($stop_times) =>
		$stop_times.filter((st) => st.trip_id === trip_id)
	)!;
	const last_stop_name = derived(trip_stop_times, ($trip_stop_times) => {
		const last_stop_id = $trip_stop_times[$trip_stop_times.length - 1].stop_id;
		return $stops.find((s) => s.id === last_stop_id)!.name;
	});

	// TODO: add button to load previous stop times and fetch from api
</script>

<!-- list of stops and their arrival times -->
<div
	class="relative overflow-auto text-white bg-neutral-800/90 border border-neutral-700 p-1 max-h-[80vh]"
>
	<div class="fixed flex items-center gap-2 bg-neutral-800 pr-12">
		<Icon width="2rem" height="2rem" name={trip.route_id} />

		<h2 class="font-bold text-xl text-indigo-300">
			{$last_stop_name}
		</h2>
	</div>

	{#if $trip_stop_times}
		<div class="pt-10 max-h-full">
			{#each $trip_stop_times as stop_time}
				<Times {stop_time} />
			{/each}
		</div>
	{/if}
</div>

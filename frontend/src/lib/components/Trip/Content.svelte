<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { stops, trips, stop_times } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import Times from '$lib/components/Trip/StopTime.svelte';

	export let trip_id: string;
	export let actions_width: number;

	let show_previous_stops = false;

	$: trip = derived(trips, ($trips) => {
		return $trips.find((t) => t.id === trip_id);
	});

	$: trip_stop_times = derived(stop_times, ($stop_times) => {
		return $stop_times.filter((st) => st.trip_id === trip_id);
	});

	$: last_stop = $trip_stop_times
		? $stops.find((s) => s.id === $trip_stop_times[$trip_stop_times.length - 1]?.stop_id)
		: undefined;

	// TODO: add button to load previous stop times and fetch from api
	// TODO: fix state not updating properly
</script>

<svelte:head>
	{#if $trip && last_stop}
		<title>{$trip.route_id} | {last_stop.name}</title>
	{/if}
</svelte:head>

<!-- list of stops and their arrival times -->
<div
	class="relative overflow-auto text-white bg-neutral-800/90 border border-neutral-700 p-1 max-h-[80dvh]"
>
	<div
		style={`max-width: calc(100% - ${actions_width}px);`}
		class="flex text-indigo-400 items-center bg-neutral-800 pt-1"
	>
		{#if $trip}
			<Icon express={$trip.express} width="2rem" height="2rem" name={$trip.route_id} />

			<ArrowBigRight class="w-8" />

			<h2 class={`font-bold text-xl text-indigo-300 ${$trip.assigned ? '' : 'italic'}`}>
				{last_stop?.name}
			</h2>
		{/if}
	</div>

	{#if $trip && $trip_stop_times.length}
		<div class="max-h-[75dvh] pt-2 overflow-auto">
			{#each $trip_stop_times as stop_time (stop_time.stop_id)}
				<Times stop_id={$trip.stop_id} train_status={$trip.train_status} {stop_time} />
			{/each}
		</div>
	{/if}
</div>

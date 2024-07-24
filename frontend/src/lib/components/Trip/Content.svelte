<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { stops, trips, stop_times, data_at } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';
	import Times from '$lib/components/Trip/StopTime.svelte';
	import List from '$lib/components/ContentList.svelte';

	export let trip_id: string;
	export let show_previous: boolean = false;
	// export let actions_width: number;

	$: trip = derived(trips, ($trips) => {
		return $trips.find((t) => t.id === trip_id);
	});

	$: trip_stop_times = derived(stop_times, ($stop_times) => {
		const st = show_previous
			? $stop_times.filter((st) => st.trip_id === trip_id)
			: $stop_times.filter((st) => st.trip_id === trip_id && st.arrival > ($data_at ?? new Date()));
		return st;
	});

	$: last_stop = $trip_stop_times
		? $stops.find((s) => s.id === $trip_stop_times[$trip_stop_times.length - 1]?.stop_id)
		: undefined;

	// TODO: add button to load previous stop times and fetch from api
</script>

<svelte:head>
	{#if $trip && last_stop}
		<title>{$trip.route_id} | {last_stop.name}</title>
	{/if}
</svelte:head>

<!-- list of stops and their arrival times -->

<div class="flex text-indigo-400 items-center p-1">
	{#if $trip}
		<Icon express={$trip.express} width="2rem" height="2rem" name={$trip.route_id} />

		<ArrowBigRight class="w-8" />

		<h2 class={`font-bold text-xl text-indigo-300 ${$trip.assigned ? '' : 'italic'}`}>
			{last_stop?.name}
		</h2>
	{/if}
</div>

{#if $trip && $trip_stop_times.length}
	<List>
		{#each $trip_stop_times as stop_time (stop_time.stop_id)}
			<Times stop_id={$trip.stop_id} train_status={$trip.train_status} {stop_time} />
		{/each}
	</List>
{/if}

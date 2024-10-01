<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { bus_trips, bus_stop_times, bus_routes } from '$lib/stores';
	import BusIcon from '$lib/components/BusIcon.svelte';
	import BusTimes from '$lib/components/Trip/BusTimes.svelte';
	import BusCapacity from '$lib/components/Trip/BusCapacity.svelte';
	import List from '$lib/components/ContentList.svelte';

	export let trip_id: string;
	// export let actions_width: number;

	$: trip = derived(bus_trips, ($bus_trips) => {
		return $bus_trips.find((t) => t.id === trip_id);
	});
	$: stop_times = derived(bus_stop_times, ($bus_stop_times) => {
		return $bus_stop_times.filter((st) => st.trip_id === trip_id);
	});
	$: route = derived(bus_routes, ($bus_routes) => {
		return $bus_routes.find((r) => r.id === $trip?.route_id);
	});

	// TODO: add button to load previous stop times and fetch from api
</script>

<svelte:head>
	{#if $trip}
		<title>{$trip.route_id} | {$trip.headsign}</title>
	{/if}
</svelte:head>

<!-- list of stops and their arrival times -->
<div class="flex items-center text-indigo-400 p-1">
	{#if $trip && $route}
		<div class="flex flex-col">
			{#if $trip.passengers}
				{#if $trip.passengers && $trip.capacity}
					<BusCapacity passengers={$trip.passengers} capacity={$trip.capacity} />
				{/if}
			{/if}
			<BusIcon route={$route} />
		</div>

		<ArrowBigRight class="w-8" />

		<h2 class="font-bold text-xl text-indigo-300">{$trip.headsign}</h2>
	{/if}
</div>

{#if $stop_times.length}
	<List>
		{#each $stop_times as stop_time (stop_time.stop_id)}
			<BusTimes {stop_time} />
		{/each}
	</List>
{/if}

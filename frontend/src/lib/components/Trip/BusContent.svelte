<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { bus_trips, bus_stop_times, bus_routes } from '$lib/stores';
	import BusIcon from '$lib/components/BusIcon.svelte';
	import BusTimes from '$lib/components/Trip/BusTimes.svelte';
	import BusCapacity from '$lib/components/Trip/BusCapacity.svelte';

	export let trip_id: string;

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

<!-- list of stops and their arrival times -->
<div class="relative text-white bg-neutral-800/90 border border-neutral-700 p-1 rounded">
	<div class="flex items-center justify-between bg-neutral-800 w-full">
		<div class="flex max-w-[calc(100%-65px)] gap-2 items-center text-indigo-400">
			{#if $trip && $route}
				<div class="flex flex-col">
					{#if $trip.passengers}
						{#if $trip.passengers && $trip.capacity}
							<BusCapacity passengers={$trip.passengers} capacity={$trip.capacity} />
						{/if}
					{/if}
					<BusIcon route={$route} />
				</div>

				<ArrowBigRight />

				<h2 class="font-bold text-xl text-indigo-300">{$trip.headsign}</h2>
			{:else}
				<h1 class="p-2">Trip not found</h1>
			{/if}
		</div>
	</div>

	{#if $stop_times.length}
		<div class="max-h-[75dvh] pt-1 flex flex-col gap-1 overflow-auto">
			{#each $stop_times as stop_time (stop_time.stop_id)}
				<BusTimes {stop_time} />
			{/each}
		</div>
	{/if}
</div>

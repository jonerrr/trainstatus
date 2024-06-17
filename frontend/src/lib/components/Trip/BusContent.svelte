<script lang="ts">
	import { Share, ClipboardCheck, ArrowBigRight, Users } from 'lucide-svelte';
	import { derived } from 'svelte/store';
	import { bus_stops, bus_trips, bus_stop_times, bus_routes } from '$lib/stores';
	import BusIcon from '$lib/components/BusIcon.svelte';
	import BusTimes from '$lib/components/Trip/BusTimes.svelte';

	export let trip_id: string;

	// TODO: Fix trip not found error when loading from URL

	$: trip = derived(bus_trips, ($bus_trips) => {
		console.log('searching for trip');
		return $bus_trips.find((t) => t.id === trip_id);
	});
	// TODO: maybe check and make sure arrival is gt now
	$: stop_times = derived(bus_stop_times, ($bus_stop_times) => {
		return $bus_stop_times.filter((st) => st.trip_id === trip_id);
	});
	$: route = derived(bus_routes, ($bus_routes) => {
		return $bus_routes.find((r) => r.id === $trip?.route_id);
	});

	$: last_stop = $bus_stops.find((s) => s.id === $stop_times[$stop_times.length - 1]?.stop_id);
	$: console.log($trip?.stop_id);

	let copied = false;
	function share() {
		// pr param is a list of route ids that should be monitored
		let url = window.location.origin + `/?bt=${trip_id}&pr=${$trip?.route_id}`;

		if (!navigator.share) {
			navigator.clipboard.writeText(url);
			// set copied to true and change back in 500 ms
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 800);
		} else {
			navigator.share({
				// TODO: maybe include next stop and route name
				// TODO: custom embeds
				title: `${$trip?.route_id} to ${last_stop?.name}`,
				url
			});
		}
	}

	// TODO: add button to load previous stop times and fetch from api
</script>

<!-- list of stops and their arrival times -->
<div
	class="relative overflow-auto text-white bg-neutral-800/90 border border-neutral-700 p-1 max-h-[80vh]
	max-w-[450px]"
>
	<div class="flex items-center justify-between bg-neutral-800 w-full">
		<div class="flex gap-2 items-center text-indigo-400">
			{#if $trip && $route}
				<div class="flex flex-col">
					{#if $trip.passengers}
						<!-- TODO: change color of icon based on ratio of passengers and capacity -->
						<div class="flex items-center gap-1 text-neutral-200">
							<Users size="16" />
							{$trip.passengers}
						</div>
					{/if}
					<BusIcon route={$route} />
				</div>

				<ArrowBigRight />

				<h2 class="font-bold text-xl text-indigo-300">{last_stop?.name}</h2>
			{:else}
				<h1>Trip not found</h1>
			{/if}
		</div>
		<!-- 
		{#if $trip && $route}
			<div class="pr-2">
				{#if !copied}
					<button aria-label="Share trip" on:click={share}>
						<Share class="h-6 w-6" />
					</button>
				{:else}
					<button class="text-green-600" aria-label="Trip link copied to clipboard">
						<ClipboardCheck class="h-6 w-6" />
					</button>
				{/if}
			</div>
		{/if} -->
	</div>

	{#if $stop_times.length}
		<div class="max-h-full">
			{#each $stop_times as stop_time}
				<BusTimes {stop_time} />
			{/each}
		</div>
	{/if}
</div>

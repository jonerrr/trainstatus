<script lang="ts">
	import List from '$lib/components/List.svelte';
	import { onMount } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { fetch_trips, type Trip } from '$lib/api';
	import { pinned_stops, stops } from '$lib/stores';
	import TripPreview from '$lib/components/stop/Preview.svelte';

	let loading_stops = true;

	let trips: Trip[] = [];
	$: pinned_stop_data = $stops.filter((stop) => $pinned_stops.includes(stop.id));

	onMount(async () => {
		trips = await fetch_trips(fetch, $pinned_stops);
		loading_stops = false;
		setInterval(async () => {
			console.log('fetching trips');
			trips = await fetch_trips(fetch, $pinned_stops);
		}, 10000);
		// trips = await fetch_trips(fetch, $pinned_stops);

		pinned_stops.subscribe(async (pinned_stops) => {
			trips = await fetch_trips(fetch, pinned_stops);
		});
	});

	// maybe in the future use https://melt-ui.com/docs/builders/tooltip for interactive tutorial
</script>

<div class="p-2 text-indigo-200 text-sm">
	<section class="flex flex-col gap-2">
		<!-- <h2 class="text-2xl font-semibold text-indigo-800">Pinned Stops</h2> -->
		<!-- maybe use svelte context module or something else for list stuff -->
		<List bind:loading={loading_stops} class="bg-neutral-800/90 border border-neutral-700 p-1">
			<div slot="header" class="flex self-center mb-2 w-full justify-between">
				<div class="font-semibold text-indigo-300">Pinned Stops</div>
				<div>Northbound</div>
				<div>Southbound</div>
				<div></div>
			</div>
			{#if trips.length === 0}
				<div
					transition:slide={{ easing: quintOut, axis: 'y', delay: 100 }}
					class="text-center text-indigo-600 font-medium"
				>
					No stops pinned
				</div>
			{/if}
			{#each pinned_stop_data as stop (stop.id)}
				<div
					class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
					transition:slide={{ easing: quintOut, axis: 'y' }}
				>
					<TripPreview bind:trips {stop} />
					<!-- <div role="separator" class="my-2 h-px w-full bg-indigo-600" /> -->
				</div>
			{/each}
		</List>
	</section>
</div>

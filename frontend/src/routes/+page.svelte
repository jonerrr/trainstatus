<script lang="ts">
	import List from '$lib/components/List.svelte';
	import { onMount } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { fetch_trips, fetch_alerts, type RouteAlerts, type Trip } from '$lib/api';
	import { pinned_stops, pinned_routes, stops } from '$lib/stores';
	import TripPreview from '$lib/components/stop/Preview.svelte';
	import RoutePreview from '$lib/components/route/Preview.svelte';

	let loading_stops = true;
	let loading_alerts = true;

	let trips: Trip[] = [];
	let alerts: RouteAlerts[] = [];
	$: pinned_stop_data = $stops.filter((stop) => $pinned_stops.includes(stop.id));

	onMount(async () => {
		trips = await fetch_trips(fetch, $pinned_stops);
		loading_stops = false;
		alerts = await fetch_alerts(fetch, $pinned_routes);
		loading_alerts = false;
		// TODO: make sure there are no duplicate intervals
		setInterval(async () => {
			console.log('fetching trips');
			trips = await fetch_trips(fetch, $pinned_stops);
		}, 10000);

		pinned_stops.subscribe(async (pinned_stops) => {
			trips = await fetch_trips(fetch, pinned_stops);
		});
	});

	// maybe in the future use https://melt-ui.com/docs/builders/tooltip for interactive tutorial
</script>

<div class="p-2 text-indigo-200 text-sm">
	<section class="flex flex-col gap-2">
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

		<List bind:loading={loading_alerts} class="bg-neutral-800/90 border border-neutral-700 p-1">
			<div slot="header" class="flex self-center mb-2 w-full">
				<div class="font-semibold text-indigo-300">Pinned Routes</div>
			</div>
			{#if trips.length === 0}
				<div
					transition:slide={{ easing: quintOut, axis: 'y', delay: 100 }}
					class="text-center text-indigo-600 font-medium"
				>
					No routes pinned
				</div>
			{/if}
			{#each $pinned_routes as route_id}
				<div
					class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
					transition:slide={{ easing: quintOut, axis: 'y' }}
				>
					<RoutePreview alerts={alerts.find((a) => a.route_id === route_id)} {route_id} />
				</div>
			{/each}
		</List>
	</section>
</div>

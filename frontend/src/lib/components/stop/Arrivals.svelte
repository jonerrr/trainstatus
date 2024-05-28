<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime.js';
	import { type Route, type Trip } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';

	dayjs.extend(relativeTime);

	export let headsign: string;
	export let routes: Route[];
	export let trips: Required<Trip>[];

	interface RouteTrips {
		[key: string]: Required<Trip>[];
	}
	const route_trips: RouteTrips = {};

	routes.forEach((route) => {
		route_trips[route.id] = [];
	});
	trips.forEach((trip) => {
		if (!route_trips[trip.route_id]) {
			console.log(`missing route ${trip.route_id} for trip ${trip.id}`);
			route_trips[trip.route_id] = [];
		}
		route_trips[trip.route_id].push(trip);
	});

	// TODO:
</script>

<div class="flex flex-col w-[30%] mt-auto">
	<div class="text-xs text-indigo-200 text-wrap text-left pb-1">
		{headsign}
	</div>

	<div class="flex flex-col gap-1">
		{#each routes as route}
			<div class="flex gap-1">
				<div class="">
					<Icon name={route.id} />
				</div>
				<div class="flex gap-2">
					{#if route_trips[route.id].length}
						<!-- TODO: make it clearer that these are arrivals -->
						{#each route_trips[route.id].slice(0, 2) as trip (trip.id)}
							<div class="text-xs">
								{trip.eta.toFixed(0)}m
							</div>
						{/each}
					{:else}
						<div class="text-xs text-neutral-400">No trips found</div>
					{/if}
				</div>
			</div>
		{/each}
	</div>
</div>

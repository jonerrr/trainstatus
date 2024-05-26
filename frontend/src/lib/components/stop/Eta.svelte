<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime.js';
	import { type Route, type Trip } from '$lib/api';
	import Icon from '$lib/components/Icon.svelte';

	dayjs.extend(relativeTime);

	// export let stop_id: string;
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
		route_trips[trip.route_id].push(trip);
	});

	console.log(route_trips);
</script>

<div class="flex flex-col gap-1">
	{#each routes as route}
		<div class="flex gap-2">
			<Icon name={route.id} />
			<div class="flex gap-1">
				{#each route_trips[route.id].slice(0, 2) as trip (trip.id)}
					<div class="text-xs">
						{trip.eta.toFixed(0)}m
					</div>
				{/each}
			</div>
		</div>
	{/each}
</div>
